// Spec editor schema logic: completion paths, hover, lint, and that every example spec lints clean.
// Run: npm test (needs cargo; the schema comes from `torc workflows schema`).
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { insertSnippet, SNIPPETS } from "../src/features/editor/snippets.ts";
import { SpecSchema } from "../src/features/editor/yamlSchema.ts";

const repo = path.resolve(import.meta.dirname, "../../..");
const schema = new SpecSchema(
  JSON.parse(execFileSync("cargo", ["run", "-q", "--bin", "torc", "--", "workflows", "schema"], { cwd: repo, encoding: "utf8" })),
);

/** Completion labels at the "|" in `src`. */
function labels(src: string): string[] {
  const pos = src.indexOf("|");
  return schema.complete(src.replace("|", ""), pos)?.options.map((o) => o.label) ?? [];
}

// Keys of the enclosing mapping, minus ones already present.
let l = labels("name: w\njobs:\n  - name: a\n    |\n");
assert.ok(l.includes("command") && !l.includes("name"), `job keys: ${l}`);
l = labels("jobs:\n- name: a\n  |"); // dash at the parent key's column
assert.ok(l.includes("command"), `same-column list: ${l}`);
l = labels("jobs:\n  - |");
assert.ok(l.includes("name") && l.includes("command"), `new item: ${l}`);
l = labels("na|");
assert.ok(l.includes("name") && l.includes("jobs"), `top level: ${l}`);
l = labels("resource_requirements:\n  - name: r\n    num_|");
assert.ok(l.includes("num_cpus") && l.includes("memory"), `rr keys: ${l}`);
l = labels("jobs:\n  - name: a\n    command: |\n      ech|");
assert.deepEqual(l, [], "no keys inside a block scalar");

// Values.
assert.deepEqual(labels("execution_config:\n  mode: |"), ["direct", "slurm", "auto"]);
assert.deepEqual(labels("enable_ro_crate: |"), ["true", "false"]);

// Hover.
const src = "resource_requirements:\n  - name: r\n    num_cpus: 2\n";
const h = schema.hover(src, src.indexOf("num_cpus") + 2);
assert.equal(h?.title, "num_cpus");
assert.ok(h?.info, "num_cpus has a description");

// Lint: one specific message per problem, on the right text.
const bad = `name: x
jobs:
  - name: a
    comand: echo
resource_requirements:
  - name: r
    num_cpus: 2
execution_config:
  mode: fast
resource_monitor: nope
`;
const diags = schema.lint(bad);
const msg = (m: string) => diags.find((d) => d.message === m);
assert.equal(bad.slice(msg('Unknown property "comand"')!.from, msg('Unknown property "comand"')!.to), "comand");
assert.ok(msg('Missing required property "command".'), JSON.stringify(diags));
assert.ok(msg('Missing required property "memory".'), JSON.stringify(diags));
assert.equal(bad.slice(msg("Expected one of: direct, slurm, auto")!.from, msg("Expected one of: direct, slurm, auto")!.to), "fast");
assert.ok(msg("Expected object"), JSON.stringify(diags));
assert.equal(diags.length, 5, JSON.stringify(diags));
assert.equal(schema.lint("jobs: [\n").length > 0, true, "YAML syntax error");

// Every example spec is valid against the schema.
for (const dir of ["yaml", "subgraphs"]) {
  for (const f of fs.readdirSync(path.join(repo, "examples", dir)).filter((f) => /\.ya?ml$/.test(f))) {
    const d = schema.lint(fs.readFileSync(path.join(repo, "examples", dir, f), "utf8"));
    assert.deepEqual(d, [], `${dir}/${f}`);
  }
}
console.log("editor schema checks passed");

// Snippets land at the end of their section, indented like its items, and stay schema-valid.
const job = SNIPPETS.find((s) => s.label === "Job")!;
let r = insertSnippet("name: w\njobs:\n    - name: a\n      command: x\n\nfiles: []\n", job);
assert.equal(r.text, "name: w\njobs:\n    - name: a\n      command: x\n    - name: my_job\n      command: echo hello\n\nfiles: []\n");
assert.equal(r.text.slice(r.from, r.to), "    - name: my_job\n      command: echo hello");
r = insertSnippet("name: w", job);
assert.equal(r.text, "name: w\n\njobs:\n  - name: my_job\n    command: echo hello\n");
let doc = "name: w\njobs:\n  - name: a\n    command: x\n";
for (const s of SNIPPETS) doc = insertSnippet(doc, s).text;
assert.deepEqual(schema.lint(doc), [], doc);
// Flow-style sections: an empty one becomes a block section; a non-empty one is refused.
r = insertSnippet("name: w\nfiles: []  # none yet\njobs:\n  - name: a\n    command: x\n", SNIPPETS.find((s) => s.section === "files")!);
assert.equal(r.text, "name: w\nfiles:  # none yet\n  - name: raw_data\n    path: data/raw.csv\njobs:\n  - name: a\n    command: x\n");
assert.throws(() => insertSnippet("jobs: [{name: a, command: x}]\n", job), /flow style/);
console.log("snippet checks passed");
