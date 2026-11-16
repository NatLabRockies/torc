/**
 * Live output of a `torc run` streamed from /api/cli/run-stream (SSE). Module-level so any
 * component can start a run (`execution.start(id)`) and <ExecutionPanel/> shows it.
 */
import { app } from "../../lib/state.svelte";
import { toast } from "../../lib/ui/toast.svelte";

export type LineKind = "info" | "stdout" | "stderr" | "error" | "success" | "warning";
export type Line = { text: string; kind: LineKind };

// ponytail: keeps the last 5000 lines in the DOM; virtualize the output if longer logs matter.
const MAX_LINES = 5000;

class Execution {
  workflowId = $state<number | null>(null);
  open = $state(false);
  collapsed = $state(false);
  running = $state(false);
  result = $state<"success" | "failed" | "stopped" | null>(null);
  lines = $state.raw<Line[]>([]);

  #source: EventSource | null = null;
  #buffer: Line[] = [];
  #flushScheduled = false;

  start(workflowId: number) {
    this.#detach();
    this.workflowId = workflowId;
    this.lines = [];
    this.#buffer = [];
    this.open = true;
    this.collapsed = false;
    this.running = true;
    this.result = null;
    this.#append(`Starting workflow ${workflowId}…`, "info");

    const es = new EventSource(`/api/cli/run-stream?workflow_id=${workflowId}`);
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
        this.#append("✓ Workflow completed successfully", "success");
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

  /** Stop following the stream. The server-side `torc run` process keeps running. */
  stop() {
    if (!this.running) return;
    this.#append(
      "⚠ Stopped following output. The run may continue on the server; use Cancel workflow to stop it.",
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
