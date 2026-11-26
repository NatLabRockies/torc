//! The workflow-spec JSON Schema applied to YAML text as it is typed: completions at the cursor,
//! hover help for a key, and unknown-key / bad-value lint. Used by the spec editor
//! (`/ui/editor/{complete,hover,lint}`).
//!
//! Paths come from indentation, not from a parse, so they work while the document is half-typed.
//! Offsets are byte offsets into the text; callers convert to the editor's UTF-16 units.
//! ponytail: block-style YAML only. Flow collections (`{a: 1}`, `[x, y]`) get no help; the spec
//! parser still reports their errors.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

static ROOT: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(&torc::client::workflow_spec::workflow_spec_schema()).unwrap_or_default()
});

/// Map keys, with "*" for "an item of this sequence".
type Path = Vec<String>;

#[derive(Serialize, Debug)]
pub struct Choice {
    pub label: String,
    pub detail: String,
    pub info: String,
    pub apply: String,
}

#[derive(Serialize, Debug)]
pub struct Completions {
    pub from: usize,
    pub to: usize,
    pub kind: &'static str,
    pub options: Vec<Choice>,
}

#[derive(Serialize, Debug)]
pub struct Hover {
    pub from: usize,
    pub to: usize,
    pub title: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub info: String,
}

/// One line split into its sequence dashes and key: "  - name: x" → dashes [2], key at 4.
struct Line<'a> {
    indent: usize,
    dashes: Vec<usize>,
    key: Option<&'a str>,
    key_col: usize,
    rest: &'a str,
}

fn word_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')))
        .unwrap_or(s.len())
}

fn parse_line(line: &str) -> Line<'_> {
    let b = line.as_bytes();
    let indent = line.len() - line.trim_start().len();
    let mut col = indent;
    let mut dashes = vec![];
    while b.get(col) == Some(&b'-') && matches!(b.get(col + 1), Some(b' ') | None) {
        dashes.push(col);
        col += 1;
        while b.get(col) == Some(&b' ') {
            col += 1;
        }
    }
    // `key:` followed by whitespace or the end of the line.
    let s = &line[col..];
    let n = word_len(s);
    let after = s[n..].trim_start_matches([' ', '\t']);
    let (key, rest) = match after.strip_prefix(':') {
        Some(r) if n > 0 && (r.is_empty() || r.starts_with(char::is_whitespace)) => {
            (Some(&s[..n]), r)
        }
        _ => (None, ""),
    };
    Line {
        indent,
        dashes,
        key,
        key_col: col,
        rest,
    }
}

fn blank(l: &str) -> bool {
    let t = l.trim_start();
    t.is_empty() || t.starts_with('#')
}

/// `command: |` or `>`: what follows is text, not keys.
fn block_scalar(rest: &str) -> bool {
    rest.trim_start().starts_with(['|', '>'])
}

/// Path of the mapping holding the key on a parsed line (through its own dashes, if any).
fn line_path(lines: &[&str], li: usize, dashes: &[usize], key_col: usize) -> Option<Path> {
    match dashes.first() {
        None => container_path(lines, li, key_col, false),
        Some(&d) => {
            let mut p = container_path(lines, li, d, true)?;
            p.extend(dashes.iter().map(|_| "*".to_string()));
            Some(p)
        }
    }
}

/// Path of the mapping that contains column `col` on line `li`. `seq` means the column holds a
/// sequence dash, so a key at the same column above (`jobs:` / `- name`) is its parent. None
/// inside a block scalar, where keys are plain text.
fn container_path(lines: &[&str], li: usize, mut col: usize, mut seq: bool) -> Option<Path> {
    let mut path = vec![];
    let mut i = li;
    while i > 0 && (col > 0 || seq) {
        i -= 1;
        if blank(lines[i]) {
            continue;
        }
        let p = parse_line(lines[i]);
        if p.indent > col || (p.indent == col && !seq) {
            continue;
        }
        if p.indent == col {
            // seq: a dash here is a sibling item; a key here owns the sequence.
            let Some(key) = p.key.filter(|_| p.dashes.is_empty()) else {
                continue;
            };
            if block_scalar(p.rest) {
                return None;
            }
            path.push(key.to_string());
            seq = false;
            continue;
        }
        // Elements on this line left of `col`, innermost first, are our ancestors.
        if let Some(key) = p.key
            && p.key_col < col
        {
            if block_scalar(p.rest) {
                return None;
            }
            path.push(key.to_string());
        }
        path.extend(
            p.dashes
                .iter()
                .rev()
                .filter(|&&d| d < col)
                .map(|_| "*".to_string()),
        );
        col = p.indent;
        seq = !p.dashes.is_empty();
    }
    path.reverse();
    Some(path)
}

