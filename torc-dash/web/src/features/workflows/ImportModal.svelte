<!-- `torc workflows import` from a server-side path or an uploaded export file. -->
<script lang="ts">
  import { cli } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import SegmentedTabs from "../../lib/ui/SegmentedTabs.svelte";
  import { toast } from "../../lib/ui/toast.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let open = $state(true);
  let source = $state<"path" | "upload">("path");
  let filePath = $state("");
  let upload = $state<{ name: string; size: number; content: string } | null>(null);
  let dragging = $state(false);
  let name = $state("");
  let skipResults = $state(false);
  let skipEvents = $state(false);
  let busy = $state(false);
  let error = $state("");

  async function readFile(file: File | undefined) {
    if (!file) return;
    try {
      upload = { name: file.name, size: file.size, content: await file.text() };
    } catch {
      toast.error("Error reading file");
    }
  }

  async function importWorkflow(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    const body: Record<string, unknown> = {
      name: name.trim() || undefined,
      skip_results: skipResults,
      skip_events: skipEvents,
    };
    if (source === "path") {
      if (!filePath.trim()) return toast.warning("Please enter a file path");
      body.file_path = filePath.trim();
    } else {
      if (!upload) return toast.warning("Please select a workflow JSON file to upload");
      try {
        JSON.parse(upload.content);
      } catch {
        return toast.error("Invalid JSON file");
      }
      body.content = upload.content;
    }

    busy = true;
    try {
      const res = await cli("import", body);
      let msg = "Workflow imported successfully";
      try {
        const id = JSON.parse(res.stdout).workflow_id;
        if (id) msg = `Workflow imported with ID ${id}`;
      } catch {
        // Non-JSON output; keep the generic message.
      }
      toast.success(msg);
      app.refresh();
      open = false;
    } catch (err) {
      error = errorText(err);
      toast.error("Import failed");
    } finally {
      busy = false;
    }
  }
</script>

<Modal bind:open title="Import workflow" size="md" {onclose}>
  <form id="import-form" class="form" onsubmit={importWorkflow}>
    <p class="muted">Import a previously exported workflow JSON file.</p>
    <SegmentedTabs
      bind:value={source}
      tabs={[
        { id: "path", label: "Server path" },
        { id: "upload", label: "Upload file" },
      ]}
    />
    {#if source === "path"}
      <label class="field">
        Path to exported JSON file on the server
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="input"
          bind:value={filePath}
          placeholder="/path/to/workflow_export.json"
          autofocus
        />
      </label>
    {:else}
      <label
        class="drop"
        class:dragging
        ondragover={(e) => (e.preventDefault(), (dragging = true))}
        ondragleave={() => (dragging = false)}
        ondrop={(e) => {
          e.preventDefault();
          dragging = false;
          readFile(e.dataTransfer?.files[0]);
        }}
      >
        <input
          type="file"
          accept=".json,application/json"
          hidden
          onchange={(e) => readFile(e.currentTarget.files?.[0])}
        />
        <Icon name="upload" size={22} />
        {#if upload}
          <span class="loaded">Loaded {upload.name} ({(upload.size / 1024).toFixed(1)} KB)</span>
          <span class="faint">Drop or click to choose a different file</span>
        {:else}
          <span>Drop an exported workflow JSON file here or click to select</span>
        {/if}
      </label>
    {/if}
    <label class="field">
      Name override (optional)
      <input class="input" bind:value={name} placeholder="Leave empty to use original name" />
    </label>
    <label class="check"><input type="checkbox" bind:checked={skipResults} /> Skip job results</label>
    <label class="check"><input type="checkbox" bind:checked={skipEvents} /> Skip events</label>
    {#if error}<p class="error-text">{error}</p>{/if}
  </form>
  {#snippet footer()}
    <button class="btn" onclick={() => (open = false)}>Cancel</button>
    <button class="btn btn-primary" type="submit" form="import-form" disabled={busy}>
      {busy ? "Importing…" : "Import"}
    </button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  p {
    margin: 0;
  }
  .drop {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 28px 16px;
    border: 2px dashed var(--border-strong);
    border-radius: var(--radius);
    color: var(--text-muted);
    text-align: center;
    cursor: pointer;
    transition:
      border-color 0.12s,
      background 0.12s;
  }
  .drop:hover,
  .drop.dragging {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .loaded {
    color: var(--success);
    font-weight: 500;
  }
</style>
