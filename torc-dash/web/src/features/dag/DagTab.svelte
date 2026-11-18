<script lang="ts" module>
  import type cytoscapeType from "cytoscape";

  let cytoscapePromise: Promise<typeof cytoscapeType> | null = null;

  /** Lazy-load cytoscape + dagre once per page. */
  function loadCytoscape(): Promise<typeof cytoscapeType> {
    cytoscapePromise ??= Promise.all([import("cytoscape"), import("cytoscape-dagre")]).then(
      ([{ default: cytoscape }, { default: dagre }]) => {
        cytoscape.use(dagre);
        return cytoscape;
      },
    );
    return cytoscapePromise;
  }
</script>

<script lang="ts">
  import type { Core, LayoutOptions, NodeSingular, StylesheetJson } from "cytoscape";
  import { onMount, tick } from "svelte";
  import type { JobStatus } from "../../lib/api/client";
  import { JOB_STATUSES, errorText, statusLabel } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import SegmentedTabs from "../../lib/ui/SegmentedTabs.svelte";
  import { loadGraph, type GraphData, type GraphMode } from "./graph";
  import { cssVar, onThemeChange } from "./theme";

  /** Above this many nodes: skip the TB→LR retry and enable cytoscape's large-graph rendering
   * shortcuts (texture while panning, straight edges). Dagre takes ~1s per 3k nodes. */
  const LARGE = 1000;
  /** Above this many nodes, ask before rendering (layout can freeze the tab for many seconds). */
  const HUGE = 5000;

  const STATUS_VAR: Record<JobStatus, string> = {
    uninitialized: "--text-faint",
    blocked: "--text-muted",
    ready: "--info",
    pending: "--accent",
    running: "--accent",
    completed: "--success",
    failed: "--danger",
    canceled: "--warning",
    terminated: "--danger",
    disabled: "--border-strong",
    pending_failed: "--warning",
  };

  let mode = $state<GraphMode>("jobs");
  let data = $state.raw<GraphData | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let forceHuge = $state(false);
  let layingOut = $state(false);
  let hovered = $state<{ name: string; status?: JobStatus; kind: string } | null>(null);
  let container = $state<HTMLDivElement>();

  let cy: Core | null = null;
  let renderedKey = "";
  let renderedSig = "";
  let seq = 0;

  let edgeCount = $derived(data ? data.elements.filter((e) => e.group === "edges").length : 0);
  let tooBig = $derived(!!data && data.nodeCount > HUGE && !forceHuge);
  let legendStatuses = $derived(
    data ? JOB_STATUSES.filter((s) => (data!.statusCounts[s] ?? 0) > 0) : [],
  );

  $effect(() => {
    const wf = app.workflowId;
    const m = mode;
    void app.refreshTick;
    if (wf === null) {
      data = null;
      return;
    }
    void load(wf, m);
  });

  // Reset state that's specific to a workflow/mode.
  $effect(() => {
    void app.workflowId;
    void mode;
    forceHuge = false;
    hovered = null;
  });

  async function load(wf: number, m: GraphMode) {
    const my = ++seq;
    const key = `${wf}:${m}`;
    if (key !== renderedKey) {
      data = null;
      error = null;
    }
    loading = true;
    try {
      const g = await loadGraph(wf, m);
      if (my !== seq) return;
      data = g;
      error = null;
      await render(g, key);
    } catch (e) {
      if (my === seq) error = errorText(e);
    } finally {
      if (my === seq) loading = false;
    }
  }

  function buildStyle(large: boolean): StylesheetJson {
    const text = cssVar("--text");
    const edge = cssVar("--border-strong");
    const font = cssVar("--font");
    // Measure labels ourselves: cytoscape's deprecated `width: "label"` caches a 0 width for some
    // nodes on first render, leaving them invisible.
    const ctx = document.createElement("canvas").getContext("2d")!;
    ctx.font = `11px ${font}`;
    const style: StylesheetJson = [
      {
        selector: "node",
        style: {
          label: "data(label)",
          "font-size": 11,
          "font-family": font,
          color: text,
          "text-valign": "center",
          "text-halign": "center",
          "background-color": cssVar("--surface"),
          "border-width": 2,
          "border-color": edge,
          width: (n: NodeSingular) => ctx.measureText(n.data("label")).width + 20,
          height: 26,
          shape: "round-rectangle",
          "min-zoomed-font-size": 7,
        },
      },
      {
        selector: "node.file",
        style: {
          shape: "rhomboid",
          "border-color": cssVar("--info"),
          "border-style": "dashed",
          color: cssVar("--text-muted"),
        },
      },
      {
        selector: "node.userdata",
        style: {
          shape: "round-tag",
          "border-color": cssVar("--text-muted"),
          "border-style": "dashed",
          color: cssVar("--text-muted"),
        },
      },
      {
        selector: "edge",
        style: {
          width: 1.5,
          "line-color": edge,
          "target-arrow-color": edge,
          "target-arrow-shape": "triangle",
          "arrow-scale": 0.9,
          "curve-style": large ? "straight" : "bezier",
        },
      },
      {
        selector: "edge.input",
        style: {
          "line-color": cssVar("--success"),
          "target-arrow-color": cssVar("--success"),
          "line-style": "dashed",
        },
      },
      {
        selector: "edge.output",
        style: { "line-color": cssVar("--accent"), "target-arrow-color": cssVar("--accent") },
      },
      {
        selector: "node:active, node:selected",
        style: {
          "overlay-color": cssVar("--accent"),
          "overlay-opacity": 0.15,
          "overlay-padding": 4,
        },
      },
    ];
    for (const s of JOB_STATUSES) {
      const c = cssVar(STATUS_VAR[s]);
      style.push({
        selector: `node.job[status = "${s}"]`,
        style: { "border-color": c, "background-color": c, "background-opacity": 0.14 },
      });
    }
    return style;
  }

  function layoutFor(rankDir: "TB" | "LR"): LayoutOptions {
    return { name: "dagre", rankDir, nodeSep: 24, rankSep: 56, padding: 24 } as LayoutOptions;
  }

  async function render(g: GraphData, key: string) {
    if (g.nodeCount > HUGE && !forceHuge) {
      destroy();
      return;
    }
    // Same graph shape as last time: just update statuses in place (no relayout, no jump).
    const sig = g.elements.map((e) => e.data.id).join(",");
    if (cy && key === renderedKey && sig === renderedSig) {
      const c = cy;
      c.batch(() => {
        for (const e of g.elements) {
          if (e.data.status) c.getElementById(e.data.id!).data("status", e.data.status);
        }
      });
      return;
    }
    const token = seq;
    const large = g.nodeCount > LARGE;
    const cytoscape = await loadCytoscape();
    if (large) {
      // Let the "Laying out…" overlay paint before dagre blocks the main thread.
      layingOut = true;
      await new Promise((r) => requestAnimationFrame(() => setTimeout(r)));
    }
    await tick();
    layingOut = false;
    if (token !== seq) return; // a newer load superseded this one
    destroy();
    const c = cytoscape({
      container,
      elements: g.elements,
      style: buildStyle(large),
      layout: { name: "preset" },
      minZoom: 0.02,
      maxZoom: 3,
      wheelSensitivity: 0.3,
      boxSelectionEnabled: false,
      textureOnViewport: large,
      hideEdgesOnViewport: large,
      pixelRatio: large ? 1 : "auto",
    });
    c.on("tap", "node", (evt) => {
      const d = evt.target.data();
      if (d.kind === "job") app.openJob(d.jobId);
      else if (d.kind === "file" && d.path) app.openFile(d.path, d.name);
    });
    c.on("mouseover", "node", (evt) => {
      const d = evt.target.data();
      hovered = { name: d.name, status: d.status, kind: d.kind };
      if (container) container.style.cursor = d.kind === "userdata" ? "" : "pointer";
    });
    c.on("mouseout", "node", () => {
      hovered = null;
      if (container) container.style.cursor = "";
    });
    // Don't blow tiny graphs up to max zoom.
    c.one("layoutstop", () => {
      // Wide, shallow graphs (e.g. parameter sweeps) read better left-to-right.
      const bb = c.elements().boundingBox();
      if (!large && bb.w / bb.h > 2.5 * (c.width() / c.height())) {
        c.layout(layoutFor("LR")).run();
      }
      capZoom(c);
    });
    // Big workflows are usually wide parameter sweeps, so start those left-to-right.
    c.layout(layoutFor(large ? "LR" : "TB")).run();
    cy = c;
    renderedKey = key;
    renderedSig = sig;
  }

  function destroy() {
    cy?.destroy();
    cy = null;
    renderedKey = "";
    renderedSig = "";
  }

  function capZoom(c: Core) {
    if (c.zoom() > 1.2) {
      c.zoom(1.2);
      c.center();
    }
  }

  function fit() {
    if (!cy) return;
    cy.fit(undefined, 24);
    capZoom(cy);
  }

  function zoomBy(factor: number) {
    if (!cy) return;
    cy.zoom({
      level: cy.zoom() * factor,
      renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 },
    });
  }

  function renderAnyway() {
    forceHuge = true;
    if (data) void render(data, `${app.workflowId}:${mode}`);
  }

  // The canvas only exists while a workflow is selected; tie cytoscape's lifetime to it.
  $effect(() => {
    if (!container) return;
    const ro = new ResizeObserver(() => cy?.resize());
    ro.observe(container);
    return () => {
      ro.disconnect();
      destroy();
    };
  });

  onMount(() => {
    const off = onThemeChange(() => {
      if (cy) cy.style(buildStyle(cy.nodes().length > LARGE));
    });
    return () => {
      off();
      destroy();
    };
  });
