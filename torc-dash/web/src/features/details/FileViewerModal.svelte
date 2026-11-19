<!-- File viewer. Opens when app.fileViewer is set (app.openFile(path, title)). -->
<script lang="ts">
  import { errorText } from "../../lib/format";
  import { highlightByName } from "../../lib/highlight";
  import { app } from "../../lib/state.svelte";
  import LogSearch from "../../lib/ui/LogSearch.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { readFile, type ReadFileResponse } from "./files";
  import JsonView from "./JsonView.svelte";

  let open = $state(false);
  let res = $state<ReadFileResponse | null>(null);
  let error = $state<string | null>(null);
  let pre = $state<HTMLPreElement>();

  $effect(() => {
    const target = app.fileViewer;
    open = target !== null;
    res = null;
    error = null;
    if (!target) return;
    let stale = false;
    readFile(target.path)
      .then((r) => !stale && (res = r))
      .catch((e) => !stale && (error = errorText(e)));
    return () => (stale = true);
  });

  async function copyPath() {
    if (!app.fileViewer) return;
    try {
      await navigator.clipboard.writeText(app.fileViewer.path);
      toast.success("Path copied");
    } catch (e) {
      toast.error(e);
    }
  }
</script>

<Modal
  bind:open
  title={app.fileViewer?.title || "File contents"}
  size="xl"
  onclose={() => (app.fileViewer = null)}
>
  <div class="viewer">
    <div class="pathbar">
      <code class="grow" title={app.fileViewer?.path}>{app.fileViewer?.path}</code>
      {#if res?.content && !res.is_json}<LogSearch target={pre} text={res.content} />{/if}
      <button class="btn btn-sm" onclick={copyPath}>Copy path</button>
    </div>
    {#if error}
      <div class="empty error-text">Error loading file: {error}</div>
    {:else if !res}
      <div class="empty"><span class="spinner"></span></div>
    {:else if !res.exists}
      <div class="empty">File does not exist</div>
    {:else if !res.success}
      <div class="empty error-text">Error: {res.error ?? "unknown error"}</div>
    {:else if res.is_json}
      <div class="content"><JsonView json={res.content ?? ""} /></div>
    {:else if !res.content}
      <div class="empty">(empty)</div>
    {:else}
      {@const html = highlightByName(res.content, app.fileViewer?.path ?? "")}
      <!-- highlightByName escapes its input. -->
      {#if html !== null}<pre bind:this={pre} class="content plain hl">{@html html}</pre>
      {:else}<pre bind:this={pre} class="content plain">{res.content}</pre>{/if}
    {/if}
  </div>
</Modal>

<style>
  .viewer {
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: 100%;
  }
  .pathbar {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .pathbar code {
    color: var(--text-muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .content {
    flex: 1;
    min-height: 0;
    display: flex;
    margin: 0;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: auto;
  }
  .plain {
    display: block;
    padding: 12px 14px;
    font-size: 12.5px;
    line-height: 1.5;
  }
</style>
