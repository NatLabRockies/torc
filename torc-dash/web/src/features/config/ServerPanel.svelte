<!-- Start/stop a torc-server process managed by torc-dash (/api/server/*). -->
<script lang="ts">
  import { app } from "../../lib/state.svelte";
  import { dashGet, dashPost } from "../../lib/api/dash";
  import { toast } from "../../lib/ui/toast.svelte";
  import { confirm } from "../../lib/ui/confirm.svelte";
  import Icon from "../../lib/ui/Icon.svelte";

  type Status = {
    running: boolean;
    managed: boolean;
    pid: number | null;
    port: number | null;
    output_lines: string[];
  };

  let status = $state<Status | null>(null);
  let busy = $state<"start" | "stop" | null>(null);
  // The dashboard proxies to its configured API URL (default port 8080), so 8080 is the useful
  // default; the legacy UI's "0" also ended up as 8080.
  let port = $state(8080);
  let database = $state("");
  let logLevel = $state("info");

  async function check() {
    try {
      status = await dashGet<Status>("/api/server/status");
    } catch {
      status = { running: false, managed: false, pid: null, port: null, output_lines: [] };
    }
  }

  $effect(() => {
    void app.refreshTick;
    check();
  });

  async function start() {
    busy = "start";
    try {
      const r = await dashPost<{ success: boolean; message: string }>("/api/server/start", {
        port: port || 0,
        database: database.trim() || null,
        log_level: logLevel,
      });
      if (!r.success) throw new Error(r.message);
      toast.success(r.message);
      await new Promise((ok) => setTimeout(ok, 1000));
      await check();
      app.refresh();
    } catch (e) {
      toast.error(e);
    } finally {
      busy = null;
    }
  }

  async function stop() {
    const ok = await confirm({
      title: "Stop torc-server?",
      message: `This kills the managed server process (PID ${status?.pid}). The dashboard loses its connection until a server is running again.`,
      confirmLabel: "Stop server",
      danger: true,
    });
    if (!ok) return;
    busy = "stop";
    try {
      const r = await dashPost<{ success: boolean; message: string }>("/api/server/stop", {});
      if (!r.success) throw new Error(r.message);
      toast.success(r.message);
    } catch (e) {
      toast.error(e);
    } finally {
      busy = null;
      await check();
      app.refresh();
    }
  }

  let running = $derived(!!status?.running && status.managed);
</script>

<section class="card card-body stack">
  <h3>Server management</h3>

  <div class="status" class:on={running}>
    <span class="dot"></span>
    {#if !status}
      Checking…
    {:else if running}
      Managed server running <span class="muted mono">PID {status.pid} · port {status.port}</span>
    {:else if status.managed}
      Managed server stopped
    {:else}
      No server managed by this dashboard
      <span class="muted">{app.connected ? "(an external server is connected)" : ""}</span>
    {/if}
  </div>

  {#if !running}
    <div class="row">
      <label class="field">
        <span>Port</span>
        <input class="input" type="number" min="0" max="65535" bind:value={port} />
      </label>
      <label class="field">
        <span>Log level</span>
        <select class="select" bind:value={logLevel}>
          {#each ["error", "warn", "info", "debug", "trace"] as l (l)}<option value={l}>{l}</option>{/each}
        </select>
      </label>
    </div>
    <label class="field">
      <span>Database path</span>
      <input class="input mono" bind:value={database} placeholder="db/sqlite/dev.db (default)" />
      <span class="hint">
        The dashboard talks to its configured API URL; use the matching port (0 picks a free port).
      </span>
    </label>
  {/if}

  <div class="toolbar">
    {#if running}
      <button class="btn btn-danger" onclick={stop} disabled={busy !== null}>
        {#if busy === "stop"}<span class="spinner"></span>{:else}<Icon name="stop" size={14} />{/if}
        Stop server
      </button>
    {:else}
      <button class="btn btn-primary" onclick={start} disabled={busy !== null || !status}>
        {#if busy === "start"}<span class="spinner"></span>{:else}<Icon name="play" size={14} />{/if}
        Start server
      </button>
    {/if}
  </div>

  {#if status?.output_lines.length}
    <div class="field">
      <span>Server output</span>
      <pre class="code-block output">{status.output_lines.join("\n")}</pre>
    </div>
  {/if}
</section>

<style>
  .stack {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  h3 {
    font-size: 15px;
  }
  .row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--neutral-soft);
  }
  .status.on {
    background: var(--success-soft);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .on .dot {
    background: var(--success);
  }
  .output {
    max-height: 220px;
  }
</style>