</script>

<div class="page">
  <div class="page-header">
    {#if app.previousTab && app.previousTab !== "dag"}
      <button class="btn btn-sm btn-ghost" onclick={() => app.back()} title="Back (Backspace)">
        ← Back
      </button>
    {/if}
    <h2>Dependency graph</h2>
    <SegmentedTabs
      bind:value={mode}
      tabs={[
        { id: "jobs", label: "Jobs" },
        { id: "files", label: "Jobs + files" },
        { id: "userdata", label: "Jobs + user data" },
      ]}
    />
  </div>

  {#if app.workflowId === null}
    <div class="card empty">Pick a workflow in the top bar to see its dependency graph.</div>
  {:else}
    <div class="card canvas-card">
      <div class="canvas" bind:this={container}></div>

      {#if error && !data}
        <div class="overlay"><span class="error-text">Failed to load graph: {error}</span></div>
      {:else if !data}
        <div class="overlay"><span class="spinner"></span></div>
      {:else if data.nodeCount === 0}
        <div class="overlay muted">This workflow has no jobs.</div>
      {:else if layingOut}
        <div class="overlay muted"><span><span class="spinner"></span> Laying out graph…</span></div>
      {:else if tooBig}
        <div class="overlay">
          <div class="empty">
            <div>
              This graph has {data.nodeCount.toLocaleString()} nodes and {edgeCount.toLocaleString()}
              edges. Laying it out may freeze the page for a while.
            </div>
            <button class="btn" onclick={renderAnyway}>Render anyway</button>
          </div>
        </div>
      {/if}

      {#if data && data.nodeCount > 0 && !tooBig}
        <div class="controls">
          <button class="btn btn-sm btn-icon" title="Zoom in" onclick={() => zoomBy(1.25)}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
          </button>
          <button class="btn btn-sm btn-icon" title="Zoom out" onclick={() => zoomBy(0.8)}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14" /></svg>
          </button>
          <button class="btn btn-sm" title="Fit to screen" onclick={fit}>
            Fit
          </button>
        </div>

        <div class="info">
          {#if hovered}
            <strong>{hovered.name}</strong>
            <span class="muted">{hovered.status ? statusLabel(hovered.status) : hovered.kind}</span>
          {:else}
            <span class="faint">
              {data.nodeCount.toLocaleString()} nodes · {edgeCount.toLocaleString()} edges
              {#if loading}· refreshing…{/if}
            </span>
          {/if}
        </div>

        <div class="legend">
          {#each legendStatuses as s (s)}
            <span class="item">
              <span class="swatch" style:--c="var({STATUS_VAR[s]})"></span>
              {statusLabel(s)}
              <span class="faint">{data.statusCounts[s]}</span>
            </span>
          {/each}
          {#if mode === "files"}
            <span class="item"><span class="swatch file"></span>file</span>
          {:else if mode === "userdata"}
            <span class="item"><span class="swatch ud"></span>user data</span>
          {/if}
          {#if mode !== "jobs"}
            <span class="item"><span class="line"></span>produces</span>
            <span class="item"><span class="line dashed"></span>consumed by</span>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .canvas-card {
    position: relative;
    flex: 1;
    min-height: 320px;
    overflow: hidden;
  }
  .canvas {
    position: absolute;
    inset: 0;
  }
  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 24px;
    background: var(--surface);
  }
  .controls {
    position: absolute;
    top: 12px;
    right: 12px;
    display: flex;
    gap: 4px;
  }
  .controls svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }
  .info {
    position: absolute;
    top: 12px;
    left: 12px;
    display: flex;
    gap: 8px;
    align-items: baseline;
    max-width: calc(100% - 160px);
    padding: 4px 10px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    font-size: 12.5px;
    pointer-events: none;
  }
  .info strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .legend {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 12px;
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    width: fit-content;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    font-size: 12px;
    text-transform: capitalize;
  }
  .item {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .swatch {
    width: 12px;
    height: 12px;
    border-radius: 3px;
    border: 2px solid var(--c);
    background: color-mix(in srgb, var(--c) 14%, transparent);
  }
  .swatch.file {
    border: 2px dashed var(--info);
    transform: skewX(-15deg);
  }
  .swatch.ud {
    border: 2px dashed var(--text-muted);
  }
  .line {
    width: 18px;
    border-top: 2px solid var(--accent);
  }
  .line.dashed {
    border-top: 2px dashed var(--success);
  }
</style>
