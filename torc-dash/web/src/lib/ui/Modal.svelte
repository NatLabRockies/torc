<!--
  Native <dialog> modal: focus trapping, Esc to close, and backdrop click all come from the browser.
  <Modal bind:open title="Edit" size="md">body…{#snippet footer()}<button …>{/snippet}</Modal>
-->
<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    open = $bindable(false),
    title,
    size = "md",
    onclose,
    children,
    footer,
  }: {
    open?: boolean;
    title: string;
    size?: "sm" | "md" | "lg" | "xl";
    onclose?: () => void;
    children: Snippet;
    footer?: Snippet;
  } = $props();

  let dialog = $state<HTMLDialogElement>();
  // Close on a backdrop click only if the press also started there: a text selection dragged
  // out of a field ends with a click on the dialog element, which must not close it.
  let pressedBackdrop = false;
  const titleId = $props.id();

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  });

  function handleClose() {
    open = false;
    onclose?.();
  }
</script>

<!-- Focus the dialog itself on open (not the close button); inputs can still use autofocus. -->
<!-- svelte-ignore a11y_autofocus -->
<dialog
  autofocus
  bind:this={dialog}
  class="modal {size}"
  aria-labelledby={titleId}
  onclose={handleClose}
  onmousedown={(e) => (pressedBackdrop = e.target === dialog)}
  onclick={(e) => pressedBackdrop && e.target === dialog && dialog?.close()}
>
  {#if open}
    <header>
      <h3 id={titleId}>{title}</h3>
      <button class="btn btn-ghost btn-icon btn-sm" aria-label="Close" onclick={() => dialog?.close()}>
        ✕
      </button>
    </header>
    <div class="body">{@render children()}</div>
    {#if footer}
      <footer>{@render footer()}</footer>
    {/if}
  {/if}
</dialog>

<style>
  .modal {
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-lg);
    width: calc(100vw - 32px);
    max-height: calc(100vh - 64px);
    display: none;
    flex-direction: column;
  }
  .modal:focus {
    outline: none;
  }
  .modal[open] {
    display: flex;
    animation: pop 0.14s ease-out;
  }
  .sm {
    max-width: 420px;
  }
  .md {
    max-width: 600px;
  }
  .lg {
    max-width: 880px;
  }
  .xl {
    max-width: 1200px;
    height: calc(100vh - 64px);
  }
  .modal::backdrop {
    background: rgb(10 12 16 / 0.45);
    backdrop-filter: blur(2px);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px 14px 20px;
    border-bottom: 1px solid var(--border);
  }
  h3 {
    font-size: 15px;
  }
  .body {
    padding: 20px;
    overflow: auto;
    flex: 1;
    min-height: 0;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 20px;
    border-top: 1px solid var(--border);
    background: var(--surface-2);
    border-radius: 0 0 var(--radius-lg) var(--radius-lg);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.98);
    }
  }
</style>
