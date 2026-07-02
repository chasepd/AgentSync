# Milestones

## M0: Docs And Skeleton

Add architecture docs, Rust workspace skeleton, adapter traits, resource model,
and placeholder CLI commands. No write behavior.

## M1: Read-Only Scanner

Implement `agentsync scan --scope project` for Claude Code, Codex CLI, Cline CLI,
Cursor CLI, and OpenCode. Add table and JSON output plus layout fixtures.

## M2: Normalization And Capability Reporting

Normalize rules, skills, and subagents. Add adapter capability matrices and
surface portable, partial, and unsupported fields.

## M3: Status And Drift Detection

Implement `.agentsync/state.json`, `status`, `status --check`, and missing or
stale target detection.

## M4: Rules And Skills Sync

Implement `diff`, `sync --dry-run`, and `sync --write` for rules and portable
`SKILL.md` skills. Add backups, no-overwrite behavior, and lossy warnings.

## M5: Subagent Conversion

Convert subagents where supported, preserve native frontmatter, and add
field-level compatibility diagnostics.

## M6: Hooks, Commands, And Security-Sensitive Resources

Discover hooks and commands first. Add rendering only behind explicit flags and
strict diagnostics. Never auto-run generated code.

## M7: Conflict Resolver And Extension System

Add `sync --interactive`, conflict strategies, and adapter/plugin extension
points for future tools.
