<!-- Docked, collapsible output of a local run started anywhere via execution.start(id). -->
<script lang="ts">
  import { app } from "../../lib/state.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { execution } from "./execution.svelte";

  let output = $state<HTMLPreElement>();
  let stick = true;

  // Follow the tail unless the user scrolled up.
  $effect(() => {
    void execution.lines;
    if (output && stick && !execution.collapsed) output.scrollTop = output.scrollHeight;
  });

  const STATE = {
    success: { text: "Completed", cls: "ok" },
    failed: { text: "Failed", cls: "bad" },
    stopped: { text: "Detached", cls: "warn" },
  } as const;
  let outcome = $derived(execution.result ? STATE[execution.result] : null);
</script>

{#if execution.open}
  <section class="panel" class:collapsed={execution.collapsed} aria-label="Execution output">
    <header>
      <button
        class="title"
        onclick={() => (execution.collapsed = !execution.collapsed)}
        aria-expanded={!execution.collapsed}
        title={execution.collapsed ? "Expand" : "Collapse"}
      >
        <Icon name={execution.collapsed ? "chevronRight" : "chevronDown"} size={14} />
        {#if execution.running}
          <span class="spinner small"></span>
        {:else if outcome}
          <span class="dot {outcome.cls}"></span>
        {/if}
        <span>Run · workflow {execution.workflowId}</span>
        <span class="faint">{execution.running ? "running" : (outcome?.text ?? "")}</span>
      </button>
      <div class="grow"></div>
      {#if execution.workflowId !== null}
        <button
          class="btn btn-ghost btn-sm"
          onclick={() => app.go("details", execution.workflowId)}
          title="Open workflow details"
        >
          Details
        </button>
      {/if}
      {#if execution.running}
        <button
          class="btn btn-danger btn-sm"
          onclick={() => execution.stop()}
          title="Stop following the output (the run itself keeps going on the server)"
        >
          <Icon name="stop" size={12} /> Stop
        </button>
      {/if}
      <button
        class="btn btn-ghost btn-icon btn-sm"
        aria-label="Close"
        title="Close"
        onclick={() => execution.close()}
      >
        <Icon name="x" size={14} />
      </button>
    </header>
    {#if !execution.collapsed}
      <pre
        bind:this={output}
        onscroll={(e) => {
          const el = e.currentTarget;
          stick = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
        }}>{#each execution.lines as line, i (i)}<span class={line.kind}>{line.text}
</span>{/each}</pre>
    {/if}
  </section>
{/if}

<style>
  .panel {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 50;
    width: min(760px, calc(100vw - 32px));
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
    animation: rise 0.16s ease-out;
  }
  .panel.collapsed {
    width: min(420px, calc(100vw - 32px));
  }
  header {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
  }
  .collapsed header {
    border-bottom: none;
  }
  .title {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 2px 4px;
    border: none;
    background: none;
    color: var(--text);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .title .faint {
    font-weight: 400;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .dot.ok {
    background: var(--success);
  }
  .dot.bad {
    background: var(--danger);
  }
  .dot.warn {
    background: var(--warning);
  }
  .spinner.small {
    width: 12px;
    height: 12px;
  }
  pre {
    margin: 0;
    height: 280px;
    min-height: 80px;
    max-height: 75vh;
    padding: 10px 12px;
    overflow: auto;
    resize: vertical;
    font-size: 12px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .stderr,
  .warning {
    color: var(--warning);
  }
  .info {
    color: var(--info);
  }
  .error {
    color: var(--danger);
  }
  .success {
    color: var(--success);
    font-weight: 600;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
</style>
