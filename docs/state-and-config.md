# State And Config

AgentSync stores project metadata under `.agentsync/`.

## Config

`.agentsync/config.toml` records project defaults. The MVP config starts with
source agent, target agents, project scope, and sync toggles for selectable
resource kinds.

Create the default project config with:

```bash
agentsync init --write
```

`agentsync init` without `--write` only previews the file and leaves the
filesystem untouched. Existing config files are never overwritten by `init`.

The current MVP config shape is intentionally small:

```toml
schema_version = 1

[defaults]
scope = "project"
source = "all"
targets = ["all"]

[sync]
rules = true
skills = true
subagents = false
commands = false
hooks = false
```

When present, `defaults.scope` is used by `scan` and `status` if `--scope` is
omitted. `defaults.source` and `defaults.targets` are used by `diff` and `sync`
if `--from` or `--to` are omitted:

```bash
agentsync scan
agentsync status
agentsync diff rules
agentsync sync rules --write
agentsync sync --all --write
```

`defaults.source = "all"` tells AgentSync to choose the changed source for each
logical resource. `defaults.targets = ["all"]` expands to every supported target
agent: Codex CLI, Claude Code, Cline CLI, Cursor CLI, and OpenCode. This is useful for
repos where different contributors edit different native tools. If a repo has
no state metadata yet and native files already disagree, run the first sync with
an explicit `--from`.

When `diff` or `sync` use config defaults, `[sync]` toggles gate the selected
resource kind. For example, `rules = false` blocks config-driven rules planning,
and `skills = false` blocks config-driven skills planning before any write.
For `sync --all`, disabled resource kinds are skipped; the command fails if no
resource kinds remain enabled. Behavioral resources default to disabled in newly
generated config because subagents, commands, and hooks can carry executable or
target-specific behavior; enable them explicitly after reviewing the plan.

Invalid config is reported by `agentsync doctor --check` and causes commands
that need config defaults to fail before planning writes.

## State

`.agentsync/state.json` records relationships between native files:

- resource id and kind
- source agent
- source and target agents
- native paths
- content checksums
- last synced timestamp
- compatibility diagnostics
- preserved native extension metadata

State is not the source of truth. Native files remain canonical.

The current state schema version is `1`. AgentSync treats unsupported state
versions as invalid instead of guessing how to interpret future schema shapes.
