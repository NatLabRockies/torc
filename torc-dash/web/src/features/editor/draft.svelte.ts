/**
 * The spec being edited, kept across tab switches and reloads (localStorage). Other features open a
 * spec in the editor with `draft.open(text)`.
 */
import { app } from "../../lib/state.svelte";
import { confirm } from "../../lib/ui/confirm.svelte";

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
    this.text = text;
    this.path = path;
    this.saved = path ? text : null;
    app.go("editor");
    return true;
  }
}

export const draft = new Draft();
