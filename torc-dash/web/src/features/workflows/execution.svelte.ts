/**
 * Live output of a `torc run` or `torc watch` streamed from /api/cli/run-stream (SSE).
 * Module-level so any component can start one (`execution.start(id, mode)`) and
 * <ExecutionPanel/> shows it.
 */
import { dashPost } from "../../lib/api/dash";
import { withToken } from "../../lib/api/token";
import { errorText } from "../../lib/format";
import { app } from "../../lib/state.svelte";
import { toast } from "../../lib/ui/toast.svelte";

export type LineKind = "info" | "stdout" | "stderr" | "error" | "success" | "warning";
export type Line = { text: string; kind: LineKind };
/** run: `torc run`. watch / watch-recover: `torc watch [--recover]`, which stops on Stop. */
export type ExecutionMode = "run" | "watch" | "watch-recover";

// ponytail: keeps the last 5000 lines in the DOM; virtualize the output if longer logs matter.
const MAX_LINES = 5000;

class Execution {
  workflowId = $state<number | null>(null);
  mode = $state<ExecutionMode>("run");
  open = $state(false);
  collapsed = $state(false);
  running = $state(false);
  result = $state<"success" | "failed" | "stopped" | null>(null);
  lines = $state.raw<Line[]>([]);

  #source: EventSource | null = null;
  /** Set when Stop was pressed during a run, so its exit reads as stopped, not failed. */
  #stopping = false;
  #buffer: Line[] = [];
  #flushScheduled = false;

  start(workflowId: number, mode: ExecutionMode = "run") {
    this.#detach();
    this.workflowId = workflowId;
    this.mode = mode;
    this.lines = [];
    this.#buffer = [];
    this.open = true;
    this.collapsed = false;
    this.running = true;
    this.result = null;
    this.#stopping = false;
    const watching = mode !== "run";
    this.#append(`${watching ? "Watching" : "Starting"} workflow ${workflowId}…`, "info");

    const es = new EventSource(
      withToken(`/api/cli/run-stream?workflow_id=${workflowId}&mode=${mode}`),
    );
    this.#source = es;
    const on = (type: string, fn: (data: string) => void) =>
      es.addEventListener(type, (e) => {
        if (this.#source === es) fn((e as MessageEvent).data ?? "");
      });

    on("start", (d) => this.#append(d, "info"));
    on("stdout", (d) => this.#append(d, "stdout"));
    on("stderr", (d) => this.#append(d, "stderr"));
    on("status", (d) => this.#append(`[Status] ${d}`, "info"));
    on("end", (status) => {
      if (this.#stopping) {
        this.#append("Run stopped.", "warning");
        return this.#finish("stopped");
      }
      if (status === "success") {
        this.#append(
          watching ? "✓ Watch finished: workflow complete" : "✓ Workflow completed successfully",
          "success",
        );
        toast.success(`Workflow ${workflowId} completed successfully`);
      } else {
        this.#append(`✗ Workflow ${status}`, "error");
        toast.error(`Workflow ${workflowId} ${status}`);
      }
      this.#finish(status === "success" ? "success" : "failed");
    });
    // Server-sent "error" events carry data; a bare error is a dropped connection. Close either
    // way on a dropped connection: EventSource would otherwise reconnect and start a second run.
    es.addEventListener("error", (e) => {
      if (this.#source !== es) return;
      const data = (e as MessageEvent).data;
      if (data) return this.#append(`Error: ${data}`, "error");
      this.#append("Connection error", "error");
      this.#finish("failed");
    });
  }

  /**
   * Stop. A `torc run` is sent SIGTERM (its jobs are terminated) and the output keeps streaming
   * until it exits. A `torc watch` ends when the stream closes; the workflow is not affected.
   */
  async stop() {
    if (!this.running) return;
    if (this.mode !== "run") {
      this.#append("Stopped watching. The workflow itself is not affected.", "warning");
      return this.#finish("stopped");
    }
    this.#stopping = true;
    this.#append("Stopping the run and terminating its jobs…", "warning");
    // Ignore the reply if another execution started meanwhile.
    const es = this.#source;
    try {
      const res = await dashPost<{ stopped: boolean }>("/api/cli/stop-run", {
        workflow_id: String(this.workflowId),
      });
      if (this.#source !== es) return;
      if (!res.stopped) {
        this.#append("The run is no longer active.", "warning");
        this.#finish("stopped");
      }
    } catch (e) {
      if (this.#source !== es) return;
      this.#stopping = false;
      this.#append(`Stop failed: ${errorText(e)}`, "error");
    }
  }

  close() {
    this.#detach();
    this.running = false;
    this.open = false;
  }

  #finish(result: "success" | "failed" | "stopped") {
    this.#detach();
    this.running = false;
    this.result = result;
    app.refresh();
  }

  #detach() {
    this.#source?.close();
    this.#source = null;
  }

  #append(text: string, kind: LineKind) {
    this.#buffer.push({ text, kind });
    if (this.#flushScheduled) return;
    this.#flushScheduled = true;
    // Batch bursts of lines into one render.
    setTimeout(() => {
      this.#flushScheduled = false;
      const next = this.lines.concat(this.#buffer);
      this.#buffer = [];
      this.lines = next.length > MAX_LINES ? next.slice(-MAX_LINES) : next;
    }, 50);
  }
}

export const execution = new Execution();
