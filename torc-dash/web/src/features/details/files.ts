import { dashPost } from "../../lib/api/dash";

/** Same key as the legacy UI and the debugging tab, so all log viewers share one directory. */
const OUTPUT_DIR_KEY = "torc-job-logs-output-dir";

export function loadOutputDir(): string {
  try {
    return localStorage.getItem(OUTPUT_DIR_KEY) || "torc_output";
  } catch {
    return "torc_output";
  }
}

export function saveOutputDir(dir: string) {
  try {
    localStorage.setItem(OUTPUT_DIR_KEY, dir);
  } catch {
    // Convenience only.
  }
}

/**
 * Join a relative output dir to the workflow's submission directory (where logs are written) so
 * paths resolve regardless of torc-dash's working directory. Absolute paths (POSIX, `~`, Windows
 * drive, UNC) are returned unchanged.
 */
export function resolveLogBaseDir(outputDir: string, submissionDir?: string | null): string {
  if (/^(\/|~|[A-Za-z]:[\\/]|\\\\)/.test(outputDir) || !submissionDir) return outputDir;
  return `${submissionDir.replace(/[\\/]+$/, "")}/${outputDir}`;
}

export type ReadFileResponse = {
  success: boolean;
  content: string | null;
  error: string | null;
  is_json: boolean;
  exists: boolean;
};

export const readFile = (path: string) =>
  dashPost<ReadFileResponse>("/api/cli/read-file", { path });

/** Read the first candidate path that exists (e.g. `.o`, then the combined `.log`). */
export async function readFirst(
  paths: string[],
): Promise<{ path: string; res: ReadFileResponse }> {
  let res!: ReadFileResponse;
  for (const path of paths) {
    res = await readFile(path);
    if (res.exists) return { path, res };
  }
  return { path: paths[0], res };
}

const escapeHtml = (s: string) =>
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
