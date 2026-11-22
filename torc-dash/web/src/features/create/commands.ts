/**
 * A workflow spec with one independent job per command line (like `torc exec -C file`).
 * Blank lines and `#` comments are skipped. Returns null when there are no commands.
 */
export function commandsSpec(text: string, name: string) {
  const lines = text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("#"));
  if (!lines.length) return null;
  const width = Math.max(3, String(lines.length).length);
  return {
    name: name.trim() || "commands",
    jobs: lines.map((command, i) => ({
      name: `job_${String(i + 1).padStart(width, "0")}`,
      command,
    })),
  };
}
