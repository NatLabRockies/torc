# Installing the Torc Agent Skill

Torc ships an agent skill that teaches AI coding assistants (Claude Code, Codex, Copilot, and other
tools that support the [Agent Skills](https://agentskills.io) format) how to author, run, inspect,
and debug Torc workflows. The skill lives in the repository at
[`skills/torc/`](https://github.com/NatLabRockies/torc/tree/main/skills/torc).

## Skill vs. MCP Server

The skill and the [MCP server](./ai-assistants.md) are complementary:

- **Skill**: instructions and reference material. The assistant learns Torc's spec format, execution
  modes, Slurm sizing, recovery steps, and which `torc` CLI commands to run. No extra binary or
  server configuration is required.
- **MCP server**: tools the assistant calls directly against the Torc API.

The skill works on its own through the `torc` CLI, and it uses the MCP tools when they are
configured.

## Install with the `skills` CLI

The [`skills`](https://github.com/vercel-labs/skills) CLI installs skills from GitHub into any
supported agent:

```bash
# Install into the current project
npx skills add NatLabRockies/torc --skill torc

# Install for your user account (all projects)
npx skills add NatLabRockies/torc --skill torc -g
```

The CLI prompts for which agents to install into. Rerun the command to pick up a newer version of
the skill.

## Install Manually

Copy the `skills/torc` directory into your agent's skills directory. For Claude Code:

```bash
git clone --depth 1 https://github.com/NatLabRockies/torc.git /tmp/torc

# User-level (all projects)
mkdir -p ~/.claude/skills
cp -r /tmp/torc/skills/torc ~/.claude/skills/

# Or project-level
mkdir -p .claude/skills
cp -r /tmp/torc/skills/torc .claude/skills/
```

Other agents use their own directories (for example, `.agents/skills/` or `~/.agents/skills/`); see
your agent's documentation.

## Verify

Start a new session and ask something Torc-specific, such as "Write a torc workflow spec that runs
10 parameterized jobs on Slurm." In Claude Code, the skill also appears in the list shown by
`/skills` and can be invoked directly with `/torc`.

## See Also

- [Configuring AI Assistants](./ai-assistants.md) - Set up the Torc MCP server
- [AI-Assisted Workflow Management](./ai-assistant.md) - Using AI for workflow management
