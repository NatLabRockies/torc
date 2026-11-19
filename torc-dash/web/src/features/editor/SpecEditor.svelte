<!--
  CodeMirror YAML editor with workflow-schema completion, hover docs, and lint.
    <SpecEditor bind:value {schema} {readOnly} bind:this={ref} />
  ref.setText(text, from, to) replaces and selects; ref.applyEdit(edit) applies an assistant edit.
-->
<script lang="ts">
  import { isolateHistory } from "@codemirror/commands";
  import { Compartment, EditorSelection, EditorState } from "@codemirror/state";
  import { EditorView } from "@codemirror/view";
  import { untrack } from "svelte";
  import { specExtensions } from "./cm";
  import type { SpecEdit } from "../chat/chat.svelte";
  import type { SpecSchema } from "./yamlSchema";

  let {
    value = $bindable(""),
    schema,
    readOnly = false,
  }: { value: string; schema: SpecSchema | null; readOnly?: boolean } = $props();

  let host = $state<HTMLDivElement>();
  let view: EditorView | undefined;
  const lock = new Compartment();
  const locked = (on: boolean) => [EditorState.readOnly.of(on), EditorView.editable.of(!on)];

  $effect(() => {
    const v = new EditorView({
      parent: host,
      doc: untrack(() => value),
      extensions: [
        specExtensions(schema, (text) => (value = text)),
        lock.of(locked(untrack(() => readOnly))),
      ],
    });
    view = v;
    v.focus();
    return () => v.destroy();
  });

  // Text replaced from outside (template, open file): load it into the editor.
  $effect(() => {
    const text = value;
    if (view && text !== view.state.doc.toString())
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
  });

  $effect(() => {
    const on = readOnly;
    view?.dispatch({ effects: lock.reconfigure(locked(on)) });
  });

  /**
   * Apply an assistant edit as its own undo step and select the new text. False if `old_str`
   * isn't in the document exactly once.
   */
  export function applyEdit(edit: SpecEdit): boolean {
    if (!view) return false;
    const doc = view.state.doc.toString();
    let from = 0;
    let to = doc.length;
    let insert: string;
    if ("content" in edit) insert = edit.content;
    else {
      from = doc.indexOf(edit.old_str);
      if (from < 0 || doc.indexOf(edit.old_str, from + edit.old_str.length) >= 0) return false;
      to = from + edit.old_str.length;
      insert = edit.new_str;
    }
    view.dispatch({
      changes: { from, to, insert },
      selection: EditorSelection.range(from, from + insert.length),
      scrollIntoView: true,
      annotations: isolateHistory.of("full"),
      userEvent: "input.assistant",
    });
    return true;
  }

  /** Replace the document and select [from, to] (e.g. an inserted snippet). */
  export function setText(text: string, from: number, to: number) {
    if (!view) return;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: text },
      selection: EditorSelection.range(from, to),
      scrollIntoView: true,
    });
    view.focus();
  }
</script>

<div class="spec-editor" bind:this={host}></div>

<style>
  .spec-editor {
    height: 100%;
    min-height: 0;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .spec-editor :global(.cm-editor.cm-focused) {
    outline: none;
  }
</style>
