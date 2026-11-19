/**
 * Starting points for "New from template": a short, curated set of examples/yaml specs, bundled as
 * lazy chunks. Most examples demo one feature; those are reachable through Insert and completion.
 */
import { BLANK } from "./draft.svelte";

/** Template names (examples/yaml/<name>.yaml) by group, in display order. */
const GROUPS: [string, string[]][] = [
  ["Basics", ["diamond_workflow"]],
  ["Many jobs", ["hundred_jobs_parameterized", "hyperparameter_sweep", "data_pipeline_parameterized"]],
  ["Slurm", ["workflow_actions_simple_slurm", "multi_node_slurm"]],
  ["Reliability", ["simple_retry"]],
];

// import.meta.glob needs literal patterns, so the files are listed again here.
const FILES = import.meta.glob<string>(
  [
    "../../../../../examples/yaml/diamond_workflow.yaml",
    "../../../../../examples/yaml/hundred_jobs_parameterized.yaml",
    "../../../../../examples/yaml/hyperparameter_sweep.yaml",
    "../../../../../examples/yaml/data_pipeline_parameterized.yaml",
    "../../../../../examples/yaml/workflow_actions_simple_slurm.yaml",
    "../../../../../examples/yaml/multi_node_slurm.yaml",
    "../../../../../examples/yaml/simple_retry.yaml",
  ],
  { query: "?raw", import: "default" },
);

export type Template = { name: string; group: string; description: string; text: string };

export const BLANK_TEMPLATE: Template = {
  name: "Blank",
  group: "Basics",
  description: "A name and one job",
  text: BLANK,
};

let cache: Promise<Template[]> | null = null;

/** All templates in display order, with descriptions from each spec's `description:` field. */
export function loadTemplates(): Promise<Template[]> {
  const byName = new Map(Object.entries(FILES).map(([p, load]) => [p.split("/").pop()!.replace(/\.yaml$/, ""), load]));
  cache ??= import("yaml").then(({ parse }) =>
    Promise.all(
      GROUPS.flatMap(([group, names]) =>
        names.map(async (name) => {
          const text = await byName.get(name)!();
          let description = "";
          try {
            description = String(parse(text)?.description ?? "");
          } catch {
            // A template that doesn't parse still opens; it just has no description.
          }
          return { name, group, description, text };
        }),
      ),
    ),
  );
  return cache;
}
