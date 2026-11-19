import type { JobStatus } from "./api/client";

export type Tone = "neutral" | "info" | "accent" | "success" | "warning" | "danger";

export const JOB_STATUS_TONE: Record<JobStatus, Tone> = {
  uninitialized: "neutral",
  blocked: "neutral",
  ready: "info",
  pending: "accent",
  running: "accent",
  completed: "success",
  failed: "danger",
  canceled: "warning",
  terminated: "danger",
  disabled: "neutral",
  pending_failed: "warning",
};

export const JOB_STATUSES = Object.keys(JOB_STATUS_TONE) as JobStatus[];

export function statusLabel(status: string | null | undefined): string {
  return (status ?? "unknown").replace(/_/g, " ");
}

/** Accepts ISO strings, unix seconds, or unix milliseconds. */
export function toDate(value: string | number | null | undefined): Date | null {
  if (value === null || value === undefined || value === "") return null;
  if (typeof value === "number") return new Date(value < 1e12 ? value * 1000 : value);
  const d = new Date(value);
  return Number.isNaN(d.getTime()) ? null : d;
}

export function formatDateTime(value: string | number | null | undefined): string {
  const d = toDate(value);
  if (!d) return "—";
  return d.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function formatRelative(value: string | number | null | undefined): string {
  const d = toDate(value);
  if (!d) return "—";
  const s = Math.round((Date.now() - d.getTime()) / 1000);
  if (s < 5) return "just now";
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

/** Seconds → "1h 02m 03s" / "4m 05s" / "6.2s". */
export function formatDuration(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds)) return "—";
  if (seconds < 60) return `${seconds < 10 ? seconds.toFixed(1) : Math.round(seconds)}s`;
  const total = Math.round(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}h ${pad(m)}m ${pad(s)}s` : `${m}m ${pad(s)}s`;
}

/** Minutes (as stored in results.exec_time_minutes) → human duration. */
export const formatMinutes = (minutes: number | null | undefined) =>
  formatDuration(minutes === null || minutes === undefined ? minutes : minutes * 60);

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes)) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = bytes;
  let i = 0;
  while (Math.abs(v) >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 && i > 0 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

export function formatPercent(value: number | null | undefined, digits = 1): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "—";
  return `${value.toFixed(digits)}%`;
}

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
