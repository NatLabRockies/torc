// The spec editor: CodeMirror (vendored in codemirror.js), loaded by shell.js on first use of the
// editor tab. The text lives in the $editor.text signal; a hidden text area bound to it carries
// edits in both directions. Completion, hover help, and lint come from torc-dash, which applies
// the workflow-spec schema and the spec parser in Rust (/ui/editor/complete, /hover, /lint).
import {
  autocompletion,
  basicSetup,
  Compartment,
  EditorState,
  EditorView,
  forceLinting,
  HighlightStyle,
  hoverTooltip,
  indentUnit,
  indentWithTab,
  insertCompletionText,
  json,
  keymap,
  lintGutter,
  linter,
  pickedCompletion,
  syntaxHighlighting,
  tags as t,
  yaml,
} from "./codemirror.js";

const theme = EditorView.theme({
  "&": { height: "100%", fontSize: "13px", backgroundColor: "var(--surface)", color: "var(--text)" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--mono)", lineHeight: "1.55" },
  ".cm-content": { caretColor: "var(--text)" },
  ".cm-cursor": { borderLeftColor: "var(--text)" },
  ".cm-gutters": { backgroundColor: "var(--surface-2)", color: "var(--text-faint)", borderRight: "1px solid var(--border)" },
  ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "color-mix(in srgb, var(--accent) 6%, transparent)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "color-mix(in srgb, var(--accent) 22%, transparent) !important",
  },
  ".cm-tooltip": {
    backgroundColor: "var(--surface)",
    color: "var(--text)",
    border: "1px solid var(--border-strong)",
    borderRadius: "var(--radius-sm)",
    boxShadow: "var(--shadow-md)",
  },
  ".cm-tooltip-autocomplete ul li[aria-selected]": { backgroundColor: "var(--accent)", color: "var(--accent-text)" },
  ".cm-completionDetail": { color: "inherit", opacity: "0.7", fontStyle: "normal", marginLeft: "1em" },
  ".cm-completionInfo": { maxWidth: "420px", whiteSpace: "pre-wrap", padding: "8px 10px" },
  ".cm-hover": { maxWidth: "480px", padding: "8px 10px", fontSize: "12.5px" },
  ".cm-hover code": { fontFamily: "var(--mono)" },
  ".cm-hover .type": { color: "var(--text-muted)", marginLeft: "8px" },
  ".cm-hover p": { margin: "6px 0 0", whiteSpace: "pre-wrap" },
  ".cm-panels": { backgroundColor: "var(--surface-2)", color: "var(--text)" },
  ".cm-searchMatch": { backgroundColor: "color-mix(in srgb, var(--warning) 30%, transparent)" },
  ".cm-foldPlaceholder": { backgroundColor: "var(--neutral-soft)", border: "none", color: "var(--text-muted)" },
});

// Same palette as the read-only highlighter (.hl in app.css).
const colors = HighlightStyle.define([
  { tag: [t.propertyName, t.definition(t.propertyName)], color: "var(--accent)" },
  { tag: [t.string, t.special(t.string), t.content], color: "var(--success)" },
  { tag: t.number, color: "var(--warning)" },
  { tag: [t.bool, t.null, t.keyword, t.atom], color: "var(--info)" },
  { tag: [t.comment, t.lineComment], color: "var(--text-muted)", fontStyle: "italic" },
  { tag: [t.punctuation, t.separator, t.meta], color: "var(--text-muted)" },
]);

const language = (format) => (format === "yaml" ? yaml() : format === "kdl" ? [] : json());

/** Paragraphs of a schema description, minus Rust doc-comment line wrapping. */
const prose = (text) => text.replace(/([^\n])\n(?=[^\n\-*])/g, "$1 ");

const ask = (path, body) =>
  fetch(path, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) })
    .then((r) => (r.ok ? r.json() : null))
    .catch(() => null);

