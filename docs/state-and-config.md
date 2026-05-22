# State And Config

AgentSync stores project metadata under `.agentsync/`.

## Config

`.agentsync/config.toml` records project defaults. The MVP config starts with
source agent, target agents, project scope, and rules/skills sync toggles.

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
source = "agents-md"
targets = ["claude", "cursor", "opencode"]

[sync]
rules = true
skills = true
```

When present, `defaults.source` and `defaults.targets` are used by `diff` and
`sync` if `--from` or `--to` are omitted:

```bash
agentsync diff rules
agentsync sync rules --write
```

Invalid config is reported by `agentsync doctor --check` and causes commands
that need config defaults to fail before planning writes.

## State

`.agentsync/state.json` records relationships between native files:

- resource id and kind
- source and target agents
- native paths
- content checksums
- last synced timestamp
- compatibility diagnostics
- preserved native extension metadata

State is not the source of truth. Native files remain canonical.