/// Keys already present in the mapping whose keys sit at `col`, around line `li`.
fn sibling_keys<'a>(lines: &[&'a str], li: usize, col: usize) -> HashSet<&'a str> {
    let mut keys = HashSet::new();
    let mut visit = |i: usize| {
        if blank(lines[i]) {
            return true;
        }
        let p = parse_line(lines[i]);
        if let Some(k) = p.key
            && p.key_col == col
        {
            keys.insert(k);
        }
        p.indent >= col // a shallower line ends the mapping (after counting "- key" on it)
    };
    for i in (0..li).rev() {
        if !visit(i) {
            break;
        }
    }
    for i in li + 1..lines.len() {
        if !visit(i) {
            break;
        }
    }
    keys
}

/// Lines of `text`, the index of the one holding byte `pos`, and `pos`'s column in it.
fn line_at(text: &str, pos: usize) -> (Vec<&str>, usize, usize) {
    let pos = pos.min(text.len());
    let start = text[..pos].rfind('\n').map_or(0, |i| i + 1);
    let li = text[..start].matches('\n').count();
    (text.split('\n').collect(), li, pos - start)
}

// ============== Schema ==============

/// Follow `$ref`s and unwrap Option (`anyOf: [T, null]`).
fn resolve(mut s: &Value) -> &Value {
    for _ in 0..16 {
        if let Some(r) = s.get("$ref").and_then(Value::as_str) {
            match ROOT["$defs"].get(r.trim_start_matches("#/$defs/")) {
                Some(d) => s = d,
                None => return s,
            }
            continue;
        }
        let Some(alts) = s.get("anyOf").or(s.get("oneOf")).and_then(Value::as_array) else {
            return s;
        };
        let mut non_null = alts.iter().filter(|a| a["type"] != "null");
        match (non_null.next(), non_null.next()) {
            (Some(a), None) => s = a,
            _ => return s,
        }
    }
    s
}

/// Raw (unresolved) schema of the value at `path`, keeping the property's own description.
fn at(path: &[String]) -> Option<&'static Value> {
    let mut s: &Value = &ROOT;
    for seg in path {
        let r = resolve(s);
        s = if seg == "*" {
            r.get("items")?
        } else {
            match r.get("properties").and_then(|p| p.get(seg)) {
                Some(p) => p,
                None => r.get("additionalProperties").filter(|a| a.is_object())?,
            }
        };
    }
    Some(s)
}