/** Create the editor in `host`, starting from the bound text area; returns its controls. */
export function mount(host) {
  const area = document.getElementById("editor-text");
  let format = document.getElementById("editor-format")?.value || "yaml";
  let readOnly = area.readOnly;
  let syncing = false;
  const lang = new Compartment();
  const ro = new Compartment();

  const complete = async (ctx) => {
    if (format !== "yaml") return null;
    const r = await ask("/ui/editor/complete", { text: ctx.state.doc.toString(), pos: ctx.pos });
    // Pop up on its own while typing a word, or right after "key: "; Ctrl+Space anywhere.
    if (!r || (!ctx.explicit && r.from === ctx.pos && !ctx.matchBefore(/:\s$/))) return null;
    return {
      from: r.from,
      validFor: /^[\w.-]*$/,
      options: r.options.map((o) => ({
        label: o.label,
        detail: o.detail,
        info: o.info ? prose(o.info) : undefined,
        type: r.kind === "key" ? "property" : "enum",
        // Filter on the text before the cursor, but replace the whole word when picked.
        apply: (view, c, from, to) => {
          const line = view.state.doc.lineAt(to);
          const end = to + /^[\w.-]*/.exec(line.text.slice(to - line.from))[0].length;
          view.dispatch({ ...insertCompletionText(view.state, o.apply, from, end), annotations: pickedCompletion.of(c) });
        },
      })),
    };
  };

  const hover = async (view, pos) => {
    if (format !== "yaml") return null;
    const h = await ask("/ui/editor/hover", { text: view.state.doc.toString(), pos });
    if (!h) return null;
    return {
      pos: h.from,
      end: h.to,
      above: true,
      create() {
        const dom = document.createElement("div");
        dom.className = "cm-hover";
        dom.appendChild(document.createElement("code")).textContent = h.title;
        const type = dom.appendChild(document.createElement("span"));
        type.className = "type";
        type.textContent = h.type;
        if (h.info) dom.appendChild(document.createElement("p")).textContent = prose(h.info);
        return { dom };
      },
    };
  };

  const lint = async (view) =>
    (await ask("/ui/editor/lint", { text: view.state.doc.toString(), format })) || [];

  const view = new EditorView({
    parent: host,
    doc: area.value,
    extensions: [
      basicSetup,
      keymap.of([indentWithTab]),
      indentUnit.of("  "),
      theme,
      syntaxHighlighting(colors),
      lang.of(language(format)),
      ro.of(EditorState.readOnly.of(readOnly)),
      autocompletion({ override: [complete] }),
      hoverTooltip(hover),
      lintGutter(),
      linter(lint, { delay: 500 }),
      EditorView.contentAttributes.of({ "aria-label": "Workflow spec" }),
      EditorView.updateListener.of((u) => {
        if (!u.docChanged || syncing) return;
        area.value = u.state.doc.toString();
        area.dispatchEvent(new Event("input", { bubbles: true }));
      }),
    ],
  });

  return {
    /** Follow the signals: text replaced by the server (templates, files, the assistant). */
    sync(text, f, ro_) {
      const effects = [];
      if (f !== format) effects.push(lang.reconfigure(language((format = f))));
      if (!!ro_ !== readOnly) effects.push(ro.reconfigure(EditorState.readOnly.of((readOnly = !!ro_))));
      const cur = view.state.doc.toString();
      const changes = typeof text === "string" && text !== cur ? { from: 0, to: cur.length, insert: text } : undefined;
      if (!effects.length && !changes) return;
      syncing = true;
      view.dispatch({ changes, effects });
      syncing = false;
      if (effects.length) forceLinting(view);
    },
    select(from, to) {
      const n = view.state.doc.length;
      view.dispatch({ selection: { anchor: Math.min(from, n), head: Math.min(to, n) }, scrollIntoView: true });
      view.focus();
    },
    cursor: () => view.state.selection.main.head,
  };
}
