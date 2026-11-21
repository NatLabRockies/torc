<script lang="ts">
  import type { Component } from "svelte";
  import { dashGet } from "./lib/api/dash";
  import { app, TABS, WORKFLOW_TABS, type TabId, type ThemeMode } from "./lib/state.svelte";
  import ConfirmHost from "./lib/ui/ConfirmHost.svelte";
  import Icon, { type IconName } from "./lib/ui/Icon.svelte";
  import Modal from "./lib/ui/Modal.svelte";
  import Toasts from "./lib/ui/Toasts.svelte";
  import { toast } from "./lib/ui/toast.svelte";
  import WorkflowPicker from "./lib/WorkflowPicker.svelte";

  import ChatTab from "./features/chat/ChatTab.svelte";
  import ConfigTab from "./features/config/ConfigTab.svelte";
  import CreateWorkflowModal from "./features/create/CreateWorkflowModal.svelte";
  import DagTab from "./features/dag/DagTab.svelte";
  import DebuggingTab from "./features/debugging/DebuggingTab.svelte";
  import DetailsTab from "./features/details/DetailsTab.svelte";
  import FileViewerModal from "./features/details/FileViewerModal.svelte";
  import JobDetailsModal from "./features/details/JobDetailsModal.svelte";
  import EditorTab from "./features/editor/EditorTab.svelte";
  import EventsTab from "./features/events/EventsTab.svelte";
  import ExecutionPanel from "./features/workflows/ExecutionPanel.svelte";
  // Starts the event stream at load so the sidebar badge updates on any tab.
  import "./features/events/eventWatcher.svelte";
  // Desktop notifications when followed workflows finish.
  import "./features/notify/notify.svelte";
  import ResourcesTab from "./features/resources/ResourcesTab.svelte";
  import WorkflowsTab from "./features/workflows/WorkflowsTab.svelte";

  const VIEWS: Record<TabId, { component: Component; icon: IconName }> = {
    workflows: { component: WorkflowsTab, icon: "layers" },
    editor: { component: EditorTab, icon: "edit" },
    details: { component: DetailsTab, icon: "list" },
    dag: { component: DagTab, icon: "graph" },
    events: { component: EventsTab, icon: "bell" },
    debugging: { component: DebuggingTab, icon: "bug" },
    resources: { component: ResourcesTab, icon: "activity" },
    chat: { component: ChatTab, icon: "chat" },
    config: { component: ConfigTab, icon: "settings" },
  };

  const THEME_ORDER: ThemeMode[] = ["system", "light", "dark"];
  const THEME_ICON: Record<ThemeMode, IconName> = { system: "monitor", light: "sun", dark: "moon" };

  let View = $derived(VIEWS[app.tab].component);
  let currentLabel = $derived(TABS.find((t) => t.id === app.tab)?.label ?? "");

  // Identity and version, once.
  dashGet<{ user: string | null }>("/api/user")
    .then((r) => (app.user = r.user || null))
    .catch(() => {})
    .finally(() => (app.userChecked = true));
  let versionWarning = $state<string | null>(null);
  let serverVersion = $state<string | null>(null);
  dashGet<{
    version: string;
    server_version: string | null;
    version_mismatch: string | null;
    mismatch_severity: "major" | "minor" | "patch" | null;
  }>("/api/version")
    .then((r) => {
      app.version = r.version;
      serverVersion = r.server_version;
      versionWarning = r.version_mismatch;
      // Patch differences only get the sidebar cue; minor ones a passing warning.
      if (!r.version_mismatch) return;
      if (r.mismatch_severity === "major") toast.error(r.version_mismatch);
      else if (r.mismatch_severity === "minor") toast.warning(r.version_mismatch, 10000);
    })
    .catch(() => {});

  // Auto-refresh, paused while the window is hidden.
  $effect(() => {
    const seconds = app.refreshSeconds;
    if (seconds <= 0) return;
    const id = setInterval(() => {
      if (document.visibilityState === "visible") app.refresh(true);
    }, seconds * 1000);
    return () => clearInterval(id);
  });

  // Health check on every refresh.
  $effect(() => {
    void app.refreshTick;
    app.checkConnection();
  });

  function cycleTheme() {
    app.setTheme(THEME_ORDER[(THEME_ORDER.indexOf(app.theme) + 1) % THEME_ORDER.length]);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey || e.defaultPrevented) return;
    const t = e.target as HTMLElement;
    // Esc leaves a text field so the single-key shortcuts work again (dialogs handle their own).
    if (e.key === "Escape" && t.matches("input, textarea, select") && !t.closest("dialog[open]")) {
      t.blur();
      return;
    }
    if (t.closest("input, textarea, select, [contenteditable], dialog[open]")) return;
    const tab = TABS.find((x) => x.key === e.key);
    if (tab) app.go(tab.id);
    else if (e.key === "g") app.go("dag");
    else if (e.key === "Backspace" && app.tab === "dag" && app.previousTab) app.back();
    else if (e.key === "r") app.refresh();
    else if (e.key === "t") cycleTheme();
    else if (e.key === "n") app.createOpen = true;
    else if (e.key === "?") app.showShortcuts = true;
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="shell">
  <nav class="sidebar">
    <div class="brand">
      <img src="/static/images/torc_icon_32.png" alt="" width="22" height="22" />
      <span>Torc</span>
    </div>
    <ul>
      {#each TABS as t (t.id)}
        <li>
          <a
            href="#/{t.id}"
            class:active={app.tab === t.id}
            onclick={(e) => {
              e.preventDefault();
              app.go(t.id);
            }}
          >
            <Icon name={VIEWS[t.id].icon} />
            <span class="grow">{t.label}</span>
            {#if t.id === "events" && app.eventBadge > 0}
              <span class="badge">{app.eventBadge > 99 ? "99+" : app.eventBadge}</span>
            {/if}
          </a>
        </li>
      {/each}
    </ul>
    <div class="sidebar-footer">
      <div
        class="status"
        title={app.connected
          ? "Connected to torc-server"
          : (app.connectionError ?? "Not connected")}
      >
        <span
          class="dot"
          class:ok={app.connected === true}
          class:bad={app.connected === false}
        ></span>
        {app.connected === null ? "Connecting…" : app.connected ? "Connected" : "Disconnected"}
      </div>
      <div class="meta">
        {#if app.user}<span title="User"><Icon name="user" size={12} /> {app.user}</span>{/if}
        {#if app.version}
          <span class:warn={versionWarning} title={versionWarning ?? "Version"}>
            {versionWarning ? "⚠ " : ""}v{app.version}{serverVersion ? ` / server v${serverVersion}` : ""}
          </span>
        {/if}
      </div>
      <div class="footer-actions">
        <button class="btn btn-ghost btn-sm" onclick={cycleTheme} title="Theme: {app.theme} (T)">
          <Icon name={THEME_ICON[app.theme]} size={14} />
          <span class="cap">{app.theme}</span>
        </button>
        <button class="btn btn-ghost btn-sm" onclick={() => (app.showShortcuts = true)}>
          <kbd>?</kbd> Shortcuts
        </button>
      </div>
    </div>
  </nav>

  <div class="main">
    <header class="topbar">
      <h1>{currentLabel}</h1>
      {#if WORKFLOW_TABS.has(app.tab)}<WorkflowPicker />{/if}
      <div class="grow"></div>
      <button class="btn btn-ghost btn-sm" onclick={() => app.refresh()} title="Refresh (R)">
        <Icon name="refresh" size={14} /> Refresh
      </button>
    </header>
    <main class="content">
      <View />
    </main>
  </div>
</div>

<CreateWorkflowModal />
<JobDetailsModal />
<FileViewerModal />
<ExecutionPanel />
<ConfirmHost />
<Toasts />

<Modal bind:open={app.showShortcuts} title="Keyboard shortcuts" size="sm">
  <dl class="kv shortcuts">
    {#each TABS as t (t.id)}<dt><kbd>{t.key}</kbd></dt><dd>{t.label}</dd>{/each}
    <dt><kbd>G</kbd></dt><dd>Graph</dd>
    <dt><kbd>⌫</kbd></dt><dd>Back from the graph</dd>
    <dt><kbd>N</kbd></dt><dd>New workflow</dd>
    <dt><kbd>R</kbd></dt><dd>Refresh</dd>
    <dt><kbd>T</kbd></dt><dd>Cycle theme (system / light / dark)</dd>
    <dt><kbd>?</kbd></dt><dd>This help</dd>
    <dt><kbd>Esc</kbd></dt><dd>Close dialog / leave text field</dd>
    <dt><kbd>/</kbd></dt><dd>Focus the filter</dd>
  </dl>
</Modal>

<style>
  .shell {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }
  .sidebar {
    width: var(--sidebar-width);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border-right: 1px solid var(--border);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    height: var(--topbar-height);
    padding: 0 18px;
    font-size: 16px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  ul {
    list-style: none;
    margin: 4px 0 0;
    padding: 0 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  ul a {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 34px;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    text-decoration: none;
    font-weight: 500;
  }
  ul a:hover {
    background: var(--surface-hover);
    color: var(--text);
  }
  ul a.active {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .badge {
    min-width: 20px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 11px;
    font-weight: 600;
    text-align: center;
  }
  .sidebar-footer {
    margin-top: auto;
    padding: 12px 14px 14px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .dot.ok {
    background: var(--success);
    box-shadow: 0 0 0 3px var(--success-soft);
  }
  .dot.bad {
    background: var(--danger);
    box-shadow: 0 0 0 3px var(--danger-soft);
  }
  .meta {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  .meta span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .warn {
    color: var(--warning);
  }
  .footer-actions {
    display: flex;
    gap: 4px;
    margin: 0 -8px;
  }
  .cap {
    text-transform: capitalize;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .topbar {
    display: flex;
    align-items: center;
    gap: 16px;
    height: var(--topbar-height);
    padding: 0 16px 0 24px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }
  .topbar h1 {
    font-size: 15px;
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .shortcuts {
    align-items: center;
  }
  @media (max-width: 760px) {
    .sidebar {
      width: 60px;
    }
    .brand span,
    ul a span,
    .sidebar-footer {
      display: none;
    }
  }
</style>
