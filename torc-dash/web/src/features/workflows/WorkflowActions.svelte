<!--
  Actions for one workflow. Used by the Workflows table rows (compact) and the Details tab header.
    <WorkflowActions workflowId={id} compact? />
  compact: Run and Submit icon buttons plus a "more" menu. Full: primary buttons plus a menu.
  Run and Submit are peers (most users submit to Slurm), so they share one style instead of one
  being highlighted, and are set apart from the setup actions.
-->
<script lang="ts">
  import { app } from "../../lib/state.svelte";
  import Icon, { type IconName } from "../../lib/ui/Icon.svelte";
  import {
    cancelWorkflow,
    deleteWorkflows,
    initializeWorkflow,
    reinitializeWorkflow,
    resetWorkflow,
    submitWorkflow,
  } from "./actions";
  import { execution } from "./execution.svelte";
  import ExecutionPlanModal from "./ExecutionPlanModal.svelte";
  import ExportModal from "./ExportModal.svelte";
  import RecoverModal from "./RecoverModal.svelte";
  import SyncStatusModal from "./SyncStatusModal.svelte";

  let { workflowId, compact = false }: { workflowId: number; compact?: boolean } = $props();

  type Dialog = "plan" | "recover" | "sync" | "export";
  type Action = { id: string; label: string; icon?: IconName; danger?: boolean; run: () => void };

  let dialog = $state<Dialog | null>(null);
  let running = $derived(
    execution.running && execution.mode === "run" && execution.workflowId === workflowId,
  );

  const menuId = $props.id();
  let trigger = $state<HTMLButtonElement>();
  let menu = $state<HTMLDivElement>();

  const id = () => workflowId;
  const ACTIONS: Record<string, Action> = {
    run: { id: "run", label: "Run locally", icon: "play", run: () => execution.start(id()) },
    submit: {
      id: "submit",
      label: "Submit to scheduler",
      icon: "upload",
      run: () => submitWorkflow(id()),
    },
    watch: { id: "watch", label: "Watch", run: () => execution.start(id(), "watch") },
    watchRecover: {
      id: "watch-recover",
      label: "Watch with auto-recovery",
      run: () => execution.start(id(), "watch-recover"),
    },
    init: { id: "init", label: "Initialize", run: () => initializeWorkflow(id()) },
    reinit: { id: "reinit", label: "Reinitialize", run: () => reinitializeWorkflow(id()) },
    reset: { id: "reset", label: "Reset status", run: () => resetWorkflow(id()) },
    plan: { id: "plan", label: "Execution plan", run: () => (dialog = "plan") },
    graph: { id: "graph", label: "View graph", icon: "graph", run: () => app.go("dag", id()) },
    recover: { id: "recover", label: "Recover failed jobs…", run: () => (dialog = "recover") },
    sync: { id: "sync", label: "Sync Slurm status…", run: () => (dialog = "sync") },
    export: { id: "export", label: "Export…", icon: "download", run: () => (dialog = "export") },
    cancel: {
      id: "cancel",
      label: "Cancel workflow",
      icon: "stop",
      danger: true,
      run: () => cancelWorkflow(id()),
    },
    delete: {
      id: "delete",
      label: "Delete",
      icon: "trash",
      danger: true,
      run: () => deleteWorkflows([id()]),
    },
  };

  // Groups separated by dividers in the menu.
  let menuGroups = $derived(
    compact
      ? [
          [ACTIONS.init, ACTIONS.reinit, ACTIONS.reset],
          [ACTIONS.watch, ACTIONS.watchRecover],
          [ACTIONS.graph, ACTIONS.plan],
          [ACTIONS.recover, ACTIONS.sync, ACTIONS.export],
          [ACTIONS.cancel, ACTIONS.delete],
        ]
      : [
          [ACTIONS.watch, ACTIONS.watchRecover],
          [ACTIONS.reset],
          [ACTIONS.recover, ACTIONS.sync, ACTIONS.export],
          [ACTIONS.cancel, ACTIONS.delete],
        ],
  );

  /** Anchor the popover (top layer, so table overflow can't clip it) under the trigger. */
  function place() {
    if (!trigger || !menu) return;
    const r = trigger.getBoundingClientRect();
    menu.style.right = `${window.innerWidth - r.right}px`;
    const below = window.innerHeight - r.bottom > 340;
    menu.style.top = below ? `${r.bottom + 4}px` : "auto";
    menu.style.bottom = below ? "auto" : `${window.innerHeight - r.top + 4}px`;
  }

  function choose(a: Action) {
    menu?.hidePopover();
    a.run();
  }