fn describe(s: &Value) -> String {
    s.get("description")
        .or(resolve(s).get("description"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn types(r: &Value) -> Vec<&str> {
    match &r["type"] {
        Value::String(t) => vec![t.as_str()],
        Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    }
}

fn type_label(s: &Value) -> String {
    let r = resolve(s);
    if types(r).contains(&"array") {
        let item = r.get("items").map_or("any".into(), type_label);
        return format!("list of {item}");
    }
    let vals = values(s);
    if !vals.is_empty() {
        return vals.join(" | ");
    }
    let alt_ref = |k| {
        s.get(k)?
            .as_array()?
            .iter()
            .find_map(|a| a.get("$ref")?.as_str())
    };
    if let Some(r) = s
        .get("$ref")
        .and_then(Value::as_str)
        .or_else(|| alt_ref("anyOf"))
        .or_else(|| alt_ref("oneOf"))
    {
        let name = r.trim_start_matches("#/$defs/");
        return name.strip_suffix("Spec").unwrap_or(name).to_string();
    }
    let t: Vec<_> = types(r).into_iter().filter(|t| *t != "null").collect();
    if t.is_empty() {
        "any".into()
    } else {
        t.join(" | ")
    }
}

/// Allowed scalar values: enum, oneOf consts, or booleans.
fn values(s: &Value) -> Vec<String> {
    let r = resolve(s);
    let show = |v: &Value| v.as_str().map_or(v.to_string(), str::to_string);
    if let Some(e) = r.get("enum").and_then(Value::as_array) {
        return e.iter().filter(|v| !v.is_null()).map(show).collect();
    }
    if let Some(alts) = r.get("oneOf").or(r.get("anyOf")).and_then(Value::as_array)
        && alts
            .iter()
            .all(|a| a.get("const").is_some() || a["type"] == "null")
    {
        return alts
            .iter()
            .filter_map(|a| a.get("const"))
            .map(show)
            .collect();
    }
    if types(r).contains(&"boolean") {
        return vec!["true".into(), "false".into()];
    }
    vec![]
}

// ============== Editor features ==============

/// Completions at `pos`: keys of the enclosing mapping, or allowed values after `key: `. The
/// range `from`..`to` covers the whole word at the cursor, so accepting one replaces it.
pub fn complete(text: &str, pos: usize) -> Option<Completions> {
    static VALUE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(\s*(?:-\s+)*)([\w.-]+):\s+([\w.-]*)$").unwrap());
    static KEY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(\s*)((?:-\s+)*)([\w.-]*)$").unwrap());
    let (lines, li, col) = line_at(text, pos);
    let pos = pos.min(text.len());
    let (before, after) = lines[li].split_at(col);
    let to = pos + word_len(after);

    if let Some(c) = VALUE.captures(before) {
        let p = parse_line(before);
        let mut path = line_path(&lines, li, &p.dashes, p.key_col)?;
        path.push(c[2].to_string());
        let s = at(&path)?;
        let vals = values(s);
        if vals.is_empty() {
            return None;
        }
        let info = describe(s);
        return Some(Completions {
            from: pos - c[3].len(),
            to,
            kind: "value",
            options: vals
                .into_iter()
                .map(|v| Choice {
                    label: v.clone(),
                    detail: String::new(),
                    info: info.clone(),
                    apply: v,
                })
                .collect(),
        });
    }

    let c = KEY.captures(before)?;
    let key_col = c[1].len() + c[2].len();
    let typed = &c[3];
    let path = line_path(&lines, li, &parse_line(before).dashes, key_col)?;
    let container = resolve(at(&path)?);
    let props = container.get("properties")?.as_object()?;
    let present = sibling_keys(&lines, li, key_col);
    let pad = " ".repeat(key_col + 2);
    // Renaming an existing key (`nam|: x`): insert only the name, keeping its separator and value.
    let tail = after[to - pos..].trim_start_matches([' ', '\t']);
    let has_colon = tail
        .strip_prefix(':')
        .is_some_and(|r| r.is_empty() || r.starts_with(char::is_whitespace));
    let required: HashSet<&str> = container
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut options: Vec<Choice> = props
        .iter()
        .filter(|(k, _)| !present.contains(k.as_str()) || k.as_str() == typed)
        .map(|(k, s)| {
            let r = resolve(s);
            let t = types(r);
            let nested = if t.contains(&"array") {
                format!("\n{pad}- ")
            } else if t.contains(&"object") && r.get("properties").is_some() {
                format!("\n{pad}")
            } else {
                " ".into()
            };
            Choice {
                label: k.clone(),
                detail: type_label(s),
                info: describe(s),
                apply: if has_colon {
                    k.clone()
                } else {
                    format!("{k}:{nested}")
                },
            }
        })
        .collect();
    options.sort_by_key(|o| !required.contains(o.label.as_str()));
    Some(Completions {
        from: pos - typed.len(),
        to,
        kind: "key",
        options,
    })
}

/// The key under `pos`, its range, type, and description.
pub fn hover(text: &str, pos: usize) -> Option<Hover> {
    let (lines, li, col) = line_at(text, pos);
    let p = parse_line(lines[li]);
    let key = p.key?;
    if col < p.key_col || col > p.key_col + key.len() {
        return None;
    }
    let mut path = line_path(&lines, li, &p.dashes, p.key_col)?;
    path.push(key.to_string());
    let s = at(&path)?;
    let from = pos.min(text.len()) - col + p.key_col;
    Some(Hover {
        from,
        to: from + key.len(),
        title: key.to_string(),
        type_: type_label(s),
        info: describe(s),
    })
}

/// Unknown keys and values outside an enum: (from, to, message), for every line (the spec parser
/// stops at the first error).
pub fn lint(text: &str) -> Vec<(usize, usize, String)> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = vec![];
    let mut start = 0;
    for (li, line) in lines.iter().enumerate() {
        let line_start = start;
        start += line.len() + 1;
        if blank(line) {
            continue;
        }
        let p = parse_line(line);
        let Some(key) = p.key else { continue };
        let Some(path) = line_path(&lines, li, &p.dashes, p.key_col) else {
            continue;
        };
        let Some(parent) = at(&path).map(resolve) else {
            continue;
        };
        let from = line_start + p.key_col;
        let known = parent.get("properties").and_then(|ps| ps.get(key));
        if known.is_none() && parent.get("additionalProperties") == Some(&Value::Bool(false)) {
            out.push((
                from,
                from + key.len(),
                format!("Unknown property \"{key}\""),
            ));
            continue;
        }
        // An inline scalar value: check it against the allowed values.
        let Some(s) = known else { continue };
        let raw = p.rest.split(" #").next().unwrap_or("").trim();
        let v = raw.trim_matches(|c| c == '"' || c == '\'');
        if v.is_empty()
            || matches!(v, "null" | "~")
            || raw.starts_with(['&', '*', '!', '{', '[', '|', '>'])
        {
            continue;
        }
        let vals = values(s);
        if !vals.is_empty() && !vals.iter().any(|x| x == v) {
            let vfrom = line_start + line.len() - p.rest.trim_start().len();
            out.push((
                vfrom,
                vfrom + raw.len(),
                format!("Expected one of: {}", vals.join(", ")),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = "name: w\njobs:\n  - name: a\n    command: echo\n    \nresource_requirements:\n  - name: r\n";

    #[test]
    fn paths() {
        let lines: Vec<&str> = SPEC.split('\n').collect();
        assert_eq!(container_path(&lines, 3, 4, false).unwrap(), ["jobs", "*"]);
        assert_eq!(
            container_path(&lines, 1, 0, false).unwrap(),
            Vec::<String>::new()
        );
        let block: Vec<&str> = "jobs:\n  - command: |\n      name: x".split('\n').collect();
        assert!(container_path(&block, 2, 6, false).is_none());
    }

    #[test]
    fn completes_job_keys() {
        let pos = SPEC.find("    \n").unwrap() + 4;
        let c = complete(SPEC, pos).unwrap();
        assert_eq!(c.kind, "key");
        let labels: Vec<_> = c.options.iter().map(|o| o.label.as_str()).collect();
        assert!(labels.contains(&"depends_on"), "{labels:?}");
        assert!(!labels.contains(&"command"), "present keys are left out");
    }

    #[test]
    fn hover_and_lint() {
        let h = hover(SPEC, SPEC.find("command").unwrap() + 2).unwrap();
        assert_eq!(h.title, "command");
        assert!(!h.info.is_empty());

        let bad = "name: w\njobs:\n  - name: a\n    comand: x\n";
        let d = lint(bad);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(&bad[d[0].0..d[0].1], "comand");
        assert!(lint(SPEC).is_empty(), "{:?}", lint(SPEC));

        let monitor =
            "name: w\nresource_monitor:\n  sample_interval_secs: 5\n  jobs:\n    enabld: true\n";
        let d: Vec<_> = lint(monitor).iter().map(|x| &monitor[x.0..x.1]).collect();
        assert_eq!(d, ["sample_interval_secs", "enabld"]);
    }

    #[test]
    fn examples_lint_clean() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/yaml");
        for e in std::fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            let d = lint(&text);
            assert!(
                d.is_empty(),
                "{}: {:?}",
                path.display(),
                d.iter()
                    .map(|x| (&text[x.0..x.1], &x.2))
                    .collect::<Vec<_>>()
            );
        }
    }
}
