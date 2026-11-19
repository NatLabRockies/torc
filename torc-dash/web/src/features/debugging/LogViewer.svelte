<!-- stdout/stderr viewer for one job result. Reloads on refresh, sticks to the bottom. -->
<script lang="ts">
  import { tick, untrack } from "svelte";
  import { app } from "../../lib/state.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { errorText } from "../../lib/format";
  import SegmentedTabs from "../../lib/ui/SegmentedTabs.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import LogSearch from "../../lib/ui/LogSearch.svelte";
  import { readFirst } from "../details/files";

  let { stdout, stderr }: { stdout: string; stderr: string } = $props();

  let stream = $state<"stdout" | "stderr">("stdout");
  let path = $derived(stream === "stdout" ? stdout : stderr);

  /** Text for `loadedPath`; kept while the same file refreshes so nothing flashes. */
  let content = $state<string | null>(null);
  let note = $state<string | null>(null);
  let isError = $state(false);
  let loadedPath = $state<string | null>(null);
  /** The file actually shown: `path`, or the combined `.log` it fell back to. */
  let shownPath = $state<string | null>(null);
  let displayPath = $derived(loadedPath === path && shownPath ? shownPath : path);
  let pre = $state<HTMLPreElement>();
  /** Bumped per load; only the latest load may show its result. */
  let loadSeq = 0;
  /** Path being loaded. A refresh of it is skipped so slow reads still finish and show. */
  let inflight: string | null = null;

  $effect(() => {
    const p = path;
    void app.refreshTick;
    untrack(() => load(p));
  });

  async function load(p: string) {
    if (p === inflight) return;
    inflight = p;
    const seq = ++loadSeq;
    const samePath = p === loadedPath;
    const atBottom = !pre || pre.scrollHeight - pre.scrollTop - pre.clientHeight < 24;
    if (!samePath) {
      content = null;
      note = null;
    }
    let next: string | null = null;
    let nextNote: string | null = null;
    let err = false;
    let shown = p;
    try {
      // With stdio mode "combined", the runner writes one .log instead of .o and .e.
      const { path: found, res: r } = await readFirst([p, p.replace(/\.[oe]$/, ".log")]);
      shown = r.exists ? found : p;
      if (!r.exists) nextNote = "File does not exist";
      else if (!r.success) [nextNote, err] = [r.error || "Unknown error", true];
      else if (!r.content?.trim()) nextNote = "(empty)";
      else next = r.content;
    } catch (e) {
      [nextNote, err] = [`Error loading file: ${errorText(e)}`, true];
    }
    if (seq !== loadSeq) return; // a newer load of another file won
    inflight = null;
    content = next;
    note = nextNote;
    isError = err;
    loadedPath = p;
    shownPath = shown;
    if (!samePath || atBottom) {
      await tick();
      if (pre) pre.scrollTop = pre.scrollHeight;
    }
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(content ?? "");
      toast.success("Copied to clipboard");
    } catch (e) {
      toast.error(e);
    }
  }
</script>

<div class="viewer">
  <div class="bar">
    <SegmentedTabs
      bind:value={stream}
      tabs={[
        { id: "stdout", label: "stdout" },
        { id: "stderr", label: "stderr" },
      ]}
    />
    <code class="path" title={displayPath}>{displayPath}</code>
    <LogSearch target={pre} text={content ?? ""} />
    <button class="btn btn-sm" onclick={copy} disabled={!content} title="Copy log contents">
      Copy
    </button>
    <button
      class="btn btn-sm btn-ghost btn-icon"
      onclick={() => app.openFile(displayPath, stream)}
      title="Open in file viewer"
    >
      <Icon name="external" size={13} />
    </button>
  </div>
  {#if content !== null}
    <pre bind:this={pre} class="log" class:stderr={stream === "stderr"}>{content}</pre>
  {:else if note !== null}
    <div class="empty" class:error-text={isError}>{note}</div>
  {:else}
    <div class="empty"><span class="spinner"></span></div>
  {/if}
</div>

<style>
  .viewer {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    height: 100%;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-muted);
    font-size: 12px;
  }
  .log {
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: 12px;
    overflow: auto;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-family: var(--mono);
    font-size: 12.5px;
    line-height: 1.5;
    white-space: pre;
    tab-size: 4;
  }
  .log.stderr {
    border-left: 3px solid var(--danger);
  }
  .empty {
    flex: 1;
    border: 1px dashed var(--border);
    border-radius: var(--radius);
  }
</style>
