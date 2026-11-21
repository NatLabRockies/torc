/**
 * Desktop notifications when a followed workflow finishes. Runs from module import (App imports
 * it). Followed: the selected workflow, plus any started with Run/Watch or submitted from here.
 *
 * Uses the Web Notification API; Torc Desktop polyfills it (tauri-plugin-notification). Notifies
 * only while the dashboard is hidden or unfocused, since otherwise the page itself shows the
 * change. Checks on every refresh tick, and on its own timer while hidden (auto-refresh pauses).
 */
import { api, unwrap } from "../../lib/api/client";
import { app, save } from "../../lib/state.svelte";
import { execution } from "../workflows/execution.svelte";

const KEY = "torc-dash.notify";

function readEnabled(): boolean {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

class Notify {
  enabled = $state(readEnabled());
  /** Browser permission; "unsupported" if there is no Notification API. */
  permission = $state<NotificationPermission | "unsupported">(
    typeof Notification === "undefined" ? "unsupported" : Notification.permission,
  );

  /** Call from a user gesture (browsers require one to ask for permission). */
  async setEnabled(on: boolean) {
    this.enabled = on;
    save(KEY, on ? "1" : null);
    if (on && this.permission === "default") {
      this.permission = await Notification.requestPermission();
    }
  }
}

export const notify = new Notify();

const followed = new Set<number>();
/** Last seen is_complete per workflow; a false → true change notifies. */
const complete = new Map<number, boolean>();

export function follow(workflowId: number) {
  followed.add(workflowId);
}

let checking = false;

async function check() {
  if (!notify.enabled || checking) return;
  checking = true;
  try {
    await checkAll();
  } finally {
    checking = false;
  }
}

async function checkAll() {
  const ids = new Set(followed);
  if (app.workflowId !== null) ids.add(app.workflowId);
  for (const id of ids) {
    let s;
    try {
      s = await unwrap(api.GET("/workflows/{id}/status", { params: { path: { id } } }));
    } catch {
      continue; // Deleted or unreachable; try again next time.
    }
    const was = complete.get(id);
    complete.set(id, s.is_complete);
    if (was !== false || !s.is_complete) continue;
    const c = s.jobs_by_status;
    const bad = c.failed + c.terminated + c.canceled;
    const title = `Workflow ${id} ${bad ? "finished with failures" : "completed"}`;
    const body = bad
      ? `${s.workflow_name}: ${c.failed} failed, ${c.terminated} terminated, ${c.canceled} canceled`
      : `${s.workflow_name}: ${c.completed} of ${s.total_jobs} jobs completed`;
    if (document.hidden || !document.hasFocus()) show(title, body);
  }
}

function show(title: string, body: string) {
  try {
    // Re-read: Torc Desktop's polyfill reports its permission asynchronously.
    if (typeof Notification !== "undefined" && Notification.permission === "granted") {
      new Notification(title, { body });
    }
  } catch {
    // Some browsers (Android Chrome) only allow notifications from a service worker.
  }
}

$effect.root(() => {
  $effect(() => {
    const id = execution.workflowId;
    if (id !== null) follow(id);
  });

  $effect(() => {
    void app.refreshTick;
    void check();
  });

  // Auto-refresh is paused while hidden; keep checking at its interval (30 s if it is off).
  $effect(() => {
    const ms = (app.refreshSeconds || 30) * 1000;
    const id = setInterval(() => {
      if (document.hidden) void check();
    }, ms);
    return () => clearInterval(id);
  });
});
