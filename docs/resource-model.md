# Resource Model

AgentSync uses a normalized model with common portable fields plus preserved
native extension data. This lets it sync the parts that map cleanly while keeping
agent-specific details visible and recoverable.

## Resource Kinds

- `RuleSet`: repository or user instructions such as `AGENTS.md` and `CLAUDE.md`.
- `Skill`: `SKILL.md` folders and related files.
- `Subagent`: named custom agents and role definitions.
- `Hook`: lifecycle automation and event handlers.
- `Command`: reusable command prompts or command actions.

## Common Fields

Every normalized resource has an id, kind, scope, source agent, native paths,
portable fields, native extension fields, and diagnostics.

Portable fields are eligible for cross-agent rendering. Native extension fields
are keyed by source agent and preserved when a target cannot represent them.

Subagents are normalized read-only in the current MVP. AgentSync preserves name,
description, body, and frontmatter in scan/status reports. `diff subagent` and
`sync subagents` return blocked plan actions; rendering and write sync remain
blocked until subagent conversion is explicitly supported.

Commands are also selectable for planning, but they remain blocked for MVP sync
because command resources can encode behavioral or executable workflows.
`diff command` and `sync commands` report blocked plan actions instead of
rendering target files or updating state. OpenCode command files and
`opencode.json` command config are discovered as blocked command resources.

Hooks follow the same safety model. Claude settings files with a top-level
`hooks` field are discovered as hook resources, and `diff hook` / `sync hooks`
return blocked plan actions until hook conversion has an explicit safety design.

Blocked behavioral resources preserve their raw native file text in native
extensions so scan/status output can surface what was blocked without rendering
or writing it.
