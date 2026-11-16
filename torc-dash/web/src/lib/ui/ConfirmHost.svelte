<script lang="ts">
  import Modal from "./Modal.svelte";
  import { confirmState } from "./confirm.svelte";

  let open = $state(false);
  let current = $derived(confirmState.current);
  $effect(() => {
    open = current !== null;
  });

  function finish(ok: boolean) {
    current?.resolve(ok);
    confirmState.current = null;
  }
</script>

<Modal bind:open title={current?.title ?? ""} size="sm" onclose={() => finish(false)}>
  {#if current?.message}<p class="msg">{current.message}</p>{/if}
  {#snippet footer()}
    <button class="btn" onclick={() => finish(false)}>Cancel</button>
    <!-- svelte-ignore a11y_autofocus -->
    <button
      class="btn {current?.danger ? 'btn-danger-solid' : 'btn-primary'}"
      autofocus
      onclick={() => finish(true)}
    >
      {current?.confirmLabel ?? "Confirm"}
    </button>
  {/snippet}
</Modal>

<style>
  .msg {
    margin: 0;
    white-space: pre-wrap;
    color: var(--text-muted);
  }
</style>
