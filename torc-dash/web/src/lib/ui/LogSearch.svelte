<!--
  Find-in-text for a log <pre>: highlights every match with the CSS Custom Highlight API (no DOM
  changes) and scrolls the current one into view. Enter / Shift+Enter: next / previous.

  <LogSearch target={pre} text={content} />
-->
<script lang="ts">
  let { target, text }: { target: HTMLElement | undefined; text: string } = $props();

  // ponytail: one global highlight name, so two visible searches share it; key per instance if needed.
  const MAX_MATCHES = 10_000;
  const supported = typeof CSS !== "undefined" && "highlights" in CSS;

  let query = $state("");
  let ranges = $state.raw<Range[]>([]);
  let current = $state(0);

  // Recompute matches when the query, element, or text changes. Matching runs on the element's
  // whole text, so a match can span syntax-highlighting spans; a case-insensitive regex (rather
  // than lowercasing) keeps offsets aligned with the original text.
  $effect(() => {
    const q = query;
    const el = target;
    void text;
    const found: Range[] = [];
    if (q && el) {
      const nodes: Text[] = [];
      const starts: number[] = [];
      let all = "";
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        nodes.push(n as Text);
        starts.push(all.length);
        all += n.textContent;
      }
      /** DOM position of a character offset into `all` (the last node starting at or before it). */
      const at = (offset: number): [Text, number] => {
        let lo = 0;
        let hi = nodes.length - 1;
        while (lo < hi) {
          const mid = (lo + hi + 1) >> 1;
          if (starts[mid] <= offset) lo = mid;
          else hi = mid - 1;
        }
        return [nodes[lo], offset - starts[lo]];
      };
      const re = new RegExp(q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
      for (let m = re.exec(all); m && found.length < MAX_MATCHES; m = re.exec(all)) {
        const r = document.createRange();
        r.setStart(...at(m.index));
        r.setEnd(...at(m.index + m[0].length));
        found.push(r);
      }
    }
    ranges = found;
    current = 0;
  });

  $effect(() => {
    if (!supported) return;
    const all = ranges;
    const cur = all[current];
    CSS.highlights.set("log-search", new Highlight(...all));
    if (cur) CSS.highlights.set("log-search-current", new Highlight(cur));
    else CSS.highlights.delete("log-search-current");
    return () => {
      CSS.highlights.delete("log-search");
      CSS.highlights.delete("log-search-current");
    };
  });

  // Scroll the current match into the middle of the target.
  $effect(() => {
    const r = ranges[current];
    const el = target;
    if (!r || !el) return;
    const box = r.getBoundingClientRect();
    const view = el.getBoundingClientRect();
    el.scrollTop += box.top - view.top - el.clientHeight / 2;
    el.scrollLeft += Math.max(0, box.right - view.right + 40) + Math.min(0, box.left - view.left - 40);
  });

  function step(delta: number) {
    if (ranges.length) current = (current + delta + ranges.length) % ranges.length;
  }
</script>

<div class="search">
  <input
    class="input"
    type="search"
    placeholder="Find…"
    bind:value={query}
    onkeydown={(e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        step(e.shiftKey ? -1 : 1);
      } else if (e.key === "Escape" && query) {
        e.stopPropagation(); // clear the search, don't close the dialog
        e.preventDefault();
        query = "";
      }
    }}
  />
  {#if query}
    <span class="count">
      {ranges.length ? `${current + 1}/${ranges.length}${ranges.length === MAX_MATCHES ? "+" : ""}` : "0"}
    </span>
    <button class="btn btn-sm btn-ghost btn-icon" title="Previous (Shift+Enter)" disabled={!ranges.length} onclick={() => step(-1)}>↑</button>
    <button class="btn btn-sm btn-ghost btn-icon" title="Next (Enter)" disabled={!ranges.length} onclick={() => step(1)}>↓</button>
  {/if}
</div>

<style>
  .search {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .input {
    width: 160px;
    height: 28px;
    padding: 0 8px;
    font-size: 12.5px;
  }
  .count {
    min-width: 44px;
    text-align: right;
    font-size: 12px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  :global(::highlight(log-search)) {
    background-color: rgb(250 204 21 / 0.35);
  }
  :global(::highlight(log-search-current)) {
    background-color: rgb(249 115 22 / 0.75);
    color: #000;
  }
</style>
