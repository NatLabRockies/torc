<!-- Settings: app preferences, version info, and the torc-server process managed by torc-dash. -->
<script lang="ts">
  import { app, DARK_PALETTES, type DarkPalette, type ThemeMode } from "../../lib/state.svelte";
  import { dashGet } from "../../lib/api/dash";
  import Icon, { type IconName } from "../../lib/ui/Icon.svelte";
  import { notify } from "../notify/notify.svelte";
  import ServerPanel from "./ServerPanel.svelte";

  type Version = {
    version: string;
    api_version: string;
    server_version: string | null;
    server_api_version: string | null;
    version_mismatch: string | null;
    mismatch_severity: "patch" | "minor" | "major" | null;
    api_url: string;
  };

  let testing = $state(false);
  let testResult = $state<{ ok: boolean; message: string } | null>(null);

  async function testConnection() {
    testing = true;
    const ok = await app.checkConnection();
    testing = false;
    const url = version?.api_url ?? "torc-server";
    testResult = ok
      ? { ok, message: `Connected to ${url}` }
      : { ok, message: `Connection to ${url} failed: ${app.connectionError ?? "unknown error"}` };
  }

  let version = $state<Version | null>(null);
  let versionError = $state<string | null>(null);

  $effect(() => {
    void app.refreshTick;
    dashGet<Version>("/api/version")
      .then((v) => ((version = v), (versionError = null)))
      .catch((e: unknown) => (versionError = e instanceof Error ? e.message : String(e)));
  });

  const THEMES: { id: ThemeMode; label: string; icon: IconName }[] = [
    { id: "system", label: "System", icon: "monitor" },
    { id: "light", label: "Light", icon: "sun" },
    { id: "dark", label: "Dark", icon: "moon" },
  ];

  const REFRESH_OPTIONS = [0, 5, 10, 15, 30, 60, 120, 300];
  // Keep a custom value (e.g. set before this list existed) selectable.
  let refreshOptions = $derived(
    REFRESH_OPTIONS.includes(app.refreshSeconds)
      ? REFRESH_OPTIONS
      : [...REFRESH_OPTIONS, app.refreshSeconds].sort((a, b) => a - b),
  );
</script>

<div class="page">
  <div class="page-header"><h2>Settings</h2></div>

  <div class="grid">
    <section class="card card-body stack">
      <h3>Appearance &amp; behavior</h3>

      <div class="field">
        <span>Theme</span>
        <div class="seg" role="radiogroup" aria-label="Theme">
          {#each THEMES as t (t.id)}
            <button
              role="radio"
              aria-checked={app.theme === t.id}
              class:active={app.theme === t.id}
              onclick={() => app.setTheme(t.id)}
            >
              <Icon name={t.icon} size={14} />
              {t.label}
            </button>
          {/each}
        </div>
        <span class="hint">Press <kbd>T</kbd> anywhere to cycle.</span>
      </div>

      {#if app.theme !== "light"}
        <label class="field">
          <span>Dark palette</span>
          <select
            class="select"
            value={app.darkPalette}
            onchange={(e) => app.setDarkPalette(e.currentTarget.value as DarkPalette)}
          >
            {#each DARK_PALETTES as p (p.id)}<option value={p.id}>{p.label}</option>{/each}
          </select>
        </label>
      {/if}

      <label class="field">
        <span>Auto-refresh</span>
        <select
          class="select"
          value={app.refreshSeconds}
          onchange={(e) => app.setRefreshSeconds(Number(e.currentTarget.value))}
        >
          {#each refreshOptions as s (s)}
            <option value={s}>{s === 0 ? "Off" : s < 60 ? `Every ${s}s` : `Every ${s / 60} min`}</option>
          {/each}
        </select>
        <span class="hint">Paused while the window is hidden. Press <kbd>R</kbd> to refresh now.</span>
      </label>

      <label class="check">
        <input
          type="checkbox"
          checked={app.showAllUsers}
          onchange={(e) => app.setShowAllUsers(e.currentTarget.checked)}
        />
        Show workflows from all users by default
      </label>

      <div class="field">
        <label class="check">
          <input
            type="checkbox"
            checked={notify.enabled}
            disabled={notify.permission === "unsupported"}
            onchange={(e) => notify.setEnabled(e.currentTarget.checked)}
          />
          Notify me when a workflow finishes
        </label>
        <span class="hint">
          {#if notify.permission === "unsupported"}
            This browser does not support notifications.
          {:else if notify.enabled && notify.permission === "denied"}
            Notifications are blocked for this site; allow them in the browser's site settings.
          {:else}
            For the selected workflow and those you run, watch, or submit here, while this window is
            in the background.
          {/if}
        </span>
      </div>
    </section>

    <section class="card card-body stack">
      <h3>Version</h3>
      {#if version}
        {#if version.version_mismatch}
          <div class="banner" class:major={version.mismatch_severity === "major"}>
            {version.version_mismatch}
          </div>
        {/if}
        <dl class="kv">
          <dt>Dashboard</dt>
          <dd class="mono">v{version.version} <span class="faint">(API {version.api_version})</span></dd>
          <dt>Server</dt>
          <dd class="mono">
            {#if version.server_version}
              v{version.server_version}
              {#if version.server_api_version}<span class="faint">(API {version.server_api_version})</span>{/if}
            {:else}
              <span class="muted">unreachable</span>
            {/if}
          </dd>
          <dt>Server URL</dt>
          <dd class="mono">{version.api_url}</dd>
          <dt>User</dt>
          <dd>{app.user ?? "—"}</dd>
        </dl>
      {:else if versionError}
        <p class="error-text">{versionError}</p>
      {:else}
        <span class="spinner"></span>
      {/if}
      <div class="test">
        <button class="btn btn-sm" onclick={testConnection} disabled={testing}>
          {#if testing}<span class="spinner"></span>{/if} Test connection
        </button>
        {#if testResult}
          <span class:ok-text={testResult.ok} class:error-text={!testResult.ok}>{testResult.message}</span>
        {/if}
      </div>
    </section>

    <ServerPanel />
  </div>
</div>

<style>
  .page {
    overflow: auto;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
    gap: 16px;
    align-items: start;
    max-width: 1200px;
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  h3 {
    font-size: 15px;
  }
  .test {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .ok-text {
    color: var(--success);
  }
  .seg {
    display: inline-flex;
    align-self: flex-start;
    gap: 2px;
    padding: 3px;
    background: var(--neutral-soft);
    border-radius: var(--radius);
  }
  .seg button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-weight: 500;
    cursor: pointer;
  }
  .seg button.active {
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-sm);
  }
  .select {
    align-self: flex-start;
    min-width: 180px;
  }
  .banner {
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--warning-soft);
    color: var(--warning);
    border: 1px solid var(--warning);
    font-size: 13px;
  }
  .banner.major {
    background: var(--danger-soft);
    color: var(--danger);
    border-color: var(--danger);
  }
</style>
