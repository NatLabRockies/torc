<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  type Prompt = { id: number; prompt: string };
  const STORAGE_KEY = "torc-desktop.connection";

  function load(): Record<string, string> {
    try {
      return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    } catch {
      return {};
    }
  }

  const saved = load();
  let mode = $state<"local" | "ssh">(saved.mode === "ssh" ? "ssh" : "local");
  let localCommand = $state(saved.localCommand ?? "torc-dash");
  let host = $state(saved.host ?? "");
  let setup = $state(saved.setup ?? "");
  let command = $state(saved.remoteCommand ?? "torc-dash");
  let workdir = $state(saved.workdir ?? "");
  // Which torc-server the dashboard talks to, per mode. A blank URL lets torc-dash use
  // TORC_API_URL or `[dash] api_url` from the torc config where it runs.
  let servers = $state({
    local: {
      private: saved.startServer === "true",
      url: saved.serverUrl ?? "http://localhost:8080/torc-service/v1",
    },
    ssh: { private: saved.remotePrivate === "true", url: saved.remoteServerUrl ?? "" },
  });
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
      workdir,
      command: mode === "local" ? localCommand : command,
      standalone: server.private,
      serverUrl: server.private ? "" : server.url,
    };
  }

  async function connect(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = "";
    lost = false;
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          mode,
          localCommand,
          serverUrl: servers.local.url,
          startServer: String(servers.local.private),
          host,
          setup,
          remoteCommand: command,
          workdir,
          remoteServerUrl: servers.ssh.url,
          remotePrivate: String(servers.ssh.private),
        }),
      );
    } catch {
      // Remembering the form is a convenience only.
    }
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
        Dashboard command
        <input bind:value={localCommand} required />
        <span class="optional">Full path if torc-dash isn't on your PATH.</span>
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
      </label>
      <label>
        Working directory <span class="optional">(optional)</span>
        <input bind:value={workdir} placeholder="~ (home)" />
        <span class="optional">Where workflows run and write output, e.g. <code>/scratch/$USER</code>.</span>
      </label>
      <label>
        Dashboard command
        <input bind:value={command} required />
        <span class="optional">Full path if torc-dash isn't on the login node's PATH.</span>
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
  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 0.9rem;
    font-weight: 500;
  }
  input {
    font: inherit;
    padding: 9px 11px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg);
    color: var(--text);
  }
  input:focus {
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
