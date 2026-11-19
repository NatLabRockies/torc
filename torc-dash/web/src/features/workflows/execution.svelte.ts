/**
 * Live output of a `torc run` or `torc watch` streamed from /api/cli/run-stream (SSE).
 * Module-level so any component can start one (`execution.start(id, mode)`) and
 * <ExecutionPanel/> shows it.
 */
import { app } from "../../lib/state.svelte";
import { withToken } from "../../lib/api/token";
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

  /** Stop following the stream. A `torc watch` stops with it; a `torc run` keeps running. */
  stop() {
    if (!this.running) return;
    this.#append(
      this.mode === "run"
        ? "⚠ Stopped following output. The run may continue on the server; use Cancel workflow to stop it."
        : "Stopped watching. The workflow itself is not affected.",
      "warning",
    );
    this.#finish("stopped");
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
