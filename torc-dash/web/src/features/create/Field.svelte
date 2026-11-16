<!-- Labeled form field with optional hint and inline error. Wrap a single control. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    label,
    error,
    hint,
    required = false,
    wide = false,
    children,
  }: {
    label: string;
    error?: string;
    hint?: string;
    required?: boolean;
    wide?: boolean;
    children: Snippet;
  } = $props();
</script>

<label class="field" class:invalid={!!error} class:wide>
  <span>{label}{#if required}<span class="req" aria-hidden="true"> *</span>{/if}</span>
  {@render children()}
  {#if error}
    <span class="err" role="alert">{error}</span>
  {:else if hint}
    <span class="hint">{hint}</span>
  {/if}
</label>

<style>
  .field {
    min-width: 0;
  }
  .wide {
    grid-column: 1 / -1;
  }
  .req {
    color: var(--danger);
  }
  .err {
    color: var(--danger);
    font-size: 12px;
    font-weight: 400;
  }
  .invalid :global(.input),
  .invalid :global(.select) {
    border-color: var(--danger);
  }
</style>
