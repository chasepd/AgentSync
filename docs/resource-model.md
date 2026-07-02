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
- `Plugin`: executable extension modules or package references.
- `Permission`: sandbox, tool, or command permission policy.

## Common Fields

Every normalized resource has an id, kind, scope, source agent, native paths,
portable fields, native extension fields, and diagnostics.

Portable fields are eligible for cross-agent rendering. Native extension fields
are keyed by source agent and preserved when a target cannot represent them.

Subagents preserve name, description, body, and frontmatter for Markdown
subagent or agent-preset files in scan/status reports. Portable subagents can
render to Codex, Claude Code, Cline, and OpenCode targets. Tool policies,
permissions, hooks, plugins, MCP fields, and native-only behavioral fields block
rendering until their target semantics can be preserved. OpenCode JSON/JSONC
`agent` config is discovered as blocked subagent behavior and preserves raw
native text.

Commands are also selectable for planning, but they remain blocked for MVP sync
because command resources can encode behavioral or executable workflows.
`diff command` and `sync commands` report blocked plan actions instead of
rendering target files or updating state. OpenCode command files and
`opencode.json` / `opencode.jsonc` command config are discovered as blocked
command resources.

Hooks follow the same safety model. Claude settings files, Codex
`.codex/hooks.json` files, Cline `.cline/hooks/*` files, and Cursor
`.cursor/hooks.json` files with a top-level `hooks` field are discovered as hook
resources. Hook conversion uses the entry-by-entry policy in
`docs/hook-equivalence-policy.md`: directly equivalent Codex/Claude/Cursor
command hooks are partial render candidates, Cline and OpenCode shim-required
entries can render to `.cline/plugins/agentsync-hooks.js` or
`.opencode/plugins/agentsync-hooks.js` when AgentSync has implemented the event
and matcher adapter, and report-only entries remain diagnostics/native
extensions.

Blocked behavioral resources preserve their raw native file text in native
extensions so scan/status output can surface what was blocked without rendering
or writing it.

Native Cline and OpenCode plugins are discovered read-only when they appear in
native plugin directories or related config. They are blocked resources in the
MVP and are not selectable for diff/sync rendering. Generated AgentSync Cline
and OpenCode hook shims are hook render targets, not portable plugin resources.

Permission policy is discovered read-only from Claude settings and OpenCode
JSON/JSONC config. It is blocked in the MVP because permission semantics are
safety-critical and not portable across agents yet.
