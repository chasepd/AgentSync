# State And Config

AgentSync stores project metadata under `.agentsync/`.

## Config

`.agentsync/config.toml` records defaults such as source agent, target agents,
enabled scopes, per-resource behavior, and ignored paths.

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

