<script lang="ts">
  import { formatDateTime, type Tone } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Badge from "../../lib/ui/Badge.svelte";
  import DataTable, { type Column } from "../../lib/ui/DataTable.svelte";
  import { clearEvents, watcher, type EventRow, type StreamStatus } from "./eventWatcher.svelte";

  let filter = $state("");
  let filterInput = $state<HTMLInputElement>();

  const SEVERITY_TONE: Record<string, Tone> = {
    error: "danger",
    warning: "warning",
    info: "info",
    debug: "neutral",
  };

  const STATUS_TEXT: Record<StreamStatus, string> = {
    off: "Paused",
    connecting: "Connecting…",
    live: "Live",
    reconnecting: "Reconnecting…",
  };

  const columns: Column<EventRow>[] = [
    {
      key: "timestamp",
      label: "Time",
      width: "210px",
      value: (r) => r.timestamp,
      cell: timeCell,
    },
    { key: "severity", label: "Level", width: "96px", cell: levelCell },
    { key: "type", label: "Type", width: "200px", mono: true },
    {
      key: "job",
      label: "Job",
      width: "200px",
      value: (r) => (r.jobId === null ? "" : `${r.jobId} ${r.jobName ?? ""}`),
      cell: jobCell,
    },
    { key: "message", label: "Details", cell: detailsCell },
  ];

  function onKey(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    if (e.key !== "/" || e.metaKey || e.ctrlKey || e.altKey) return;
    if (t.closest("input, textarea, select, [contenteditable]")) return;
    e.preventDefault();
    filterInput?.focus();
  }
</script>

<svelte:window onkeydown={onKey} />

{#snippet timeCell(r: EventRow)}
  <span title={new Date(r.timestamp).toISOString()}>{formatDateTime(r.timestamp)}</span>
{/snippet}

{#snippet levelCell(r: EventRow)}
  <Badge tone={SEVERITY_TONE[r.severity.toLowerCase()] ?? "neutral"}>{r.severity}</Badge>
{/snippet}

{#snippet jobCell(r: EventRow)}
  {#if r.jobId !== null}
    <button
      class="link"
      title="Open job {r.jobId}"
      onclick={(e) => {
        e.stopPropagation();
        app.openJob(r.jobId!);
      }}
    >
      <span class="mono">#{r.jobId}</span>
      {#if r.jobName}<span class="job-name">{r.jobName}</span>{/if}
    </button>
  {:else}
    <span class="faint">—</span>
  {/if}
{/snippet}

{#snippet detailsCell(r: EventRow)}
  <span class="details" title={JSON.stringify(r.data, null, 2)}>{r.message}</span>
{/snippet}

<div class="page">
  <div class="page-header">
    <h2>Events</h2>
    {#if app.workflowId !== null}
      <label
        class="stream {watcher.status}"
        title="Stream live events from the server (server-sent events). Paused while this browser tab is hidden."
      >
        <input type="checkbox" bind:checked={watcher.enabled} />
        <span class="dot"></span>{watcher.enabled ? STATUS_TEXT[watcher.status] : "Live updates"}
      </label>
      <input
        class="input filter"
        placeholder="Filter events  /"
        bind:value={filter}
        bind:this={filterInput}
      />
      <button
        class="btn"
        disabled={watcher.rows.length === 0}
        title="Clear the displayed events"
        onclick={clearEvents}>Clear</button
      >
    {/if}
  </div>

  {#if app.workflowId === null}
    <div class="card empty">Pick a workflow in the top bar to see its events.</div>
  {:else}
    {#if watcher.error && watcher.rows.length === 0}
      <div class="error-text">Failed to load events: {watcher.error}</div>
    {/if}
    <div class="card table">
      <DataTable
        rows={watcher.rows}
        rowKey={(r) => r.key}
        {columns}
        {filter}
        loading={!watcher.loaded}
        empty={watcher.status === "live"
          ? "No events yet. Waiting for new events…"
          : "No events yet."}
        onRowClick={(r) => r.jobId !== null && app.openJob(r.jobId)}
      />
    </div>
  {/if}
</div>

<style>
  .filter {
    width: 240px;
  }
  .table {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .stream {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-muted);
    cursor: pointer;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .stream.live {
    color: var(--success);
  }
  .stream.live .dot {
    background: var(--success);
    animation: pulse 1.6s ease-in-out infinite;
  }
  .stream.reconnecting .dot,
  .stream.connecting .dot {
    background: var(--warning);
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .link {
    display: inline-flex;
    gap: 6px;
    min-width: 0;
    max-width: 100%;
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }
  .link:hover .job-name {
    text-decoration: underline;
  }
  .job-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .details {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-muted);
  }
</style>
