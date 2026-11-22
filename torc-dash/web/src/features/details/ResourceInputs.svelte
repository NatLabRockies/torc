<!-- Inputs for a resource requirement's CPUs, memory, GPUs, nodes, and runtime. -->
<script lang="ts">
  import type { ResourceFields } from "./editing";

  let { fields = $bindable(), disabled = false }: { fields: ResourceFields; disabled?: boolean } =
    $props();
</script>

<div class="grid">
  <label class="field">
    CPUs
    <input class="input" type="number" min="1" bind:value={fields.num_cpus} {disabled} required />
  </label>
  <label class="field">
    Memory
    <input class="input mono" bind:value={fields.memory} {disabled} required />
    <span class="hint">e.g. 512m, 4g</span>
  </label>
  <label class="field">
    GPUs
    <input class="input" type="number" min="0" bind:value={fields.num_gpus} {disabled} required />
  </label>
  <label class="field">
    Nodes
    <input class="input" type="number" min="1" bind:value={fields.num_nodes} {disabled} required />
  </label>
  <label class="field">
    Runtime
    <input class="input mono" bind:value={fields.runtime} {disabled} required />
    <span class="hint">ISO 8601, e.g. PT30M, PT2H, P1DT4H</span>
  </label>
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 12px;
  }
</style>
