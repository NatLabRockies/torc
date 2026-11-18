/**
 * The spec being edited, kept across tab switches and reloads (localStorage). Other features open a
 * spec in the editor with `draft.open(text)`.
 */
import { app } from "../../lib/state.svelte";
import { confirm } from "../../lib/ui/confirm.svelte";
import { toast } from "../../lib/ui/toast.svelte";
import { specChat, type SpecEdit } from "../chat/chat.svelte";

const KEY = "torc-dash.editor";

export const BLANK = `name: my_workflow
description: What this workflow does
jobs:
  - name: hello
    command: echo hello
`;

function load(): { text?: string; path?: string | null; saved?: string | null } {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}");
  } catch {
    return {};
  }
}

class Draft {
  #initial = load();
  text = $state(this.#initial.text ?? BLANK);
  /** File on the torc-dash machine this text was opened from or saved to. */
  path = $state<string | null>(this.#initial.path ?? null);
  /** Text as last opened from or saved to `path`. */
  saved = $state<string | null>(this.#initial.saved ?? null);
  dirty = $derived(this.path !== null && this.text !== this.saved);

  constructor() {
    $effect.root(() => {
      $effect(() => {
        const state = { text: this.text, path: this.path, saved: this.saved };
        const id = setTimeout(() => {
          try {
            localStorage.setItem(KEY, JSON.stringify(state));
          } catch {
            // Keeping the draft across reloads is a convenience only.
          }
        }, 300);
        return () => clearTimeout(id);
      });
    });
  }

  /**
   * Replace the editor contents and switch to the editor tab, after asking if that would discard
   * edits not saved anywhere. Resolves false if the user said no.
   */
  async open(text: string, path: string | null = null): Promise<boolean> {
    // The assistant's later edits would land on the new text.
    const assistantBusy = () => {
      if (!specChat.streaming) return false;
      toast.info("The spec assistant is still editing. Stop it or wait for it to finish first.");
      return true;
    };
    if (assistantBusy()) return false;
    const unsaved = this.path === null ? this.text.trim() !== "" && this.text !== BLANK : this.dirty;
    if (
      unsaved &&
      this.text !== text &&
      !(await confirm({
        title: "Replace the spec in the editor?",
        message: "Its unsaved changes will be lost.",
        confirmLabel: "Replace",
      }))
    )
      return false;
    if (assistantBusy()) return false; // it may have started while the dialog was open
    this.text = text;
    this.path = path;
    this.saved = path ? text : null;
    app.go("editor");
    return true;
  }
}

export const draft = new Draft();

/** Set by the editor page while it is open, so assistant edits go through CodeMirror. */
export const editorHooks: { apply: ((edit: SpecEdit) => boolean) | null } = { apply: null };

/** Apply an assistant edit to the draft text directly (the editor isn't open). */
function applyToText(edit: SpecEdit): boolean {
  if ("content" in edit) {
    draft.text = edit.content;
    return true;
  }
  const i = draft.text.indexOf(edit.old_str);
  if (i < 0 || draft.text.indexOf(edit.old_str, i + edit.old_str.length) >= 0) return false;
  draft.text = draft.text.slice(0, i) + edit.new_str + draft.text.slice(i + edit.old_str.length);
  return true;
}

// The editor's assistant shares this draft: it reads it with each message and edits it.
specChat.spec = {
  get: () => draft.text,
  apply: (edit) => (editorHooks.apply ?? applyToText)(edit),
};
