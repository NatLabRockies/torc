<!-- Follows one log file: polls /api/cli/tail-file and appends what the job writes. -->
<script lang="ts">
  import { tick } from "svelte";
  import { dashPost } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import Icon from "../../lib/ui/Icon.svelte";
  import type { LiveFile, TailFileResponse } from "./logs";

  let {
    title,
    file,
    paused = false,
    clearTick = 0,
  }: { title: string; file: LiveFile; paused?: boolean; clearTick?: number } = $props();

  const POLL_MS = 1000;
  // ponytail: whole-string <pre> re-render; cap keeps it cheap. Virtualize lines if panes lag.
  const MAX_CHARS = 512 * 1024;

  let text = $state("");
  let note = $state<string | null>("Loading…");
  let isError = $state(false);
  let localPause = $state(false);
  let pre = $state<HTMLPreElement>();
  let active = $derived(!paused && !localPause);

  // "Clear all" from the parent; keeps the offset so only new output shows.
  $effect(() => {
    void clearTick;
    text = "";
  });

  /** Where the next poll resumes; kept across pauses. */
  let offset: number | null = null;
  let tailedPath = "";

  $effect(() => {
    const path = file.path;
    if (!active) return;
    // Start over when the path changes (the job's next attempt writes a new file).
    if (path !== tailedPath) {
      [tailedPath, offset, text, note, isError] = [path, null, "", "Loading…", false];
    }
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;

    async function poll() {
      // Skip the request while the window is hidden, like App's auto-refresh.
      if (document.visibilityState !== "visible") {
        timer = setTimeout(poll, POLL_MS);
        return;
      }
      try {
        const r = await dashPost<TailFileResponse>("/api/cli/tail-file", { path, offset });
        if (stopped) return;
        // An error echoes back a placeholder offset; keep ours so a fresh tail still starts
        // near the end once the file becomes readable.
        if (!r.error) offset = r.offset;
        isError = !!r.error;
        if (r.error) note = r.error;
        else if (!r.exists) note = text ? "File no longer exists" : "Waiting for file…";
        else note = null;
        if (r.content) await append(r.content);
      } catch (e) {
        if (stopped) return;
        [note, isError] = [errorText(e), true];
      }
      if (!stopped) timer = setTimeout(poll, POLL_MS);
    }
    poll();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  });

  async function append(chunk: string) {
    const atBottom = !pre || pre.scrollHeight - pre.scrollTop - pre.clientHeight < 24;
    text += chunk;
    if (text.length > MAX_CHARS) {
      const cut = text.indexOf("\n", text.length - MAX_CHARS);
      text = text.slice(cut < 0 ? text.length - MAX_CHARS : cut + 1);
    }
    await tick();
    if (atBottom && pre) pre.scrollTop = pre.scrollHeight;
  }
</script>

<div class="pane card">
  <div class="head">
    <strong class="title">{title}</strong>
    <span class="tag" class:err={file.stream === "stderr"}>{file.stream}</span>
    {#if !active}<span class="faint">paused</span>{/if}
    <div class="grow"></div>
    <button
      class="btn btn-sm btn-icon"
      title={localPause ? "Resume" : "Pause"}
      disabled={paused}
      onclick={() => (localPause = !localPause)}
    >
      <Icon name={localPause ? "play" : "stop"} size={14} />
    </button>
    <button class="btn btn-sm btn-icon" title="Clear" onclick={() => (text = "")}>
      <Icon name="trash" size={14} />
    </button>
  </div>
  <code class="path" title={file.path}>{file.path}</code>
  {#if note}<div class="note" class:error-text={isError}>{note}</div>{/if}
  <pre bind:this={pre} class="code-block out" class:stderr={file.stream === "stderr"}>{text}</pre>
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    min-height: 0;
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--neutral-soft);
  }
  .tag.err {
    color: var(--danger);
  }
  .path {
    color: var(--text-muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .note {
    font-size: 12px;
    color: var(--text-muted);
  }
  .out {
    flex: 1;
    height: 280px;
    min-height: 120px;
    margin: 0;
    white-space: pre;
    overflow: auto;
    resize: vertical;
  }
  .stderr {
    color: var(--danger);
  }
</style>
