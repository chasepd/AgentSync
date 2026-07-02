# Sync Safety

AgentSync is dry-run first. Commands that can change files must require
`--write`, and generated changes should be previewable with `diff`.

## Defaults

- Do not write unless `--write` is passed.
- Show diffs before overwriting.
- Use transient recovery for overwritten files by default.
- Retain `.bak` backups only when `sync --retain-backups --write` is passed.
- Refuse drifted-file overwrites unless the strategy allows them.
- Warn on partial conversions.
- Block unsafe lossy conversions by default.
- Never execute generated hooks, plugins, commands, or skills.
- Hook rendering must follow `docs/hook-equivalence-policy.md`: render only
  directly equivalent entries or implemented shims, and report everything else.

## Conflict Handling

Conflicts can be resolved with an explicit source of truth, a strategy such as
`newest`, or an interactive resolver. Without one of those choices, AgentSync
should report the conflict and avoid writing.
