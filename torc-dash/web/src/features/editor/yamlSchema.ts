/**
 * Workflow-spec JSON Schema applied to YAML text: what to complete at the cursor, what a key means
 * (hover), and where the spec breaks the schema (lint). No CodeMirror here, so it runs under Node
 * for tests; see cm.ts for the editor wiring.
 *
 * Paths come from indentation, not from a parse, so they work while the document is half-typed.
 * ponytail: block-style YAML only. Flow collections ({a: 1}, [x, y]) get no completion or hover;
 * lint still covers them because it validates the parsed document.
 */
import { Validator } from "@cfworker/json-schema";
import { isMap, isPair, isScalar, isSeq, parseDocument, type Node as YamlNode } from "yaml";

export type Schema = Record<string, any>;
/** Map keys, with "*" for "an item of this sequence". */
export type Path = string[];

export type Completion = { label: string; detail: string; info: string; apply: string };
export type Diagnostic = { from: number; to: number; message: string; severity: "error" | "warning" };

const KEY = /^([\w.-]+)\s*:(?=\s|$)/;

/** One line split into its sequence dashes and key: "  - name: x" → dashes [2], key at 4. */
function parseLine(line: string): { indent: number; dashes: number[]; key?: string; keyCol: number; rest: string } {
  let col = line.length - line.trimStart().length;
  const indent = col;
  const dashes: number[] = [];
  while (line[col] === "-" && (line[col + 1] === " " || col + 1 === line.length)) {
    dashes.push(col);
    col += 1;
    while (line[col] === " ") col++;
  }
  const m = KEY.exec(line.slice(col));
  return { indent, dashes, key: m?.[1], keyCol: col, rest: m ? line.slice(col + m[0].length) : "" };
}

/** Path of the mapping holding the key on a parsed line (through its own dashes, if any). */
function linePath(lines: string[], li: number, p: { dashes: number[]; keyCol: number }): Path | null {
  if (!p.dashes.length) return containerPath(lines, li, p.keyCol, false);
  const path = containerPath(lines, li, p.dashes[0], true);
  return path && [...path, ...p.dashes.map(() => "*")];
}

const blank = (l: string) => !l.trim() || l.trimStart().startsWith("#");

/**
 * Path of the mapping that contains column `col` on line `li`. `seq` means the column holds a
 * sequence dash, so a key at the same column above (`jobs:` / `- name`) is its parent.
 * Returns null inside a block scalar (`command: |`), where keys are plain text.
 */
function containerPath(lines: string[], li: number, col: number, seq: boolean): Path | null {
  const path: Path = [];
  for (let i = li - 1; i >= 0 && (col > 0 || seq); i--) {
    if (blank(lines[i])) continue;
    const p = parseLine(lines[i]);
    if (p.indent > col || (p.indent === col && !seq)) continue;
    if (p.indent === col) {
      // seq: a dash here is a sibling item; a key here owns the sequence.
      if (p.dashes.length || !p.key) continue;
      if (/^\s*[|>]/.test(p.rest)) return null;
      path.push(p.key);
      seq = false;
      continue;
    }
    // Elements on this line left of `col`, innermost first, are our ancestors.
    if (p.key && p.keyCol < col) {
      if (/^\s*[|>]/.test(p.rest)) return null;
      path.push(p.key);
    }
    for (let d = p.dashes.length - 1; d >= 0; d--) if (p.dashes[d] < col) path.push("*");
    col = p.indent;
    seq = p.dashes.length > 0;
  }
  return path.reverse();
}

/** Keys already present in the mapping whose keys sit at `col`, around line `li`. */
function siblingKeys(lines: string[], li: number, col: number): Set<string> {
  const keys = new Set<string>();
  const visit = (i: number) => {
    if (blank(lines[i])) return true;
    const p = parseLine(lines[i]);
    if (p.key && p.keyCol === col) keys.add(p.key);
    return p.indent >= col; // a shallower line ends the mapping (after counting "- key" on it)
  };
  for (let i = li - 1; i >= 0 && visit(i); i--);
  for (let i = li + 1; i < lines.length && visit(i); i++);
  return keys;
}

function lineAt(text: string, pos: number) {
  const start = text.lastIndexOf("\n", pos - 1) + 1;
  const lines = text.split("\n");
  const li = text.slice(0, start).split("\n").length - 1;
  return { lines, li, col: pos - start };
}

export class SpecSchema {
  readonly #validator: Validator;
  readonly root: Schema;

  constructor(root: Schema) {
    this.root = root;
    this.#validator = new Validator(root as never, "2020-12", false);
  }

