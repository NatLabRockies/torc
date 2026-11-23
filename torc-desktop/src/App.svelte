<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  type Prompt = { id: number; prompt: string };
  /** The form's fields as saved: the last connection, and each named profile (plus `name`). */
  type Settings = Record<string, string>;
  const STORAGE_KEY = "torc-desktop.connection";
  const PROFILES_KEY = "torc-desktop.profiles";

  function load<T>(key: string, fallback: T): T {
    try {
      return JSON.parse(localStorage.getItem(key) ?? "") ?? fallback;
    } catch {
      return fallback;
    }
  }

  let mode = $state<"local" | "ssh">("local");
  let host = $state("");
  let setup = $state("");
  // Per mode: a login-node path rarely exists on this computer.
  let workdir = $state("");
  let localWorkdir = $state("");
  // Which torc-server the dashboard talks to, per mode. A blank URL lets torc-dash use
  // TORC_API_URL or `[dash] api_url` from the torc config where it runs.
  let servers = $state({ local: { private: false, url: "" }, ssh: { private: false, url: "" } });

  function apply(s: Settings) {
    mode = s.mode === "ssh" ? "ssh" : "local";
    host = s.host ?? "";
    setup = s.setup ?? "";
    workdir = s.workdir ?? "";
    localWorkdir = s.localWorkdir ?? "";
    servers = {
      local: {
        private: s.startServer === "true",
        // Settings saved before version 2 hold this pre-filled default, which hid TORC_API_URL;
        // blank means the same thing. Later saves keep it as an explicit choice.
        url:
          !s.version && s.serverUrl === "http://localhost:8080/torc-service/v1"
            ? ""
            : (s.serverUrl ?? ""),
      },
      ssh: { private: s.remotePrivate === "true", url: s.remoteServerUrl ?? "" },
    };
  }

  function snapshot(): Settings {
    return {
      version: "2",
      mode,
      serverUrl: servers.local.url,
      startServer: String(servers.local.private),
      host,
      setup,
      workdir,
      localWorkdir,
      remoteServerUrl: servers.ssh.url,
      remotePrivate: String(servers.ssh.private),
    };
  }

  const saved = load<Settings>(STORAGE_KEY, {});
  apply(saved);
  let profiles = $state<Settings[]>(load(PROFILES_KEY, []));
  /** Name of the profile being edited; blank for an unnamed connection. */
  let name = $state(saved.profile ?? "");
  let current = $derived(profiles.find((p) => p.name === name.trim()));
  let busy = $state(false);
  let error = $state("");
  // Set by the Rust side when the SSH session dropped while connected.
  let lost = $state(location.hash === "#lost");
  let prompt = $state<Prompt | null>(null);
  let answer = $state("");
  // Host-key confirmations ("yes/no") are shown in clear text; everything else is masked.
  let masked = $derived(!/yes\/no/i.test(prompt?.prompt ?? ""));

  listen<Prompt>("askpass", (e) => {
    prompt = e.payload;
    answer = "";
  });

  function connectArgs() {
    const server = servers[mode];
    return {
      mode,
      host,
      setup,
      workdir: mode === "local" ? localWorkdir : workdir,
      command: "torc-dash",
      standalone: server.private,
      serverUrl: server.private ? "" : server.url,
    };
  }

  function store(key: string, value: unknown) {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {
      // Remembering the form is a convenience only.
    }
  }

  function pick(e: Event) {
    const p = profiles.find((p) => p.name === (e.currentTarget as HTMLSelectElement).value);
    if (p) apply(p);
    name = p?.name ?? "";
  }

  /** Saves the form as the named profile, replacing one of the same name. */
  function saveProfile() {
    const n = name.trim();
    if (!n) return;
    profiles = [...profiles.filter((p) => p.name !== n), { ...snapshot(), name: n }].sort((a, b) =>
      a.name.localeCompare(b.name),
    );
    store(PROFILES_KEY, profiles);
  }

  function deleteProfile() {
    profiles = profiles.filter((p) => p !== current);
    store(PROFILES_KEY, profiles);
    name = "";
  }

  async function connect(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = "";
    lost = false;
    saveProfile();
    store(STORAGE_KEY, { ...snapshot(), profile: name.trim() });
    try {
      // On success the window navigates to the dashboard and this page goes away.
      await invoke("connect", { args: connectArgs() });
    } catch (err) {
      error = String(err);
      busy = false;
      prompt = null; // the attempt is over; its password prompt can no longer be answered
    }
  }

  async function reply(e: SubmitEvent | null) {
    e?.preventDefault();
    const p = prompt!;
    prompt = null;
    await invoke("askpass_reply", { id: p.id, answer: e ? answer : null });
    answer = "";
  }
