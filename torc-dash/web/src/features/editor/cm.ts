/** CodeMirror setup for the spec editor: YAML, dashboard colors, and schema completion/hover/lint. */
import { autocompletion, type CompletionContext } from "@codemirror/autocomplete";
import { indentWithTab } from "@codemirror/commands";
import { yaml } from "@codemirror/lang-yaml";
import { HighlightStyle, indentUnit, syntaxHighlighting } from "@codemirror/language";
import { linter, lintGutter } from "@codemirror/lint";
import { hoverTooltip, keymap, EditorView } from "@codemirror/view";
import { tags as t } from "@lezer/highlight";
import { basicSetup } from "codemirror";
import { SpecSchema } from "./yamlSchema";

const theme = EditorView.theme({
  "&": { height: "100%", fontSize: "13px", backgroundColor: "var(--surface)", color: "var(--text)" },
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

/** Paragraphs of a schema description, minus Rust doc-comment line wrapping. */
function prose(text: string): string {
  return text.replace(/([^\n])\n(?=[^\n-*])/g, "$1 ");
}

function schemaSupport(schema: SpecSchema) {
  return [
    autocompletion({
      override: [
        (ctx: CompletionContext) => {
          const r = schema.complete(ctx.state.doc.toString(), ctx.pos);
          // Pop up on its own while typing a word, or right after "key: "; Ctrl+Space anywhere.
          if (!r || (!ctx.explicit && r.from === ctx.pos && !ctx.matchBefore(/:\s$/))) return null;
          return {
            from: r.from,
            to: r.to,
            validFor: /^[\w.-]*$/,
            options: r.options.map((o) => ({
              label: o.label,
              detail: o.detail,
              info: o.info ? prose(o.info) : undefined,
              apply: o.apply,
              type: r.kind === "key" ? "property" : "enum",
            })),
          };
        },
      ],
    }),
    hoverTooltip((view, pos) => {
      const h = schema.hover(view.state.doc.toString(), pos);
      if (!h) return null;
      return {
        pos: h.from,
        end: h.to,
        above: true,
        create() {
          const dom = document.createElement("div");
          dom.className = "cm-hover";
          const title = dom.appendChild(document.createElement("code"));
          title.textContent = h.title;
          const type = dom.appendChild(document.createElement("span"));
          type.className = "type";
          type.textContent = h.type;
          if (h.info) dom.appendChild(document.createElement("p")).textContent = prose(h.info);
          return { dom };
        },
      };
    }),
    linter((view) => schema.lint(view.state.doc.toString()), { delay: 400 }),
  ];
}

/** Extensions for the editor; schema features are left out when the schema couldn't be loaded. */
export function specExtensions(schema: SpecSchema | null, onChange: (text: string) => void) {
  return [
    basicSetup,
    keymap.of([indentWithTab]),
    indentUnit.of("  "),
    yaml(),
    theme,
    syntaxHighlighting(colors),
    lintGutter(),
    schema ? schemaSupport(schema) : [],
    EditorView.updateListener.of((u) => {
      if (u.docChanged) onChange(u.state.doc.toString());
    }),
  ];
}