  /** Follow $refs and unwrap Option (`anyOf: [T, null]`). */
  resolve(s: Schema | undefined): Schema | undefined {
    for (let i = 0; s && i < 16; i++) {
      if (typeof s.$ref === "string") s = this.root.$defs?.[s.$ref.replace("#/$defs/", "")];
      else {
        const alts = (s.anyOf ?? s.oneOf)?.filter((a: Schema) => a.type !== "null");
        if (alts?.length !== 1) return s;
        s = alts[0];
      }
    }
    return s;
  }

  /** Raw (unresolved) schema of the value at `path`, keeping the property's own description. */
  at(path: Path): Schema | undefined {
    let s: Schema | undefined = this.root;
    for (const seg of path) {
      const r = this.resolve(s);
      s = seg === "*" ? r?.items : (r?.properties?.[seg] ?? (typeof r?.additionalProperties === "object" ? r.additionalProperties : undefined));
      if (!s) return undefined;
    }
    return s;
  }

  describe(s: Schema): string {
    return s.description ?? this.resolve(s)?.description ?? "";
  }

  typeLabel(s: Schema): string {
    const ref = (s.$ref ?? (s.anyOf ?? s.oneOf)?.find((a: Schema) => a.$ref)?.$ref) as string | undefined;
    const r = this.resolve(s) ?? {};
    if (r.type === "array" || (Array.isArray(r.type) && r.type.includes("array"))) {
      const item = r.items ? this.typeLabel(r.items) : "any";
      return `list of ${item}`;
    }
    if (ref && !this.values(s).length) return ref.replace("#/$defs/", "").replace(/Spec$/, "");
    if (this.values(s).length) return this.values(s).join(" | ");
    const t = [r.type].flat().filter((x) => x && x !== "null");
    return t.join(" | ") || "any";
  }

  /** Allowed scalar values: enum, oneOf consts, or booleans. */
  values(s: Schema): string[] {
    const r = this.resolve(s) ?? {};
    if (Array.isArray(r.enum)) return r.enum.filter((v: unknown) => v !== null).map(String);
    const alts = r.oneOf ?? r.anyOf;
    if (alts?.every((a: Schema) => "const" in a || a.type === "null"))
      return alts.filter((a: Schema) => "const" in a).map((a: Schema) => String(a.const));
    if ([r.type].flat().includes("boolean")) return ["true", "false"];
    return [];
  }

  /**
   * Completions at `pos`: keys of the enclosing mapping, or allowed values after `key: `. The
   * range `from`..`to` covers the whole word at the cursor, so accepting one replaces it.
   */
  complete(
    text: string,
    pos: number,
  ): { from: number; to: number; kind: "key" | "value"; options: Completion[] } | null {
    const { lines, li, col } = lineAt(text, pos);
    const before = lines[li].slice(0, col);
    const after = lines[li].slice(col);
    const to = pos + /^[\w.-]*/.exec(after)![0].length;

    const value = /^(\s*(?:-\s+)*)([\w.-]+):\s+([\w.-]*)$/.exec(before);
    if (value) {
      const path = linePath(lines, li, parseLine(before));
      if (!path) return null;
      const s = this.at([...path, value[2]]);
      const vals = s ? this.values(s) : [];
      if (!vals.length) return null;
      const info = s ? this.describe(s) : "";
      return {
        from: pos - value[3].length,
        to,
        kind: "value",
        options: vals.map((v) => ({ label: v, detail: "", info, apply: v })),
      };
    }

    const key = /^(\s*)((?:-\s+)*)([\w.-]*)$/.exec(before);
    if (!key) return null;
    const keyCol = key[1].length + key[2].length;
    const path = linePath(lines, li, { dashes: parseLine(before).dashes, keyCol });
    if (!path) return null;

    const container = this.resolve(this.at(path));
    const props: Record<string, Schema> | undefined = container?.properties;
    if (!props) return null;
    const present = siblingKeys(lines, li, keyCol);
    const pad = " ".repeat(keyCol + 2);
    // Renaming an existing key (`nam|: x`): insert only the name, keeping its separator and value.
    const hasColon = /^\s*:/.test(after.slice(to - pos));
    const options = Object.entries(props)
      .filter(([k]) => !present.has(k) || k === key[3])
      .map(([k, s]) => {
        const r = this.resolve(s) ?? {};
        const types = [r.type].flat();
        const nested = types.includes("array") ? `\n${pad}- ` : types.includes("object") && r.properties ? `\n${pad}` : " ";
        const apply = hasColon ? k : `${k}:${nested}`;
        return { label: k, detail: this.typeLabel(s), info: this.describe(s), apply };
      });
    const required = new Set<string>(container?.required ?? []);
    options.sort((a, b) => Number(required.has(b.label)) - Number(required.has(a.label)));
    return { from: pos - key[3].length, to, kind: "key", options };
  }

