# Hook Equivalence Policy

Hooks are executable behavior, so AgentSync maps them entry-by-entry instead of
treating one unmapped field as a reason to drop the whole resource.

This policy is the contract for hook rendering. Today, AgentSync renders the
direct Codex CLI, Claude Code, and Cursor command-hook subset, plus supported
Cline and OpenCode plugin shims, and keeps unsupported shim-required or
report-only entries as diagnostics/native extensions.

## Mapping Outcomes

- `direct`: the source and target have equivalent event, matcher, command
  handler, stdin payload, and blocking-output semantics. AgentSync may render the
  entry and mark it portable or partial depending on fields.
- `shim-required`: the target can express the lifecycle point, but only through
  generated adapter code or a payload/output shim. AgentSync may render this only
  when wrapper generation is implemented and the user has explicit write intent.
- `report-only`: no safe target representation is known. AgentSync must preserve
  native data and emit a note explaining what was not rendered.

Partial hook rendering is allowed only when every plan includes diagnostics for
dropped or emulated behavior. Unsupported entries must stay visible in the
plan/report and must not be silently omitted.

## Event Equivalence

| Canonical event | Codex CLI | Claude Code | Cline CLI | OpenCode | Cursor CLI |
| --- | --- | --- | --- | --- | --- |
| `tool.before` | `PreToolUse` direct | `PreToolUse` direct | `beforeTool` shim-required | `tool.execute.before` shim-required | `preToolUse` direct |
| `tool.after` | `PostToolUse` direct | `PostToolUse` direct | `afterTool` shim-required | `tool.execute.after` shim-required | `postToolUse` direct |
| `permission.request` | `PermissionRequest` direct | `PermissionRequest` direct | report-only | `permission.ask` shim-required | report-only |
| `session.start` | `SessionStart` direct | `SessionStart` direct | `beforeRun` shim-required | `session.created` shim-required | `sessionStart` direct |
| `session.end` | report-only | `SessionEnd` direct | report-only | report-only | `sessionEnd` direct |
| `prompt.submit` | `UserPromptSubmit` direct | `UserPromptSubmit` direct | `beforeRun` shim-required | report-only | `beforeSubmitPrompt` direct |
| `compact.before` | `PreCompact` direct | `PreCompact` direct | report-only | `experimental.session.compacting` shim-required | `preCompact` direct |
| `compact.after` | `PostCompact` direct | `PostCompact` direct | report-only | `session.compacted` shim-required | report-only |
| `agent.start` | `SubagentStart` direct | `SubagentStart` direct | report-only | report-only | `subagentStart` direct |
| `agent.stop` | `SubagentStop` direct | `SubagentStop` direct | report-only | report-only | `subagentStop` direct |
| `session.stop` | `Stop` direct | `Stop` direct | `afterRun` shim-required | report-only | `stop` direct |

For Cline targets, AgentSync renders supported command hooks to
`.cline/plugins/agentsync-hooks.js`. The generated runtime plugin reuses source
commands, passes adapted JSON on stdin, honors numeric timeouts, and maps
blocking output such as `{"decision":"block"}` or `{"block":true}` to Cline
`skip` or `stop` hook results. Native `.cline/hooks/*` file hooks are discovered
as source behavior; supported file-hook events render to other targets through
target-local AgentSync shims that invoke the original hook file. Unsupported
file-hook events remain report-only.

OpenCode mappings are shim-required because OpenCode exposes hooks as plugins,
not as JSON command-hook declarations. AgentSync renders supported
Claude/Codex/Cline/Cursor command hooks to
`.opencode/plugins/agentsync-hooks.js`.
The generated plugin runs the original command, passes adapted JSON on stdin,
and maps nonzero exit or `{"decision":"block"}` / `{"block":true}` output to
OpenCode plugin errors. OpenCode events or matchers without a safe implemented
adapter remain report-only.

Cursor documents native `.cursor/hooks.json` command hooks. AgentSync renders
only the direct event subset above. Cursor-specific events such as
`afterFileEdit`, `beforeShellExecution`, tab hooks, and `workspaceOpen` remain
report-only until AgentSync can map their payload and matcher semantics without
broadening behavior.

## Tool Matcher Equivalence

| Canonical matcher | Codex CLI | Claude Code | Cline CLI | OpenCode | Cursor CLI |
| --- | --- | --- | --- | --- | --- |
| `tool.shell` | `Bash\|exec_command` | `Bash` | `run_commands` shim-required | `bash` shim-required | `Shell` direct |
| `tool.file.read` | `Read` | `Read` | `read_files` shim-required | `read` shim-required | `Read` direct |
| `tool.file.search` | `Grep` | `Grep` | `search_files` shim-required | `grep` shim-required | `Grep` direct |
| `tool.file.glob` | `Glob` | `Glob` | `list_files` shim-required | `glob` shim-required | report-only |
| `tool.file.write` | `apply_patch\|Write\|Edit` | `Write\|Edit\|MultiEdit` | `editor\|write_file\|apply_patch` shim-required | `edit\|write\|apply_patch` shim-required | `Write` direct |
| `tool.web.fetch` | `WebFetch` | `WebFetch` | report-only | `webfetch` shim-required | report-only |
| `tool.web.search` | `WebSearch` | `WebSearch` | report-only | `websearch` shim-required | report-only |
| `tool.agent` | `spawn_agent\|Agent` | `Agent` | report-only | report-only | `Task` direct |
| `tool.mcp` | `mcp__<server>__<tool>` | `mcp__<server>__<tool>` | report-only | `<server>_<tool>` shim-required | report-only |

