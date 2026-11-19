<!-- Colored pill. <Badge tone="success">completed</Badge> or <Badge status={job.status} /> -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import type { JobStatus } from "../api/client";
  import { JOB_STATUS_TONE, statusLabel, type Tone } from "../format";

  let {
    tone,
    status,
    children,
  }: { tone?: Tone; status?: JobStatus | null; children?: Snippet } = $props();

  let resolved = $derived<Tone>(tone ?? (status ? JOB_STATUS_TONE[status] : "neutral"));
</script>

<span class="badge {resolved}">
  {#if status && resolved === "accent"}<span class="pulse"></span>{/if}
  {#if children}{@render children()}{:else}{statusLabel(status)}{/if}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 20px;
    padding: 0 8px;
    border-radius: 999px;
    font-size: 11.5px;
    font-weight: 600;
    text-transform: capitalize;
    white-space: nowrap;
    background: var(--neutral-soft);
    color: var(--text-muted);
  }
  .info {
    background: var(--info-soft);
    color: var(--info);
  }
  .accent {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .success {
    background: var(--success-soft);
    color: var(--success);
  }
  .warning {
    background: var(--warning-soft);
    color: var(--warning);
  }
  .danger {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .pulse {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
    animation: pulse 1.4s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }
</style>
