<!-- Renders one Plotly figure (as produced by `torc plot-resources`) using the current theme. -->
<script lang="ts" module>
  import type PlotlyType from "plotly.js-dist-min";
  type ReactArgs = Parameters<(typeof PlotlyType)["react"]>;

  let plotlyPromise: Promise<typeof PlotlyType> | null = null;
  const loadPlotly = () => (plotlyPromise ??= import("plotly.js-dist-min").then((m) => m.default));

  export type Figure = { data?: unknown; layout?: Record<string, unknown> } | unknown[];
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { errorText } from "../../lib/format";
  import { cssVar, onThemeChange } from "../dag/theme";

  let { figure }: { figure: Figure } = $props();

  let el: HTMLDivElement;
  let error = $state<string | null>(null);
  let themeTick = $state(0);

  type Obj = Record<string, unknown>;

  /** Copy of the figure's layout with backgrounds, fonts, and gridlines from the app theme. */
  function themedLayout(layout: Obj | undefined): Obj {
    const text = cssVar("--text");
    const grid = cssVar("--border");
    const l: Obj = structuredClone(layout ?? {});
    l.paper_bgcolor = "rgba(0,0,0,0)";
    l.plot_bgcolor = "rgba(0,0,0,0)";
    l.font = { ...(l.font as Obj), color: text, family: cssVar("--font") };
    l.legend = { ...(l.legend as Obj), bgcolor: "rgba(0,0,0,0)" };
    l.hoverlabel = { bgcolor: cssVar("--surface"), bordercolor: grid, font: { color: text } };
    for (const key of Object.keys(l)) {
      if (/^[xy]axis\d*$/.test(key)) {
        l[key] = {
          ...(l[key] as Obj),
          gridcolor: grid,
          linecolor: cssVar("--border-strong"),
          zerolinecolor: grid,
        };
      }
    }
    for (const axis of ["xaxis", "yaxis"]) l[axis] ??= { gridcolor: grid, zerolinecolor: grid };
    l.autosize = true;
    l.margin ??= { l: 60, r: 60, t: 60, b: 50 };
    delete l.width;
    delete l.height;
    return l;
  }

  $effect(() => {
    void themeTick;
    const fig = figure;
    let cancelled = false;
    loadPlotly()
      .then((Plotly) => {
        if (cancelled || !el) return;
        const data = (Array.isArray(fig) ? fig : fig.data) as ReactArgs[1];
        const layout = themedLayout(Array.isArray(fig) ? undefined : fig.layout);
        error = null;
        return Plotly.react(el, data ?? [], layout as ReactArgs[2], {
          responsive: true,
          displaylogo: false,
        });
      })
      .catch((e) => (error = errorText(e)));
    return () => {
      cancelled = true;
    };
  });

  onMount(() => {
    const node = el;
    const off = onThemeChange(() => themeTick++);
    const ro = new ResizeObserver(() => {
      if (plotlyPromise) void plotlyPromise.then((P) => node.isConnected && P.Plots.resize(node));
    });
    ro.observe(node);
    return () => {
      off();
      ro.disconnect();
      void plotlyPromise?.then((P) => P.purge(node));
    };
  });
</script>

{#if error}<div class="error-text">Error rendering plot: {error}</div>{/if}
<div class="plot" bind:this={el}></div>

<style>
  .plot {
    flex: 1;
    min-height: 420px;
    width: 100%;
  }
</style>