For file-write hooks, AgentSync should prefer the target's broad edit class when
translating from a patch-specific source. For example, a Codex `apply_patch`
hook targeting Claude Code should render as `Write|Edit|MultiEdit` unless the
source hook explicitly depends on patch text. If the script parses patch marker
lines from `tool_input.command`, AgentSync should warn that non-patch target
tools may not provide that payload.

## Handler Field Policy

AgentSync can render a hook handler only when these fields are safe:

- `type = "command"`: direct between Codex, Claude, and Cursor when
  command-output decision semantics are compatible.
- `command`: preserve exactly unless path rewriting is explicitly implemented.
- `timeout`: render when the target supports the same units and cancellation
  behavior.
- `statusMessage`: render when supported; otherwise report as omitted.
- `async`, `args`, `shell`, `if`, HTTP hooks, MCP-tool hooks, prompt hooks, and
  agent hooks: report-only until the target-specific semantics are implemented.

When rendering into Cursor, `statusMessage` is reported and omitted because
Cursor hook definitions do not support it. Cursor `failClosed`, `loop_limit`,
prompt hooks, and Cursor-only matcher surfaces are report-only.

When rendering into OpenCode, every command hook is `shim-required`; the source
command is reused, but wrapper generation adapts stdin/stdout. The current shim
does not rewrite OpenCode tool args or compact prompts from hook stdout; those
native output mutations remain report-only.

When rendering into Cline, every command hook is `shim-required`; the source
command is reused, but wrapper generation adapts stdin/stdout and converts
blocking output into Cline runtime hook results. Native Cline plugins, MCP
servers, permission policies, and custom plugin tools remain blocked unless
they are the generated AgentSync hook shim.

## Planning Rules

1. Normalize hook entries into canonical event plus canonical matcher class.
2. Render entries with `direct` mappings when all handler fields are supported.
3. Render `shim-required` entries only when an implemented shim covers the
   source event, matcher, and handler fields.
4. Preserve report-only entries in diagnostics and native extensions.
5. Mark the hook resource `partial` when at least one entry is rendered and at
   least one entry is omitted or shimmed.
6. Mark the hook resource `blocked` when no entry can be safely rendered.
7. Never write generated hook files without explicit `--write`.
8. Never execute hook commands during scan, diff, sync, or validation.

## Source Notes

- Claude Code documents JSON hook configuration, tool-event matchers, command
  hooks, stdin payloads, and blocking output semantics:
  https://code.claude.com/docs/en/hooks
- Codex CLI hook events and matcher aliases are documented in the current
  Codex manual. As of July 2, 2026, Codex exposes
  `PreToolUse`, `PermissionRequest`, `PostToolUse`, `PreCompact`,
  `PostCompact`, `SessionStart`, `UserPromptSubmit`, `SubagentStart`,
  `SubagentStop`, and `Stop`, and treats `apply_patch` as the canonical edit
  hook name with `Write` and `Edit` matcher aliases:
  https://developers.openai.com/codex/hooks
- AgentSync also treats `exec_command` as a Codex-compatible shell matcher
  because Codex API sessions can expose the shell tool under that name.
- OpenCode documents local `.opencode/plugins` auto-loading plus plugin hooks
  and event names, including `tool.execute.before`, `tool.execute.after`,
  `permission.ask`, `session.created`, and
  `experimental.session.compacting`:
  https://opencode.ai/docs/plugins/
- The OpenCode plugin package type exposes the decision-capable
  `permission.ask` hook used by AgentSync for permission request shims:
  https://github.com/sst/opencode/blob/dev/packages/plugin/src/index.ts
- OpenCode documents built-in tool names including `bash`, `edit`, `write`,
  `read`, `grep`, `glob`, `apply_patch`, `webfetch`, and `websearch`:
  https://opencode.ai/docs/tools/
- Cursor documents `.cursor/hooks.json`, command-based hooks, supported events,
  cloud-agent hook support, prompt hooks, and lifecycle events including
  `sessionEnd`, `beforeShellExecution`, `afterFileEdit`, and MCP hooks:
  https://cursor.com/docs/hooks
- Cursor documents Claude Code hook compatibility and the Claude-to-Cursor event
  and tool-name mappings:
  https://cursor.com/docs/reference/third-party-hooks
- Cline documents project/global configuration layout, including `.cline/rules`,
  `.cline/hooks`, `.cline/agents`, `.cline/plugins`, and related global paths:
  https://docs.cline.bot/getting-started/config
- Cline SDK plugins expose runtime hooks including `beforeRun`, `afterRun`,
  `beforeTool`, and `afterTool`:
  https://docs.cline.bot/sdk/plugins
- Cline file hook examples map `PreToolUse`, `PostToolUse`,
  `UserPromptSubmit`, `TaskStart`, and `TaskComplete` onto runtime hooks:
  https://github.com/cline/cline/blob/main/sdk/examples/hooks/README.md
