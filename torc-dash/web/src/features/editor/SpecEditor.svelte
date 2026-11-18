<!--
  CodeMirror YAML editor with workflow-schema completion, hover docs, and lint.
    <SpecEditor bind:value {schema} bind:this={ref} />  ref.setText(text, from, to) replaces and selects.
-->
<script lang="ts">
  import { EditorSelection } from "@codemirror/state";
  import { EditorView } from "@codemirror/view";
  import { untrack } from "svelte";
  import { specExtensions } from "./cm";
  import type { SpecSchema } from "./yamlSchema";

  let { value = $bindable(""), schema }: { value: string; schema: SpecSchema | null } = $props();

  let host = $state<HTMLDivElement>();
  let view: EditorView | undefined;

  $effect(() => {
    const v = new EditorView({
      parent: host,
      doc: untrack(() => value),
      extensions: specExtensions(schema, (text) => (value = text)),
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
