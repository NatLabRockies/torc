# Torc skills

Agent Skills for operating [Torc](https://github.com/NatLabRockies/torc), a distributed workflow
orchestration system for computational pipelines. Every skill is a lean `SKILL.md` entry point with
task-routed references that agents load only when the task needs them.

The skills describe the `torc` CLI, server, and workflow specification as implemented in this
repository. When a site runs a different Torc version, the installed `torc <command> --help` is
authoritative.

## Quick start

### Skills CLI

```bash
# Install one skill globally
npx skills add NatLabRockies/torc --skill torc-workflows --global

# Install every skill for every detected agent
npx skills add NatLabRockies/torc --all

# See what is available first
npx skills add NatLabRockies/torc --list
```

### GitHub CLI

```bash
# Install one skill
gh skill install NatLabRockies/torc torc-workflows

# Install the complete collection
gh skill install NatLabRockies/torc --all
```

The repository also carries a Claude-specific `.claude/skills/review-api` skill. `gh skill` skips
hidden directories unless `--allow-hidden-dirs` is passed; the Skills CLI lists it alongside the
five skills below, so select by name when you only want these.

## Update

Use the same tool that installed the skills.

| Installed with | Update all              | Update one                         |
| -------------- | ----------------------- | ---------------------------------- |
| GitHub CLI     | `gh skill update --all` | `gh skill update torc-workflows`   |
| Skills CLI     | `npx skills update`     | `npx skills update torc-workflows` |

## Skill catalog

### Using Torc

| Skill                               | Best for                                                             |
| ----------------------------------- | -------------------------------------------------------------------- |
| [`torc-workflows`](torc-workflows/) | Writing specs and running workflows locally, on Slurm, or on workers |
| [`torc-inspect`](torc-inspect/)     | Querying workflow, job, result, and resource state for scripting     |
| [`torc-debug`](torc-debug/)         | Diagnosing failed, stuck, orphaned, or unscheduled jobs              |
| [`torc-config`](torc-config/)       | Layered configuration, log levels, and log file locations            |

### Developing Torc

| Skill                                     | Best for                                                         |
| ----------------------------------------- | ---------------------------------------------------------------- |
| [`torc-contributing`](torc-contributing/) | Repository quality gates, tests, migrations, and OpenAPI syncing |

## How agents navigate this collection

1. The `name` and `description` in `SKILL.md` decide which skill activates.
2. The `SKILL.md` body carries the mental model, hard constraints, and core loop.
3. A task router links to a single reference per task, so only the relevant file is loaded.
4. Behavior that `--help` does not explain (exit codes, stream routing, log paths, state
   preconditions) lives in the references, verified against this repository.

## License

These skills ship under the repository's
[BSD 3-Clause license](https://github.com/NatLabRockies/torc/blob/main/LICENSE).
