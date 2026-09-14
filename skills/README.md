# Torc skill

An Agent Skill for [Torc](https://github.com/NatLabRockies/torc), a distributed workflow
orchestration system for computational pipelines. One skill, one `SKILL.md` entry point, and a task
router into focused references that agents load only when the task needs them.

The skill describes the `torc` CLI, server, and workflow specification as implemented in this
repository. When a site runs a different Torc version, the installed `torc <command> --help` is
authoritative.

## Quick start

### Skills CLI

```bash
# Install one skill globally
npx skills add NatLabRockies/torc --skill torc --global

# Install every skill for every detected agent
npx skills add NatLabRockies/torc --all

# See what is available first
npx skills add NatLabRockies/torc --list
```

### GitHub CLI

```bash
# Install one skill
gh skill install NatLabRockies/torc torc

# Install the complete collection
gh skill install NatLabRockies/torc --all
```

The repository also carries a Claude-specific `.claude/skills/review-api` skill. `gh skill` skips
hidden directories unless `--allow-hidden-dirs` is passed; the Skills CLI lists it alongside `torc`,
so select by name when you only want this one.

## Update

Use the same tool that installed the skill.

| Installed with | Update all              | Update one               |
| -------------- | ----------------------- | ------------------------ |
| GitHub CLI     | `gh skill update --all` | `gh skill update torc`   |
| Skills CLI     | `npx skills update`     | `npx skills update torc` |

## What it covers

[`torc/SKILL.md`](torc/) carries the mental model, the mode-selection table, the packing arithmetic,
and the constraints that are easy to get wrong. Its task router points at one reference per task:

| Area                | References                                                                    |
| ------------------- | ----------------------------------------------------------------------------- |
| Authoring           | `spec-authoring.md`                                                           |
| Running             | `local-execution.md`, `execution-modes.md`, `slurm.md`, `remote-workers.md`   |
| Optimizing          | `optimization.md`                                                             |
| Reruns and recovery | `rerun-and-recovery.md`                                                       |
| Inspection          | `query-map.md`, `scripting.md`, `live-monitoring.md`, `command-behavior.md`   |
| Debugging           | `failure-analysis.md`, `log-map.md`, `stuck-workflows.md`, `connectivity.md`  |
| Configuration       | `settings.md`, `logging.md`, `hpc-profiles.md`                                |
| Development         | `quality-gates.md`, `testing.md`, `api-and-database.md`, `adding-features.md` |

`evals/trigger-prompts.json` tunes activation and `evals/evals.json` grades output quality. Neither
is loaded during normal work.

## How agents navigate this skill

1. The `name` and `description` in `SKILL.md` decide when the skill activates.
2. The `SKILL.md` body carries the mental model, hard constraints, and core loop.
3. The task router links to a single reference per task, so only the relevant file is loaded.
4. Behavior that `--help` does not explain (exit codes, stream routing, packing arithmetic, log
   paths, state preconditions) lives in the references, verified against this repository.

## License

This skill ships under the repository's
[BSD 3-Clause license](https://github.com/NatLabRockies/torc/blob/main/LICENSE).
