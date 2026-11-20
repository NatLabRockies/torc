<!-- Slurm allocation stdout/stderr: {output_dir}/slurm_output_wf{wf}_sl{slurm_job_id}.o/.e -->
<script lang="ts">
  import Modal from "../../lib/ui/Modal.svelte";
  import { loadOutputDir, resolveLogBaseDir, saveOutputDir } from "../debugging/logs";
  import LogPane from "./LogPane.svelte";

  let {
    schedulerId = $bindable(),
    workflowId,
    submissionDir,
  }: {
    schedulerId: number | null;
    workflowId: number;
    submissionDir: string | null | undefined;
  } = $props();

  let open = $state(false);
  $effect(() => {
    open = schedulerId !== null;
  });

  // Applied on change (Enter/blur), not per keystroke, so typing does not fire reads.
  const initialDir = loadOutputDir();
  let dirInput = $state(initialDir);
  let outputDir = $state(initialDir);
  let base = $derived(
    `${resolveLogBaseDir(outputDir || "torc_output", submissionDir)}/slurm_output_wf${workflowId}_sl${schedulerId}`,
  );
</script>

<Modal bind:open title="Slurm job {schedulerId} logs" size="xl" onclose={() => (schedulerId = null)}>
  <div class="body">
    <label class="field">
      Output directory
      <input
        class="input"
        bind:value={dirInput}
        placeholder="torc_output"
        onchange={() => saveOutputDir((outputDir = dirInput))}
      />
    </label>
    <LogPane stdout={[`${base}.o`]} stderr={[`${base}.e`]} />
  </div>
</Modal>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    height: 100%;
  }
  .field {
    max-width: 420px;
  }
</style>
