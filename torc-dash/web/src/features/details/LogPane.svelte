<!--
  stdout/stderr viewer. Each stream takes candidate paths; the first that exists is shown
  (job logs fall back from .o/.e to the combined .log). Reloads when the paths change.
-->
<script lang="ts">
  import { untrack } from "svelte";
  import { errorText } from "../../lib/format";
  import Icon from "../../lib/ui/Icon.svelte";
  import LogSearch from "../../lib/ui/LogSearch.svelte";
  import SegmentedTabs from "../../lib/ui/SegmentedTabs.svelte";
  import { readFirst } from "./files";

  let { stdout, stderr }: { stdout: string[]; stderr: string[] } = $props();

  let stream = $state<"stdout" | "stderr">("stdout");
  let path = $state("");
  let text = $state("");
  let note = $state<string | null>("Loading…");
  let reloadTick = $state(0);
  let pre = $state<HTMLPreElement>();

  $effect(() => {
    const paths = stream === "stdout" ? stdout : stderr;
    void reloadTick;
    let stale = false;
    // Keep the current text visible on reload; only show "Loading" when there is nothing yet.
    if (!untrack(() => text)) note = "Loading…";
    readFirst(paths)
      .then(({ path: p, res }) => {
        if (stale) return;
        path = p;
        text = "";
        if (!res.exists) note = "File does not exist";
        else if (!res.success) note = `Error: ${res.error ?? "unknown error"}`;
        else if (!res.content?.trim()) note = "(empty)";
        else {
          text = res.content;
          note = null;
        }
      })
      .catch((e) => {
        if (!stale) note = `Error loading file: ${errorText(e)}`;
      });
    return () => (stale = true);
  });
</script>

<div class="logs">
  <div class="toolbar">
    <SegmentedTabs
      bind:value={stream}
      tabs={[
        { id: "stdout", label: "stdout" },
        { id: "stderr", label: "stderr" },
      ]}
    />
    <code class="path grow" title={path}>{path}</code>
    <LogSearch target={pre} {text} />
    <button class="btn btn-sm btn-icon" title="Reload" onclick={() => reloadTick++}>
      <Icon name="refresh" size={14} />
    </button>
  </div>
  {#if note}
    <div class="empty">{note}</div>
  {:else}
    <pre bind:this={pre} class="code-block out" class:stderr={stream === "stderr"}>{text}</pre>
  {/if}
</div>

<style>
  .logs {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    flex: 1;
  }
  .path {
    color: var(--text-muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .out {
    flex: 1;
    min-height: 240px;
    max-height: 60vh;
    white-space: pre;
  }
  .stderr {
    color: var(--danger);
  }
</style>
