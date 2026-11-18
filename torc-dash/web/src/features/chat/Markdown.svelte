<!-- Markdown rendered with marked (lazy-loaded) and sanitized with DOMPurify. -->
<script lang="ts" module>
  type Render = (text: string) => string;

  /**
   * Assistant text is untrusted: a reply with ![](/api/cli/run-stream?...) would otherwise make a
   * dashboard request just by being shown. Allow only Markdown elements that fetch nothing, and
   * links only to absolute http(s) or mailto URLs (a relative link would target the dashboard).
   */
  const SANITIZE = {
    ALLOWED_TAGS: [
      "p", "br", "hr", "h1", "h2", "h3", "h4", "h5", "h6", "strong", "em", "del", "code", "pre",
      "blockquote", "ul", "ol", "li", "a", "table", "thead", "tbody", "tr", "th", "td",
    ],
    ALLOWED_ATTR: ["href", "title", "start", "align", "class"],
    ALLOWED_URI_REGEXP: /^(?:https?:|mailto:)/i,
  };
  let renderer: Promise<Render> | null = null;

  function loadRenderer(): Promise<Render> {
    renderer ??= Promise.all([import("marked"), import("dompurify")]).then(
      ([{ Marked }, { default: DOMPurify }]) => {
        const marked = new Marked({ breaks: true, gfm: true, async: false });
        DOMPurify.addHook("afterSanitizeAttributes", (node) => {
          if (node.tagName === "A") {
            node.setAttribute("target", "_blank");
            node.setAttribute("rel", "noopener noreferrer");
          }
          if (node.tagName === "PRE") node.classList.add("code-block");
        });
        return (text) => DOMPurify.sanitize(marked.parse(text) as string, SANITIZE);
      },
    );
    return renderer;
  }
</script>

<script lang="ts">
  let { text }: { text: string } = $props();
  let render = $state<Render | null>(null);

  $effect(() => {
    loadRenderer().then((r) => (render = r));
  });
</script>

{#if render}
  <!-- Sanitized by DOMPurify. -->
  <div class="md">{@html render(text)}</div>
{:else}
  <div class="md plain">{text}</div>
{/if}

<style>
  .md {
    overflow-wrap: anywhere;
  }
  .plain {
    white-space: pre-wrap;
  }
  .md :global(:first-child) {
    margin-top: 0;
  }
  .md :global(:last-child) {
    margin-bottom: 0;
  }
  .md :global(p),
  .md :global(ul),
  .md :global(ol),
  .md :global(blockquote),
  .md :global(table) {
    margin: 0 0 10px;
  }
  .md :global(ul),
  .md :global(ol) {
    padding-left: 22px;
  }
  .md :global(li + li) {
    margin-top: 2px;
  }
  .md :global(h1),
  .md :global(h2),
  .md :global(h3),
  .md :global(h4) {
    margin: 14px 0 8px;
    font-size: 15px;
  }
  .md :global(pre.code-block) {
    margin: 0 0 10px;
    white-space: pre;
  }
  .md :global(pre code) {
    font-size: inherit;
    background: none;
    padding: 0;
  }
  .md :global(:not(pre) > code) {
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--neutral-soft);
    font-size: 0.88em;
  }
  .md :global(blockquote) {
    padding-left: 12px;
    border-left: 3px solid var(--border-strong);
    color: var(--text-muted);
  }
  .md :global(table) {
    border-collapse: collapse;
    display: block;
    overflow-x: auto;
    font-size: 13px;
  }
  .md :global(th),
  .md :global(td) {
    padding: 5px 10px;
    border: 1px solid var(--border);
    text-align: left;
  }
  .md :global(th) {
    background: var(--surface-2);
  }
  .md :global(hr) {
    border: 0;
    border-top: 1px solid var(--border);
    margin: 12px 0;
  }
</style>
