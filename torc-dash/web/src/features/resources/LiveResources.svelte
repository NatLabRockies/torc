<!-- Live CPU/memory plots: polls /api/cli/resource-samples for each selected node database. -->
<script lang="ts">
  import { untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { dashPost } from "../../lib/api/dash";
  import { errorText, formatDuration } from "../../lib/format";
  import PlotView, { type Figure } from "./PlotView.svelte";

  let { dbs }: { dbs: { path: string; name: string }[] } = $props();

  type Series = {
    timestamp: number[];
    cpu_percent: number[];
    memory_bytes: number[];
    /** Node samples only. */
    total_memory_bytes?: number[];
  };
  const COLUMNS = ["timestamp", "cpu_percent", "memory_bytes", "total_memory_bytes"] as const;
  type SamplesResponse = {
    success: boolean;
    system: Series;
    jobs: Record<string, Series>;
    job_names: [number, string][];
    error?: string;
  };
  type Node = {
    label: string;
    system: Series;
    jobs: Map<number, Series>;
    names: [number, string][];
    error: string | null;
    /** Newest sample timestamp read so far; the next poll resumes after it. */
    newest: number;
  };

  const POLL_MS = 5000;
  const WINDOWS = [
    { s: 900, label: "15 min" },
    { s: 3600, label: "1 hour" },
    { s: 6 * 3600, label: "6 hours" },
    { s: 0, label: "All" },
  ];

  let windowSec = $state(3600);
  let showNodes = $state(true);
  let paused = $state(false);
  let jobFilter = $state("");
  /** Selected jobs as `${dbPath}|${jobId}`. */
  const selectedJobs = new SvelteSet<string>();
  let nodes = $state.raw(new Map<string, Node>());
  let now = $state(Date.now() / 1000);

  const empty = (): Series => ({ timestamp: [], cpu_percent: [], memory_bytes: [] });
  const last = (s: Series) => s.timestamp.at(-1) ?? 0;
  const nodeLabel = (name: string) => name.replace(/^resource_metrics_/, "").replace(/\.db$/, "");
  const jobsFor = (path: string) =>
    [...selectedJobs].filter((k) => k.startsWith(path + "|")).map((k) => Number(k.split("|").pop()));

  const mapColumns = (f: (k: (typeof COLUMNS)[number]) => number[]) =>
    Object.fromEntries(COLUMNS.map((k) => [k, f(k)])) as Series;

  /** Append points newer than what `into` already has, then drop those older than the window. */
  function merge(into: Series, add: Series, cutoff: number): Series {
    const from = add.timestamp.findIndex((t) => t > last(into));
    const out =
      from < 0 ? into : mapColumns((k) => (into[k] ?? []).concat((add[k] ?? []).slice(from)));
    const keep = out.timestamp.findIndex((t) => t > cutoff);
    if (keep <= 0) return keep < 0 ? empty() : out;
    return mapColumns((k) => (out[k] ?? []).slice(keep));
  }

  async function pollOne(path: string, label: string, win: number, prev: Node | undefined) {
    const jobIds = jobsFor(path);
    const newest = prev?.newest ?? 0;
    // A newly selected job needs its history, so reload the window (merge drops duplicates).
    const known = prev !== undefined && jobIds.every((id) => prev.jobs.has(id));
    const since = known ? newest : win && newest ? newest - win : 0;
    const node: Node = prev
      ? { ...prev, jobs: new Map(prev.jobs) }
      : { label, system: empty(), jobs: new Map(), names: [], error: null, newest: 0 };
    try {
      const r = await dashPost<SamplesResponse>("/api/cli/resource-samples", {
        db_path: path,
        since,
        job_ids: jobIds,
      });
      if (!r.success) throw new Error(r.error || "Failed to read samples");
      const top = Math.max(last(r.system), ...Object.values(r.jobs).map(last), newest);
      const cutoff = win ? top - win : 0;
      node.system = merge(node.system, r.system, cutoff);
      for (const id of [...node.jobs.keys()]) if (!jobIds.includes(id)) node.jobs.delete(id);
      for (const id of jobIds) {
        node.jobs.set(id, merge(node.jobs.get(id) ?? empty(), r.jobs[id] ?? empty(), cutoff));
      }
      node.names = r.job_names.toReversed();
      node.error = null;
      node.newest = top;
    } catch (e) {
      node.error = errorText(e);
    }
    return node;
  }

  // Window changes reload from scratch.
  $effect(() => {
    void windowSec;
    untrack(() => (nodes = new Map()));
  });

  $effect(() => {
    const list = dbs;
    const win = windowSec;
    void [...selectedJobs];
    if (paused) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      if (document.visibilityState === "visible") {
        const prev = untrack(() => nodes);
        const next = await Promise.all(
          list.map((d) => pollOne(d.path, nodeLabel(d.name), win, prev.get(d.path))),
        );
        if (stopped) return;
        nodes = new Map(list.map((d, i) => [d.path, next[i]]));
        now = Date.now() / 1000;
      }
      if (!stopped) timer = setTimeout(poll, POLL_MS);
    }
    void poll();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  });

  const GIB = 1024 ** 3;
  type Panel = { title: string; traces: Record<string, unknown>[]; range?: number[] };

  // Node utilization is a percentage of the node's capacity; job CPU is per process (100% = one
  // core), so the two get separate panels instead of sharing a misleading axis.
  let panels = $derived.by((): Panel[] => {
    const nodeCpu: Panel = { title: "Node CPU %", traces: [], range: [0, 100] };
    const nodeMem: Panel = { title: "Node memory %", traces: [], range: [0, 100] };
    const jobCpu: Panel = { title: "Job CPU % (100 = 1 core)", traces: [] };
    const jobMem: Panel = { title: "Job memory (GiB)", traces: [] };
    const line = (name: string, s: Series, extra: Record<string, unknown> = {}) => ({
      name,
      x: s.timestamp.map((t) => new Date(t * 1000)),
      type: "scatter",
      mode: "lines",
      legendgroup: name,
      ...extra,
    });
    for (const n of nodes.values()) {
      const sys = n.system;
      if (showNodes && sys.timestamp.length) {
        const name = `${n.label} (node)`;
        const total = sys.total_memory_bytes ?? [];
        nodeCpu.traces.push(line(name, sys, { y: sys.cpu_percent }));
        nodeMem.traces.push(
          line(name, sys, {
            y: sys.memory_bytes.map((b, i) => (total[i] ? (100 * b) / total[i] : null)),
            customdata: sys.memory_bytes.map((b) => b / GIB),
            hovertemplate: "%{y:.1f}% (%{customdata:.1f} GiB)",
            showlegend: false,
          }),
        );
      }
      const names = new Map(n.names);
      for (const [id, s] of n.jobs) {
        const name = `${names.get(id) ?? "job"} (${id})${nodes.size > 1 ? ` · ${n.label}` : ""}`;
        jobCpu.traces.push(line(name, s, { y: s.cpu_percent }));
        jobMem.traces.push(
          line(name, s, { y: s.memory_bytes.map((b) => b / GIB), showlegend: false }),
        );
      }
    }
    return [nodeCpu, nodeMem, jobCpu, jobMem].filter((p) => p.traces.length);
  });

  let figure = $derived.by((): Figure => {
    const gap = 0.06;
    const h = (1 - gap * (panels.length - 1)) / panels.length;
    const layout: Record<string, unknown> = {
      uirevision: "live",
      // One shared time axis under the stacked panels.
      xaxis: { type: "date", anchor: `y${panels.length > 1 ? panels.length : ""}` },
      legend: { orientation: "h", y: -0.08 },
      margin: { l: 60, r: 20, t: 20, b: 40 },
      hovermode: "x unified",
    };
    const data = panels.flatMap((p, i) => {
      const axis = i ? `y${i + 1}` : "y";
      const top = 1 - i * (h + gap);
      layout[`yaxis${i ? i + 1 : ""}`] = {
        title: { text: p.title },
        domain: [Math.max(0, top - h), top],
        ...(p.range ? { range: p.range } : { rangemode: "tozero" }),
      };
      return p.traces.map((t) => ({ ...t, yaxis: axis }));
    });
    return { data, layout };
  });
  let panelCount = $derived(panels.length);

  let hasData = $derived(
    [...nodes.values()].some(
      (n) => n.system.timestamp.length || [...n.jobs.values()].some((s) => s.timestamp.length),
    ),
  );
  let filterLower = $derived(jobFilter.trim().toLowerCase());
  const matches = ([id, name]: [number, string]) =>
    !filterLower || name.toLowerCase().includes(filterLower) || String(id).includes(filterLower);

  function age(n: Node): string {
    const t = n.newest;
    if (!t) return "no samples yet";
    return `last sample ${formatDuration(Math.max(0, Math.round(now - t)))} ago`;
  }

  function toggleJob(key: string, on: boolean) {
    if (on) selectedJobs.add(key);
    else selectedJobs.delete(key);
  }
