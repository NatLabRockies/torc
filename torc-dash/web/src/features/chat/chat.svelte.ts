/**
 * Chat session state. Module-level so the conversation (and an in-flight stream) survives tab
 * switches. Talks to POST /api/chat, which streams SSE events from torc-dash:
 *   text        data = JSON-encoded string (one per model round)
 *   tool_use    data = {"id","name","input"}
 *   tool_result data = {"id","name","result","is_error"}
 *   spec_edit   data = {"old_str","new_str"} or {"content"} (editor conversations only)
 *   error       data = plain text
 *   done        data = ""
 */
import { app } from "../../lib/state.svelte";
import { errorText } from "../../lib/format";

export type Part =
  | { kind: "text"; text: string }
  | { kind: "tool"; id: string; name: string; input: unknown; result?: string; isError?: boolean }
  | { kind: "note"; text: string; error: boolean };

export type Message = { role: "user"; text: string } | { role: "assistant"; parts: Part[] };

/** An edit the assistant made to the spec editor's buffer. */
export type SpecEdit = { old_str: string; new_str: string } | { content: string };
/** Connects a conversation to the spec editor: its text goes with each message, edits come back. */
export type SpecBinding = { get: () => string; apply: (edit: SpecEdit) => boolean };

export class ChatState {
  /** Send the selected workflow as context (the AI Chat tab, not the editor). */
  readonly workflowContext: boolean;
  /** Set by the spec editor for its conversation. */
  spec: SpecBinding | null = null;

  messages = $state<Message[]>([]);
  streaming = $state(false);
  /** null until the first status check. */
  available = $state<boolean | null>(null);
  reason = $state<string | null>(null);
  /** Show the provider form even when a provider is configured. */
  setupOpen = $state(false);
  /** Conversation sent to the server: plain text turns, as the legacy UI did. */
  private history: { role: "user" | "assistant"; content: string }[] = [];
  private abort: AbortController | null = null;

  constructor(workflowContext: boolean) {
    this.workflowContext = workflowContext;
  }

  async checkStatus() {
    try {
      const r = await fetch("/api/chat/status");
      const data = (await r.json()) as { available: boolean; reason?: string };
      this.available = data.available;
      this.reason = data.reason ?? null;
    } catch {
      // Leave the previous state; the send path reports real failures.
    }
  }

  clear() {
    this.messages = [];
    this.history = [];
  }

  stop() {
    this.abort?.abort();
  }

  async send(text: string) {
    if (!text || this.streaming) return;
    this.messages.push({ role: "user", text });
    this.history.push({ role: "user", content: text });
    this.messages.push({ role: "assistant", parts: [] });
    // Mutate through the proxied array element so updates are reactive.
    const msg = this.messages[this.messages.length - 1] as Extract<Message, { role: "assistant" }>;
    const note = (t: string, error: boolean) => msg.parts.push({ kind: "note", text: t, error });

    this.streaming = true;
    this.abort = new AbortController();
    let fullText = "";
    try {
      const response = await fetch("/api/chat", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          messages: this.history,
          workflow_id: this.workflowContext ? app.workflowId : null,
          spec: this.spec?.get(),
        }),
        signal: this.abort.signal,
      });
      if (!response.ok || !response.body) {
        const body = await response.text();
        if (response.status === 503) this.available = false;
        note(`Error: ${body || `HTTP ${response.status}`}`, true);
        return;
      }
      const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
      let buffer = "";
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += value.replace(/\r\n/g, "\n");
        const blocks = buffer.split("\n\n");
        buffer = blocks.pop() ?? "";
        for (const block of blocks) {
          const ev = parseEvent(block);
          if (!ev) continue;
          if (ev.type === "text") {
            const t = parseJson<string>(ev.data) ?? ev.data;
            fullText += t;
            const last = msg.parts[msg.parts.length - 1];
            if (last?.kind === "text") last.text += t;
            else msg.parts.push({ kind: "text", text: t });
          } else if (ev.type === "tool_use") {
            const t = parseJson<{ id: string; name: string; input: unknown }>(ev.data);
            if (t) msg.parts.push({ kind: "tool", id: t.id, name: t.name, input: t.input });
          } else if (ev.type === "tool_result") {
            const r = parseJson<{ id: string; result: string; is_error: boolean }>(ev.data);
            const part = msg.parts.find((p) => p.kind === "tool" && p.id === r?.id);
            if (r && part?.kind === "tool") {
              part.result = r.result ?? "";
              part.isError = r.is_error;
            }
          } else if (ev.type === "spec_edit") {
            const edit = parseJson<SpecEdit>(ev.data);
            if (edit && !this.spec?.apply(edit)) note("Could not apply an edit: the spec changed.", true);
          } else if (ev.type === "error") {
            note(`Error: ${ev.data}`, true);
          }
        }
      }
    } catch (e) {
      if (e instanceof DOMException && e.name === "AbortError") note("Stopped.", false);
      else note(`Error: ${errorText(e)}`, true);
    } finally {
      if (fullText) this.history.push({ role: "assistant", content: fullText });
      else this.history.pop(); // keep user/assistant alternation for the next request
      this.streaming = false;
      this.abort = null;
    }
  }

  async configure(body: Record<string, string>) {
    const r = await fetch("/api/chat/configure", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!r.ok) throw new Error((await r.text()) || "Failed to configure provider");
    // The provider is torc-dash-wide, so every conversation can use it now.
    for (const s of [chat, specChat]) {
      s.available = true;
      s.reason = null;
    }
    this.setupOpen = false;
  }
}

function parseEvent(block: string): { type: string; data: string } | null {
  let type = "message";
  const data: string[] = [];
  for (const line of block.split("\n")) {
    if (line.startsWith("event:")) type = line.slice(6).trim();
    else if (line.startsWith("data:")) data.push(line.slice(line.startsWith("data: ") ? 6 : 5));
  }
  return data.length ? { type: type === "message" ? "text" : type, data: data.join("\n") } : null;
}

function parseJson<T>(s: string): T | null {
  try {
    return JSON.parse(s) as T;
  } catch {
    return null;
  }
}

/** The AI Chat tab's conversation. */
export const chat = new ChatState(true);
/** The spec editor's conversation, about the spec being edited. */
export const specChat = new ChatState(false);