</script>

<!-- Stop clicks (including those inside dialogs/menus rendered below) from reaching table rows. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="actions" class:compact onclick={(e) => e.stopPropagation()}>
  {#if compact}
    <button
      class="btn btn-ghost btn-icon btn-sm run"
      title={running ? "Running…" : "Run locally"}
      aria-label="Run locally"
      disabled={running}
      onclick={ACTIONS.run.run}
    >
      {#if running}<span class="spinner small"></span>{:else}<Icon name="play" size={14} />{/if}
    </button>
    <button
      class="btn btn-ghost btn-icon btn-sm submit"
      title="Submit to scheduler"
      aria-label="Submit to scheduler"
      onclick={ACTIONS.submit.run}
    >
      <Icon name="upload" size={14} />
    </button>
  {:else}
    <button class="btn btn-sm exec" disabled={running} onclick={ACTIONS.run.run}>
      {#if running}<span class="spinner small"></span> Running…{:else}<Icon name="play" size={13} /> Run{/if}
    </button>
    <button class="btn btn-sm exec" onclick={ACTIONS.submit.run}>
      <Icon name="upload" size={13} /> Submit
    </button>
    <span class="sep" aria-hidden="true"></span>
    <button class="btn btn-sm" onclick={ACTIONS.init.run}>Initialize</button>
    <button class="btn btn-sm" onclick={ACTIONS.reinit.run}>Reinitialize</button>
    <button class="btn btn-sm" onclick={ACTIONS.plan.run}>Execution plan</button>
  {/if}
  <button
    bind:this={trigger}
    class="btn btn-sm {compact ? 'btn-ghost btn-icon' : 'btn-icon'}"
    popovertarget={menuId}
    title="More actions"
    aria-label="More actions"
    onclick={place}
  >
    <Icon name="more" size={14} />
  </button>
  <div bind:this={menu} id={menuId} popover="auto" class="menu" role="menu">
    {#each menuGroups as group, gi (gi)}
      {#if gi > 0}<hr />{/if}
      {#each group as a (a.id)}
        <button role="menuitem" class:danger={a.danger} onclick={() => choose(a)}>
          <span class="ico">{#if a.icon}<Icon name={a.icon} size={14} />{/if}</span>
          {a.label}
        </button>
      {/each}
    {/each}
  </div>

  {#if dialog === "plan"}
    <ExecutionPlanModal {workflowId} onclose={() => (dialog = null)} />
  {:else if dialog === "recover"}
    <RecoverModal {workflowId} onclose={() => (dialog = null)} />
  {:else if dialog === "sync"}
    <SyncStatusModal {workflowId} onclose={() => (dialog = null)} />
  {:else if dialog === "export"}
    <ExportModal {workflowId} onclose={() => (dialog = null)} />
  {/if}
</div>

<style>
  .actions {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .actions.compact {
    gap: 0;
    flex-wrap: nowrap;
  }
  .run:not(:disabled) {
    color: var(--success);
  }
  .submit {
    color: var(--accent);
  }
  .exec {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent);
  }
  .exec:hover:not(:disabled) {
    background: var(--accent);
    color: var(--accent-text);
  }
  .sep {
    width: 1px;
    align-self: stretch;
    margin: 2px 4px;
    background: var(--border);
  }
  .spinner.small {
    width: 12px;
    height: 12px;
  }
  .menu {
    position: fixed;
    inset: auto;
    margin: 0;
    min-width: 200px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-lg);
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 30px;
    padding: 0 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: inherit;
    font: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover,
  .menu button:focus-visible {
    background: var(--surface-hover);
    outline: none;
  }
  .menu button.danger {
    color: var(--danger);
  }
  .menu button.danger:hover {
    background: var(--danger-soft);
  }
  .ico {
    display: inline-flex;
    width: 14px;
    color: var(--text-muted);
  }
  .danger .ico {
    color: inherit;
  }
  hr {
    margin: 4px 0;
    border: none;
    border-top: 1px solid var(--border);
  }
</style>
