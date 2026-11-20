import { dashPost } from "../../lib/api/dash";

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
