<script lang="ts">
  import { dismiss, toasts } from "./toast.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast {t.kind}" role="status">
      <span class="msg">{t.message}</span>
      <button class="btn btn-ghost btn-sm btn-icon" aria-label="Dismiss" onclick={() => dismiss(t.id)}>
        ✕
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 16px;
    bottom: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 1000;
    max-width: min(440px, calc(100vw - 32px));
  }
  .toast {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 8px 10px 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 4px solid var(--info);
    border-radius: var(--radius);
    box-shadow: var(--shadow-md);
    animation: slide 0.16s ease-out;
  }
  .success {
    border-left-color: var(--success);
  }
  .error {
    border-left-color: var(--danger);
  }
  .msg {
    flex: 1;
    white-space: pre-wrap;
    word-break: break-word;
    padding-top: 2px;
  }
  @keyframes slide {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
</style>
