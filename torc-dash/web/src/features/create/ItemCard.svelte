<!-- Collapsible card for one wizard job / scheduler / action. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "../../lib/ui/Icon.svelte";

  let {
    index,
    title,
    subtitle,
    open = $bindable(),
    invalid = false,
    onremove,
    children,
  }: {
    index: number;
    title: string;
    subtitle?: string;
    open?: boolean;
    invalid?: boolean;
    onremove: () => void;
    children: Snippet;
  } = $props();
</script>

<details class="card item" class:invalid bind:open>
  <summary>
    <span class="chev"><Icon name="chevronRight" size={14} /></span>
    <span class="idx">{index}</span>
    <span class="title">{title}</span>
    {#if subtitle}<span class="sub">{subtitle}</span>{/if}
    {#if invalid}<span class="flag">Needs attention</span>{/if}
    <button
      type="button"
      class="btn btn-ghost btn-sm btn-icon remove"
      aria-label="Remove {title}"
      title="Remove"
      onclick={(e) => {
        e.preventDefault();
        onremove();
      }}
    >
      <Icon name="trash" size={14} />
    </button>
  </summary>
  <div class="content">{@render children()}</div>
</details>

<style>
  .item {
    box-shadow: none;
    transition: border-color 0.12s;
  }
  .item.invalid {
    border-color: var(--danger);
  }
  summary {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 8px 8px 12px;
    cursor: pointer;
    list-style: none;
    user-select: none;
    border-radius: var(--radius-lg);
  }
  summary::-webkit-details-marker {
    display: none;
  }
  summary:hover {
    background: var(--surface-hover);
  }
  summary:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .chev {
    display: inline-flex;
    color: var(--text-faint);
    transition: transform 0.12s;
  }
  details[open] .chev {
    transform: rotate(90deg);
  }
  .idx {
    display: inline-grid;
    place-items: center;
    min-width: 22px;
    height: 22px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
  }
  .title {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    color: var(--text-muted);
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 1;
  }
  .flag {
    margin-left: auto;
    font-size: 11.5px;
    color: var(--danger);
    background: var(--danger-soft);
    padding: 1px 8px;
    border-radius: 999px;
    white-space: nowrap;
  }
  .remove {
    margin-left: auto;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .flag + .remove {
    margin-left: 0;
  }
  .remove:hover {
    color: var(--danger);
  }
  .content {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px 14px;
    padding: 6px 14px 14px;
    border-top: 1px solid var(--border);
    padding-top: 14px;
  }
</style>
