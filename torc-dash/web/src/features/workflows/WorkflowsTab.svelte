<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import { api, listAll, unwrap, type Workflow } from "../../lib/api/client";
  import { errorText, formatDateTime, formatRelative } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Badge from "../../lib/ui/Badge.svelte";
  import DataTable, { type Column, type SortState } from "../../lib/ui/DataTable.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { deleteWorkflows } from "./actions";
  import ImportModal from "./ImportModal.svelte";
  import WorkflowActions from "./WorkflowActions.svelte";

  let workflows = $state.raw<Workflow[]>([]);
  let loaded = $state(false);
  let loadError = $state("");
  let filter = $state("");
  let groupInput = $state(app.accessGroup);
  let sort = $state<SortState>({ key: "id", dir: "desc" });
  let selected = $state(new SvelteSet<string | number>());
  let importOpen = $state(false);
  let filterEl = $state<HTMLInputElement>();

  let seq = 0;
  $effect(() => {
    void app.refreshTick;
    const query = app.workflowQuery();
    if (!query) return;
    const mine = ++seq;
    listAll((offset, limit) =>
      unwrap(
        api.GET("/workflows", {
          params: { query: { offset, limit, ...query } },
        }),
      ),
    )
      .then((items) => {
        if (mine !== seq) return;
        workflows = items;
        loadError = "";
        // Drop selections for workflows that no longer exist.
        const ids = new Set(items.map((w) => w.id));
        for (const k of selected) if (!ids.has(k as number)) selected.delete(k);
      })
      .catch((e) => {
        if (mine === seq) loadError = errorText(e);
      })
      .finally(() => {
        if (mine === seq) loaded = true;
      });
  });

  function applyGroup() {
    app.accessGroup = groupInput.trim();
  }

  async function bulkDelete() {
    const deleted = await deleteWorkflows([...selected] as number[]);
    for (const id of deleted) selected.delete(id);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key !== "/" || e.metaKey || e.ctrlKey || e.altKey) return;
    if ((e.target as HTMLElement).closest("input, textarea, select, [contenteditable], dialog[open]"))
      return;
    e.preventDefault();
    filterEl?.focus();
  }

  const columns: Column<Workflow>[] = [
    { key: "id", label: "ID", width: "72px", mono: true },
    { key: "name", label: "Name", cell: nameCell },
    { key: "user", label: "User", width: "12%" },
    { key: "project", label: "Project", width: "12%" },
    { key: "description", label: "Description", width: "28%", cell: descriptionCell },
    { key: "timestamp", label: "Created", width: "130px", cell: createdCell },
    { key: "actions", label: "", width: "124px", sortable: false, value: () => null, cell: actionsCell },
  ];
</script>

<svelte:window onkeydown={onKeydown} />

{#snippet nameCell(w: Workflow)}
  <span class="name">
    <span class="ellipsis" title={w.name}>{w.name || "Unnamed"}</span>
    {#if w.is_canceled}<Badge status="canceled" />{/if}
    {#if w.is_archived}<Badge tone="neutral">archived</Badge>{/if}
  </span>
{/snippet}

{#snippet descriptionCell(w: Workflow)}
  <span class="muted" title={w.description ?? ""}>{w.description || "—"}</span>
{/snippet}

{#snippet createdCell(w: Workflow)}
  <span title={formatDateTime(w.timestamp)}>{formatRelative(w.timestamp)}</span>
{/snippet}

{#snippet actionsCell(w: Workflow)}
  <WorkflowActions workflowId={w.id!} compact />
{/snippet}

<div class="page">
  <div class="page-header">
    <div class="search">
      <Icon name="search" size={14} />
      <input
        bind:this={filterEl}
        class="input"
        type="search"
        placeholder="Filter workflows…  /"
        aria-label="Filter workflows"
        bind:value={filter}
      />
    </div>
    <input
      class="input group"
      type="search"
      placeholder="Access group"
      title="Show workflows shared with this access group (press Enter)"
      aria-label="Access group"
      bind:value={groupInput}
      onchange={applyGroup}
      onkeydown={(e) => e.key === "Enter" && applyGroup()}
    />
    <label class="check" title="Show workflows owned by every user">
      <input
        type="checkbox"
        checked={app.showAllUsers}
        onchange={(e) => app.setShowAllUsers(e.currentTarget.checked)}
      />
      All users
    </label>
    <div class="grow"></div>
    <button class="btn" onclick={() => (importOpen = true)}>
      <Icon name="download" size={14} /> Import
    </button>
    <button class="btn btn-primary" onclick={() => (app.createOpen = true)} title="New workflow (N)">
      <Icon name="plus" size={14} /> Create workflow
    </button>
  </div>

  {#if selected.size > 0}
    <div class="bulk">
      <strong>{selected.size} selected</strong>
      <button class="btn btn-sm btn-danger" onclick={bulkDelete}>
        <Icon name="trash" size={13} /> Delete selected
      </button>
      <button class="btn btn-sm btn-ghost" onclick={() => selected.clear()}>Clear selection</button>
    </div>
  {/if}

  {#if loadError}<p class="error-text">Error loading workflows: {loadError}</p>{/if}

  <div class="table">
    <DataTable
      rows={workflows}
      {columns}
      rowKey={(w) => w.id!}
      {filter}
      bind:sort
      selectable
      bind:selected
      loading={!loaded}
      activeKey={app.workflowId}
      onRowClick={(w) => app.go("details", w.id!)}
      empty={app.accessGroup
        ? `No workflows shared with group "${app.accessGroup}"`
        : "No workflows yet. Create one to get started."}
    />
  </div>
</div>

{#if importOpen}<ImportModal onclose={() => (importOpen = false)} />{/if}

<style>
  .search {
    position: relative;
    display: flex;
    align-items: center;
    color: var(--text-faint);
  }
  .search :global(svg) {
    position: absolute;
    left: 10px;
    pointer-events: none;
  }
  .search .input {
    width: 280px;
    padding-left: 30px;
  }
  .group {
    width: 160px;
  }
  .bulk {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    background: var(--accent-soft);
  }
  .table {
    flex: 1;
    min-height: 0;
  }
  .name {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    max-width: 100%;
    font-weight: 500;
  }
  .ellipsis {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  @media (max-width: 760px) {
    .search .input {
      width: 100%;
    }
  }
</style>