</script>

<main>
  <form class="card" onsubmit={connect}>
    <h1>Connect to Torc</h1>

    {#if profiles.length}
      <select aria-label="Saved connection" value={current?.name ?? ""} onchange={pick}>
        <option value="">New connection</option>
        {#each profiles as p (p.name)}<option>{p.name}</option>{/each}
      </select>
    {/if}

    <div class="seg" role="tablist">
      <button
        type="button"
        role="tab"
        aria-selected={mode === "local"}
        class:active={mode === "local"}
        onclick={() => (mode = "local")}>This computer</button
      >
      <button
        type="button"
        role="tab"
        aria-selected={mode === "ssh"}
        class:active={mode === "ssh"}
        onclick={() => (mode = "ssh")}>Remote (SSH)</button
      >
    </div>

    {#if lost}
      <p class="notice">
        {mode === "ssh"
          ? `The connection to ${host || "the login node"} was lost.`
          : "The dashboard stopped."}
      </p>
    {/if}

    {#if mode === "local"}
      <p class="hint">Starts the Torc dashboard on this computer.</p>
      <label>
        Working directory <span class="optional">(optional)</span>
        <input bind:value={localWorkdir} placeholder="~ (home)" />
        <span class="optional">Where workflows run and write output, e.g. <code>~/projects/sim</code>.</span>
      </label>
    {:else}
      <p class="hint">
        Opens an SSH session to your login node and starts the Torc dashboard there.
      </p>
      <label>
        Login node
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={host} placeholder="user@login.example.org" required autofocus />
      </label>
      <label>
        Environment setup <span class="optional">(optional)</span>
        <input bind:value={setup} placeholder="module load torc" />
        <span class="optional">Runs before <code>torc-dash</code>; use it to put torc-dash on the PATH.</span>
      </label>
      <label>
        Working directory <span class="optional">(optional)</span>
        <input bind:value={workdir} placeholder="~ (home)" />
        <span class="optional">Where workflows run and write output, e.g. <code>/scratch/$USER</code>.</span>
      </label>
    {/if}

    <fieldset>
      <legend>Torc server</legend>
      <label class="check">
        <input type="radio" bind:group={servers[mode].private} value={false} />
        Use a shared torc-server
      </label>
      {#if !servers[mode].private}
        <label class="indent">
          URL
          <input
            type="url"
            bind:value={servers[mode].url}
            placeholder="https://torc.example.org/torc-service/v1"
          />
          <span class="optional">
            Leave blank to use <code>TORC_API_URL</code> or <code>[dash] api_url</code> in the
            torc config{mode === "ssh" ? " on the login node" : ""}.
          </span>
        </label>
        <span class="optional indent">
          If it requires a login, set <code>TORC_PASSWORD</code> in your shell profile{mode ===
          "ssh"
            ? " on the login node"
            : ""}.
        </span>
      {/if}
      <label class="check">
        <input type="radio" bind:group={servers[mode].private} value={true} />
        Start a private torc-server for me
      </label>
      {#if servers[mode].private}
        <span class="optional indent">
          Listens on localhost only; its database is
          {mode === "ssh" ? "~/torc-desktop.db on the login node" : "kept in the app's data folder"}.
        </span>
      {/if}
    </fieldset>

    <div class="field">
      <label for="profile-name">Save as <span class="optional">(optional)</span></label>
      <div class="row">
        <input id="profile-name" bind:value={name} placeholder="Profile name, e.g. Kestrel" />
        <button type="button" class="secondary" onclick={saveProfile} disabled={!name.trim()}>
          Save
        </button>
        {#if current}
          <button type="button" class="secondary" onclick={deleteProfile}>Delete</button>
        {/if}
      </div>
      <span class="optional">Connecting also saves changes to the named profile.</span>
    </div>

    <button type="submit" disabled={busy}>
      {busy ? "Starting…" : lost ? "Reconnect" : mode === "local" ? "Open dashboard" : "Connect"}
    </button>

    {#if error}
      <pre class="error">{error}</pre>
    {/if}
  </form>

  {#if prompt}
    <div class="overlay">
      <form class="card" onsubmit={reply}>
        <pre class="prompt">{prompt.prompt}</pre>
        <!-- svelte-ignore a11y_autofocus -->
        <input type={masked ? "password" : "text"} bind:value={answer} autofocus />
        <div class="actions">
          <button type="button" class="secondary" onclick={() => reply(null)}>Cancel</button>
          <button type="submit">OK</button>
        </div>
      </form>
    </div>
  {/if}
</main>

<style>
  :global(:root) {
    --bg: #f5f6f8;
    --card: #ffffff;
    --text: #1d2330;
    --muted: #6b7280;
    --border: #d6d9e0;
    --accent: #2563eb;
    --error: #b42318;
    color-scheme: light dark;
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
  }
  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --bg: #14171c;
      --card: #1d2128;
      --text: #e6e8ec;
      --muted: #9aa1ad;
      --border: #333944;
      --accent: #4f8cff;
      --error: #ff7a6b;
    }
  }
  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--text);
  }
  main {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 16px;
    box-sizing: border-box;
  }
  .card {
    width: min(440px, 100%);
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 28px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.08);
  }
  h1 {
    margin: 0;
    font-size: 1.4rem;
  }
  .hint,
  .optional {
    color: var(--muted);
    margin: 0;
    font-size: 0.9rem;
  }
  label,
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 0.9rem;
    font-weight: 500;
  }
  input,
  select {
    font: inherit;
    padding: 9px 11px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg);
    color: var(--text);
  }
  input:focus,
  select:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  button {
    font: inherit;
    font-weight: 600;
    padding: 10px;
    border: none;
    border-radius: 8px;
    background: var(--accent);
    color: white;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  button.secondary {
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
  }
  pre {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
    font-size: 0.85rem;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .row input {
    flex: 1;
    min-width: 0;
  }
  .row button {
    padding: 8px 14px;
  }
  .seg {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2px;
    padding: 3px;
    border-radius: 8px;
    background: var(--border);
  }
  .seg button {
    padding: 7px;
    background: transparent;
    color: var(--muted);
    font-weight: 500;
  }
  .seg button.active {
    background: var(--card);
    color: var(--text);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.1);
  }
  fieldset {
    border: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  legend {
    font-size: 0.9rem;
    font-weight: 500;
    padding: 0;
    margin-bottom: 10px;
  }
  .indent {
    margin-left: 24px;
  }
  code {
    font-size: 0.85em;
  }
  label.check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    font-weight: 400;
  }
  .notice {
    margin: 0;
    padding: 10px 12px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--error) 12%, transparent);
    color: var(--error);
    font-size: 0.9rem;
  }
  .error {
    color: var(--error);
    max-height: 240px;
    overflow: auto;
  }
  .overlay {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.4);
    padding: 16px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .actions button {
    padding: 8px 18px;
  }
</style>
