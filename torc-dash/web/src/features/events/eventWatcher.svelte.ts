/**
 * Background event watcher for the selected workflow. Runs from module import (App imports it), so
 * the sidebar badge (`app.eventBadge`) updates even when the Events tab isn't mounted.
 *
 * The live stream is opt-in (`watcher.enabled`, toggled on the Events tab and remembered in
 * localStorage) and is closed while the browser tab is hidden, so idle dash tabs don't each hold
 * one of the browser's ~6 HTTP/1.1 connections to the server.
 *
 * Two sources, merged newest-first:
 * - live: the server's SSE stream (`/workflows/{id}/events/stream`). These broadcast events
 *   (job_started, job_completed, ...) are ephemeral and never stored, so we keep them here.
 * - stored: rows from `/events`, fetched once per workflow and then incrementally (only ids newer
 *   than the newest one seen) on every `app.refreshTick`.
 */
import { api, API_BASE, unwrap, type TorcEvent } from "../../lib/api/client";
import { withToken } from "../../lib/api/token";
import { untrack } from "svelte";
import { JOB_STATUSES } from "../../lib/format";
import { app } from "../../lib/state.svelte";

export type EventRow = {
  key: string;
  source: "live" | "stored";
  timestamp: number;
  severity: string;
  type: string;
  jobId: number | null;
  jobName: string | null;
  message: string;
  data: unknown;
};

export type StreamStatus = "off" | "connecting" | "live" | "reconnecting";

const MAX = 1000;
const PAGE = 200;
const ENABLED_KEY = "torc-dash.events.live";

/** SSE frames use `event: <type>`; EventSource only routes named events to explicit listeners. */
const SSE_TYPES = [
  ...JOB_STATUSES.map((s) => `job_${s}`),
  "job_started",
  "task_completed",
  "compute_node_started",
  "compute_node_stopped",
  "scheduler_node_created",
  "workflow_started",
  "workflow_reinitialized",
  "warning",
];

type Obj = Record<string, unknown>;
const str = (v: unknown) => (typeof v === "string" && v ? v : null);

function summarize(data: unknown): string {
  if (!data || typeof data !== "object") return data == null ? "" : String(data);
  const d = data as Obj;
  const msg = str(d.message);
  if (msg) return msg;
  // No message: show the remaining fields compactly (type/job are shown in their own columns).
  return Object.entries(d)
    .filter(([k]) => !HIDDEN_KEYS.has(k))
    .map(([k, v]) => `${k}=${typeof v === "object" ? JSON.stringify(v) : String(v)}`)
    .join("  ");
}
const HIDDEN_KEYS = new Set(["event_type", "type", "action", "job_id", "job_name", "workflow_id"]);

function fromStored(e: TorcEvent): EventRow {
  const d = (e.data && typeof e.data === "object" ? e.data : {}) as Obj;
  return {
    key: `s${e.id}`,
    source: "stored",
    timestamp: e.timestamp,
    severity: str(d.severity) ?? str(d.level) ?? "info",
    type: str(d.event_type) ?? str(d.type) ?? str(d.action) ?? str(d.category) ?? "event",
    jobId: typeof d.job_id === "number" ? d.job_id : null,
    jobName: str(d.job_name),
    message: summarize(e.data),
    data: e.data,
  };
}

let liveSeq = 0;
function fromLive(type: string, raw: string): EventRow {
  const e = JSON.parse(raw) as Obj;
  const d = (e.data && typeof e.data === "object" ? e.data : {}) as Obj;
  return {
    key: `l${++liveSeq}`,
    source: "live",
    timestamp: typeof e.timestamp === "number" ? e.timestamp : Date.now(),
    severity: str(e.severity) ?? "info",
    type: str(e.event_type) ?? type,
    jobId: typeof d.job_id === "number" ? d.job_id : null,
    jobName: str(d.job_name),
    message: summarize(e.data ?? e),
    data: e.data ?? e,
  };
}

function readEnabled(): boolean {
  try {
    return localStorage.getItem(ENABLED_KEY) === "1";
  } catch {
    return false;
  }
}

class EventWatcher {
  /** User opt-in for the live SSE stream. */
  enabled = $state(readEnabled());
  live = $state.raw<EventRow[]>([]);
  stored = $state.raw<EventRow[]>([]);
  status = $state<StreamStatus>("off");
  /** True once the first stored-events fetch for the current workflow finished. */
  loaded = $state(false);
  error = $state<string | null>(null);
  rows = $derived(
    [...this.live, ...this.stored].sort((a, b) => b.timestamp - a.timestamp).slice(0, MAX),
  );
}