</script>

<div class="live">
  <div class="toolbar">
    <label class="field">
      <span>Window</span>
      <select class="input" bind:value={windowSec}>
        {#each WINDOWS as w (w.s)}<option value={w.s}>{w.label}</option>{/each}
      </select>
    </label>
    <label class="check"><input type="checkbox" bind:checked={showNodes} /> Node utilization</label>
    <button class="btn" onclick={() => (paused = !paused)}>{paused ? "Resume" : "Pause"}</button>
    <span class="faint">Updates every {POLL_MS / 1000}s</span>
  </div>

  {#if dbs.length === 0}
    <div class="empty">Select one or more compute node databases to monitor.</div>
  {:else}
    <div class="body">
      <aside class="jobs">
        <input class="input" placeholder="Filter jobs" bind:value={jobFilter} />
        {#each dbs as d (d.path)}
          {@const n = nodes.get(d.path)}
          <div class="node">
            <div class="node-head" title={d.path}>{nodeLabel(d.name)}</div>
            {#if n?.error}
              <div class="error-text">{n.error}</div>
            {:else if n}
              <div class="faint">{age(n)}</div>
            {:else}
              <div class="faint"><span class="spinner"></span> Loading…</div>
            {/if}
            {#each (n?.names ?? []).filter(matches) as [id, name] (id)}
              {@const key = `${d.path}|${id}`}
              <label class="check job">
                <input
                  type="checkbox"
                  checked={selectedJobs.has(key)}
                  onchange={(e) => toggleJob(key, e.currentTarget.checked)}
                />
                <span title={name}>{name} <span class="faint">({id})</span></span>
              </label>
            {/each}
          </div>
        {/each}
      </aside>
      <div class="chart" style="--panels: {Math.max(2, panelCount)}">
        {#if hasData}
          <PlotView {figure} />
        {:else}
          <div class="empty">
            <div>No samples yet.</div>
            <div class="faint">
              Live plots need <code>resource_monitor.live_monitoring: true</code> with time-series
              monitoring, and new samples appear every <code>flush_interval_seconds</code> (default
              300). See the resource monitoring docs.
            </div>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .live {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: flex-end;
    gap: 12px;
    flex-wrap: wrap;
    padding: 12px;
    border-bottom: 1px solid var(--border);
  }
  .body {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(200px, 260px) 1fr;
    min-height: 0;
  }
  @media (max-width: 900px) {
    .body {
      grid-template-columns: 1fr;
    }
  }
  .jobs {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border-right: 1px solid var(--border);
    overflow: auto;
    max-height: 640px;
  }
  .node {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12.5px;
  }
  .node-head {
    font-family: var(--mono);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .job span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chart {
    display: flex;
    flex-direction: column;
    padding: 8px;
    min-width: 0;
  }
  .chart :global(.plot) {
    min-height: calc(var(--panels) * 240px);
  }
  .empty {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 520px;
  }
</style>
