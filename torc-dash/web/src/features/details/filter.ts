/**
 * Table filter syntax ported from the legacy dashboard: `field<op>value` with ops
 * `= != > < >= <= ~ :` (`~`/`:` = contains). Numbers compare numerically, `status` compares by name,
 * everything else as case-insensitive strings. Anything else is a plain substring search, which
 * DataTable handles itself.
 */

const OPERATOR = /^(\w+)\s*(!=|>=|<=|>|<|=|~|:)\s*(.+)$/;

/** Friendly aliases for column names. */
const ALIASES: Record<string, string> = {
  exec_time: "exec_time_minutes",
  peak_mem: "peak_memory_bytes",
  avg_cpu: "avg_cpu_percent",
  modified: "st_mtime",
};

export type OperatorFilter = { field: string; op: string; value: string };

export function parseOperatorFilter(text: string): OperatorFilter | null {
  const m = text.trim().toLowerCase().match(OPERATOR);
  return m ? { field: ALIASES[m[1]] ?? m[1], op: m[2], value: m[3] } : null;
}

/** The whole string as a number, or null (unlike parseFloat, which accepts numeric prefixes). */
function toNumber(s: string): number | null {
  const t = s.trim();
  if (!t) return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}

export function matchesOperator(item: object, { field, op, value }: OperatorFilter): boolean {
  const raw = (item as Record<string, unknown>)[field];

  if (field === "status") {
    const s = String(raw ?? "").toLowerCase();
    if (op === "=") return s === value;
    if (op === "!=") return s !== value;
    if (op === "~" || op === ":") return s.includes(value);
    return true;
  }

  // Numeric only when both sides are entirely numbers ("1_train" is a name, not 1).
  const num = toNumber(value);
  const itemNum = typeof raw === "number" ? raw : toNumber(String(raw ?? ""));
  if (num !== null && itemNum !== null) {
    switch (op) {
      case "=":
        return itemNum === num;
      case "!=":
        return itemNum !== num;
      case ">":
        return itemNum > num;
      case "<":
        return itemNum < num;
      case ">=":
        return itemNum >= num;
      case "<=":
        return itemNum <= num;
    }
  }

  const s = String(raw ?? "").toLowerCase();
  if (op === "=") return s === value;
  if (op === "!=") return s !== value;
  return s.includes(value);
}
