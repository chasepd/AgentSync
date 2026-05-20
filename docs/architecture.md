# Architecture

AgentSync is a native-file sync tool, not a shared agent runtime. It discovers
configuration files used by coding agents, normalizes the portable parts, reports
compatibility gaps, and renders native files only when explicitly asked to write.

## Layers

- CLI: parses `scan`, `status`, `diff`, `sync`, `init`, and `doctor`.
- Core engine: coordinates discovery, normalization, comparison, planning, and rendering.
- Adapters: know each agent's files, schemas, capabilities, and render rules.
- State/config: stores user preferences and sync relationships in `.agentsync/`.
- Safety layer: owns dry-run defaults, diffs, backups, lossy warnings, and write blockers.

## Command Flow

`scan` discovers native files and prints what exists. `status` compares discovered
files with stored sync state. `diff` builds a render plan and shows file changes.
`sync` uses the same plan, but writes only when `--write` is present. `doctor`
validates parseability, state consistency, and compatibility warnings.

## Distribution

The implementation is a Rust CLI. npm and Homebrew packages should distribute
the compiled binary. `cargo install --path .` remains the source-install path.

