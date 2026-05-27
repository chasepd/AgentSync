# Changelog

## v0.1.0 MVP

AgentSync v0.1.0 establishes the first safe sync workflow for multi-agent
configuration.

### Shipped

- Scan project and user config for Claude Code, Codex CLI, Cursor CLI, and OpenCode.
- Report native capabilities and compatibility diagnostics for discovered resources.
- Sync rules/context files across supported native formats.
- Sync portable `SKILL.md` skills and text assets while blocking path traversal and binary assets.
- Preview generated changes with `diff`, write only with explicit `--write`, and track sync metadata in `.agentsync/state.json`.
- Detect drift with `status` and `status --check` for CI.
- Run a dedicated GitHub Actions drift workflow with `agentsync status --check`.
- Target specific resources such as `agentsync diff skill review` and `agentsync sync subagent reviewer`.
- Sync every matching resource kind with `agentsync sync --all`.
- Protect existing files with backups, `--no-overwrite`, `--strategy source`, and `--strategy newest`.
- Resolve conflicts with `sync --interactive` when running in a TTY.
- Parse YAML frontmatter for skills and subagents while preserving unsupported native fields.
- Render portable subagents for Claude Code, Codex CLI, and OpenCode.
- Render prompt-only OpenCode commands and block executable or agent-like command behavior.
- Discover hooks, permissions, plugins, and unsafe behavioral resources with structured diagnostics while keeping them blocked for writes.
- Provide an adapter registry boundary so external adapters can declare capabilities, discover resources, and participate in planning without changing built-ins.
- Run GitHub Actions CI for formatting, clippy, and workspace tests.

### Safety Notes

- Dry-run behavior is the default. Files are written only when commands use explicit write intent.
- Unsupported behavioral fields are never silently dropped; they are reported as blocked or partial.
- User-scope discovery is supported, but user-scope writes remain blocked until a home-directory write policy lands.
- Cursor support is focused on documented Cursor CLI rules and shared instruction files until stable custom subagent file-format docs are available.