  /** The key under `pos`, its range, type, and description. */
  hover(text: string, pos: number): { from: number; to: number; title: string; type: string; info: string } | null {
    const { lines, li, col } = lineAt(text, pos);
    const p = parseLine(lines[li]);
    if (!p.key || col < p.keyCol || col > p.keyCol + p.key.length) return null;
    const path = linePath(lines, li, p);
    if (!path) return null;
    const s = this.at([...path, p.key]);
    if (!s) return null;
    const from = pos - col + p.keyCol;
    return { from, to: from + p.key.length, title: p.key, type: this.typeLabel(s), info: this.describe(s) };
  }

  /** YAML syntax errors, then schema violations, with offsets into `text`. */
  lint(text: string): Diagnostic[] {
    const doc = parseDocument(text, { prettyErrors: false });
    const out: Diagnostic[] = doc.errors.map((e) => ({
      from: e.pos[0],
      to: Math.max(e.pos[1], e.pos[0] + 1),
      message: e.message.split("\n")[0],
      severity: "error" as const,
    }));
    if (out.length || !text.trim()) return out;

    const errors = this.#validator.validate(doc.toJS()).errors;
    const locs = errors.map((e) => e.instanceLocation);
    const hasDeeper = (loc: string) => locs.some((l) => l.startsWith(loc + "/"));
    const seen = new Set<string>();
    const add = (loc: string, message: string, onKey = false) => {
      if (seen.has(loc + message)) return;
      seen.add(loc + message);
      const r = nodeRange(doc.contents, pointer(loc), onKey, text);
      out.push({ from: r[0], to: r[1], message, severity: "error" });
    };

    const byLoc = new Map<string, typeof errors>();
    for (const e of errors) byLoc.set(e.instanceLocation, [...(byLoc.get(e.instanceLocation) ?? []), e]);
    for (const [loc, errs] of byLoc) {
      for (const e of errs.filter((x) => x.keyword === "required")) add(loc, e.error.replace("Instance does not have", "Missing"));
      if (errs.some((x) => x.keyword === "false")) {
        add(loc, `Unknown property "${pointer(loc).at(-1)}"`, true);
        continue;
      }
      if (hasDeeper(loc)) continue; // wrapper errors; the deeper ones say what's wrong
      const consts = errs.filter((x) => x.keyword === "const" || x.keyword === "enum");
      if (consts.length) {
        const s = this.at(pointer(loc).map((x) => (/^\d+$/.test(x) ? "*" : x)));
        const vals = s ? this.values(s) : [];
        add(loc, vals.length ? `Expected one of: ${vals.join(", ")}` : consts[0].error);
        continue;
      }
      const types = errs
        .filter((x) => x.keyword === "type")
        .map((x) => /Expected "?([\w, "]+?)"?\.$/.exec(x.error)?.[1].replaceAll('"', ""))
        .filter((t) => t && t !== "null");
      if (types.length) {
        add(loc, `Expected ${[...new Set(types)].join(" or ")}`);
        continue;
      }
      const leaf = errs.find((x) => !["properties", "items", "$ref", "anyOf", "oneOf", "allOf", "additionalProperties", "required"].includes(x.keyword));
      if (leaf) add(loc, leaf.error);
    }
    return out;
  }
}

/** "#/jobs/0/name" → ["jobs", "0", "name"] */
function pointer(loc: string): string[] {
  return loc
    .replace(/^#\/?/, "")
    .split("/")
    .filter(Boolean)
    .map((s) => decodeURIComponent(s).replaceAll("~1", "/").replaceAll("~0", "~"));
}

/** Text range of the node at `path` (its key when `onKey`); a whole map is cut to its first line. */
function nodeRange(root: unknown, path: string[], onKey: boolean, text: string): [number, number] {
  let node = root as YamlNode | null | undefined;
  let key: YamlNode | null = null;
  for (const seg of path) {
    if (isMap(node)) {
      const pair = node.items.find((p) => isPair(p) && isScalar(p.key) && String(p.key.value) === seg);
      key = (pair?.key as YamlNode) ?? null;
      node = pair?.value as YamlNode | undefined;
    } else if (isSeq(node)) {
      node = node.items[Number(seg)] as YamlNode | undefined;
      key = null;
    } else break;
  }
  const target = onKey && key ? key : (node ?? key);
  const range = target?.range;
  if (!range) return [0, Math.max(1, text.indexOf("\n"))];
  const eol = text.indexOf("\n", range[0]);
  return [range[0], Math.max(range[0] + 1, Math.min(range[1], eol < 0 ? range[1] : eol))];
}