export const watcher = new EventWatcher();

function bump(n: number) {
  if (n > 0 && app.tab !== "events") app.eventBadge += n;
}

let maxId = 0;
let fetchSeq = 0;

/** Fetch stored events newer than `maxId` (all of the newest MAX on first load). */
async function poll(workflowId: number, initial: boolean) {
  const my = ++fetchSeq;
  const fresh: TorcEvent[] = [];
  try {
    for (let offset = 0; fresh.length < MAX; offset += PAGE) {
      const page = await unwrap(
        api.GET("/events", {
          params: {
            query: {
              workflow_id: workflowId,
              sort_by: "id",
              reverse_sort: true,
              offset,
              limit: PAGE,
            },
          },
        }),
      );
      const newer = page.items.filter((e) => (e.id ?? 0) > maxId);
      fresh.push(...newer);
      if (!page.has_more || newer.length < page.items.length) break;
    }
    if (my !== fetchSeq) return;
    watcher.error = null;
    if (fresh.length) {
      maxId = Math.max(maxId, ...fresh.map((e) => e.id ?? 0));
      watcher.stored = [...fresh.map(fromStored), ...watcher.stored].slice(0, MAX);
      if (!initial) bump(fresh.length);
    }
  } catch (e) {
    if (my === fetchSeq) watcher.error = e instanceof Error ? e.message : String(e);
  } finally {
    if (my === fetchSeq) watcher.loaded = true;
  }
}

/** Hide everything currently shown (the legacy "Clear" button). */
export function clearEvents() {
  watcher.live = [];
  watcher.stored = [];
}

let pageVisible = $state(typeof document === "undefined" || document.visibilityState === "visible");
if (typeof document !== "undefined") {
  document.addEventListener("visibilitychange", () => {
    pageVisible = document.visibilityState === "visible";
  });
}

$effect.root(() => {
  $effect(() => {
    const on = watcher.enabled;
    try {
      localStorage.setItem(ENABLED_KEY, on ? "1" : "0");
    } catch {
      // Storage unavailable (private mode): the choice lasts for this page only.
    }
  });

  // Per-workflow: reset and load stored history.
  $effect(() => {
    const wf = app.workflowId;
    watcher.live = [];
    watcher.stored = [];
    watcher.loaded = false;
    watcher.error = null;
    app.eventBadge = 0;
    maxId = 0;
    if (wf !== null) void poll(wf, true);
  });

  // Live SSE stream: only when opted in, a workflow is selected, and the page is visible.
  $effect(() => {
    const wf = app.workflowId;
    if (wf === null || !watcher.enabled || !pageVisible) {
      watcher.status = "off";
      return;
    }
    const es = new EventSource(withToken(`${API_BASE}/workflows/${wf}/events/stream`));
    watcher.status = "connecting";
    es.onopen = () => (watcher.status = "live");
    // EventSource reconnects on its own unless the server refused the stream outright.
    es.onerror = () =>
      (watcher.status = es.readyState === EventSource.CLOSED ? "off" : "reconnecting");
    // Batch events per animation frame so a burst (thousands of job completions) costs one
    // update and re-render per frame instead of one per event.
    let pending: EventRow[] = [];
    let frame = 0;
    const flush = () => {
      frame = 0;
      if (!pending.length) return;
      watcher.live = [...pending.reverse(), ...watcher.live].slice(0, MAX);
      bump(pending.length);
      pending = [];
    };
    const onEvent = (ev: MessageEvent<string>) => {
      try {
        pending.push(fromLive(ev.type, ev.data));
      } catch {
        return; // Ignore malformed frames.
      }
      frame ||= requestAnimationFrame(flush);
    };
    es.onmessage = onEvent;
    for (const t of SSE_TYPES) es.addEventListener(t, onEvent);
    return () => {
      // Drop the unflushed batch: on a workflow switch it belongs to the old workflow.
      es.close();
      cancelAnimationFrame(frame);
    };
  });

  // Incremental refresh of stored events on every app refresh tick.
  $effect(() => {
    if (app.refreshTick === 0) return;
    const wf = app.workflowId;
    if (wf !== null && untrack(() => watcher.loaded)) void poll(wf, false);
  });

  // Viewing the Events tab marks everything as seen.
  $effect(() => {
    if (app.tab === "events" && app.eventBadge > 0) app.eventBadge = 0;
  });
});
