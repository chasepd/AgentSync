# Adapters

Adapters isolate native agent knowledge from the core sync engine.

## Contract

Each adapter provides:

- `discover(root, scope)`: find native files.
- `read(native)`: parse a native file into a normalized resource.
- `render(resource)`: produce target native files plus diagnostics.
- `capabilities()`: declare supported resources and fields.
- `validate(root, scope)`: report parse, config, and compatibility issues.

## Initial Adapters

- Claude Code: `CLAUDE.md`, `.claude/agents`, `.claude/skills`, settings hooks.
- Codex CLI: `AGENTS.md`, skills, plugins, subagents, Codex customization.
- Cursor CLI: CLI-consumed rules in `.cursor/rules`, root `AGENTS.md` /
  `CLAUDE.md` where supported, and portable skills. This target is not Cursor
  IDE workspace automation.
- OpenCode: `AGENTS.md`, `.opencode/agents`, `.opencode/commands`,
  `.opencode/skills`, `opencode.json` instructions, and `opencode.json`
  command config.
