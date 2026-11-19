/** Syntax highlighting for JSON and YAML: escaped HTML with token spans (see app.css `.hl`). */

export const escapeHtml = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

const JSON_TOKEN =
  /("(?:\\.|[^"\\])*")(\s*:)?|\b(true|false|null)\b|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|([{}[\]])/g;

/** Escape and wrap JSON tokens in spans (.k key, .s string, .n number, .b bool/null, .p punct). */
export function highlightJson(json: string): string {
  let out = "";
  let last = 0;
  for (const m of json.matchAll(JSON_TOKEN)) {
    out += escapeHtml(json.slice(last, m.index));
    const [tok, str, colon, lit, num] = m;
    if (str !== undefined) {
      out += colon
        ? `<span class="k">${escapeHtml(str)}</span>${colon}`
        : `<span class="s">${escapeHtml(str)}</span>`;
    } else if (lit) out += `<span class="b">${lit}</span>`;
    else if (num) out += `<span class="n">${num}</span>`;
    else out += `<span class="p">${tok}</span>`;
    last = m.index + tok.length;
  }
  return out + escapeHtml(json.slice(last));
}

// YAML: per line, `indent [- ]key: value # comment`; block scalars (| or >) continue as strings.
const YAML_KEY = /^(\s*)((?:- +)*)((?:"(?:\\.|[^"\\])*"|'(?:[^']|'')*'|[^\s#'"[{-][^:#]*?|-[^\s:#][^:#]*?)\s*:)(?=\s|$)/;
const YAML_SCALAR =
  /^(?:(true|false|null|yes|no|on|off|~)|(-?(?:\d[\d_]*(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?|0x[\da-fA-F]+)|([|>][-+\d]*)|([&*!]\S+))$/i;

/** Index of a `#` comment start outside quotes, or -1. */
function commentStart(s: string): number {
  let quote = "";
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (quote) {
      if (c === "\\" && quote === '"') i++;
      else if (c === quote) quote = "";
    } else if (c === '"' || c === "'") quote = c;
    else if (c === "#" && (i === 0 || /\s/.test(s[i - 1]))) return i;
  }
  return -1;
}

function yamlValue(v: string): string {
  const t = v.trim();
  if (!t) return escapeHtml(v);
  const lead = v.slice(0, v.indexOf(t[0]));
  const m = t.match(YAML_SCALAR);
  let body: string;
  if (t.startsWith("[") || t.startsWith("{")) body = highlightJson(t);
  else if (m?.[1]) body = `<span class="b">${escapeHtml(t)}</span>`;
  else if (m?.[2]) body = `<span class="n">${escapeHtml(t)}</span>`;
  else if (m?.[3] || m?.[4]) body = `<span class="p">${escapeHtml(t)}</span>`;
  else body = `<span class="s">${escapeHtml(t)}</span>`;
  return escapeHtml(lead) + body + escapeHtml(v.slice(lead.length + t.length));
}

/** Escape and wrap YAML tokens in the same span classes as highlightJson (plus .c comment). */
export function highlightYaml(yaml: string): string {
  let blockIndent = -1; // inside a | or > block scalar whose parent line had this indent
  return yaml
    .split("\n")
    .map((line) => {
      const indent = line.length - line.trimStart().length;
      if (blockIndent >= 0) {
        if (!line.trim() || indent > blockIndent) return `<span class="s">${escapeHtml(line)}</span>`;
        blockIndent = -1;
      }
      if (/^(---|\.\.\.)\s*$/.test(line)) return `<span class="p">${escapeHtml(line)}</span>`;
      const hash = commentStart(line);
      const code = hash >= 0 ? line.slice(0, hash) : line;
      const comment = hash >= 0 ? `<span class="c">${escapeHtml(line.slice(hash))}</span>` : "";
      const m = code.match(YAML_KEY);
      let out: string;
      let value: string;
      if (m) {
        const [whole, ws, dashes, key] = m;
        const colon = key.lastIndexOf(":");
        out =
          ws +
          (dashes ? `<span class="p">${dashes}</span>` : "") +
          `<span class="k">${escapeHtml(key.slice(0, colon))}</span>${escapeHtml(key.slice(colon))}`;
        value = code.slice(whole.length);
      } else {
        const d = code.match(/^(\s*)((?:- +|-$)*)/)!;
        out = d[1] + (d[2] ? `<span class="p">${d[2]}</span>` : "");
        value = code.slice(d[0].length);
      }
      if (/^\s*[|>][-+\d]*\s*$/.test(value)) blockIndent = indent;
      return out + yamlValue(value) + comment;
    })
    .join("\n");
}

/** Above this size, highlighting costs seconds; show plain text. */
export const MAX_HIGHLIGHT = 1_000_000;

/** Highlighted HTML for a spec or data file by extension (".json", "x.yaml", …); null: show plain. */
export function highlightByName(text: string, name: string): string | null {
  const ext = name.slice(name.lastIndexOf(".")).toLowerCase();
  if (text.length > MAX_HIGHLIGHT) return null;
  if (ext === ".yaml" || ext === ".yml") return highlightYaml(text);
  if (ext === ".json" || ext === ".json5") return highlightJson(text);
  return null;
}

/** JSON re-indented for display when it parses; otherwise unchanged. */
export function prettyJson(text: string): string {
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
}

