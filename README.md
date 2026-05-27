# AgentSync

Keep AI coding-agent configuration in sync across Claude Code, Codex CLI, Cursor CLI, OpenCode, and other `AGENTS.md`-compatible tools.

AgentSync scans your personal and project-level agent configuration, shows what exists in each tool's native format, and helps you safely sync rules, subagents, hooks, skills, commands, and related automation across ecosystems.

No more hand-copying `.claude/agents`, rewriting hooks by hand, or maintaining three almost-identical versions of the same repo instructions because every agent decided to be special.

> Status: v0.1 MVP. Rules, portable skills, prompt-only OpenCode commands, and supported subagents can be planned and synced; security-sensitive behavior is reported but blocked unless safe render semantics exist.

## Why

Modern coding agents are useful, but their configuration formats are fragmented:

- Claude Code uses `CLAUDE.md`, skills, hooks, subagents, and settings files.
- Codex CLI uses `AGENTS.md`, skills, plugins, subagents, and Codex-specific customization.
- Cursor CLI has rules, skills, subagents, hooks, and CLI behavior.
- OpenCode uses `AGENTS.md`, agents, commands, skills, plugins, and OpenCode config.

Teams end up with duplicated instructions, stale subagents, missing hooks, and subtle drift between tools.

AgentSync gives teams one way to inspect, compare, and sync those files without forcing everyone to use the same coding agent.

## What AgentSync does

AgentSync can:

- Discover personal and project-level agent configuration.
- Show which formats each item is implemented in.
- Convert compatible config between supported agent formats.
- Detect drift between synced files.
- Preview diffs before writing changes.
- Preserve native files instead of replacing them with a proprietary runtime.
- Record sync metadata so future syncs know which files are linked.
- Support explicit source-of-truth and conflict-resolution workflows.

## Supported resources

Initial target adapter coverage:

Cursor support means **Cursor CLI**. AgentSync targets files the CLI consumes,
including the shared rules system in `.cursor/rules` and root-level
`AGENTS.md` / `CLAUDE.md`; it does not automate Cursor IDE workspace behavior.

| Resource | Claude Code | Codex CLI | Cursor CLI | OpenCode |
| --- | --- | --- | --- | --- |
| Rules / context | `CLAUDE.md`, `.claude/CLAUDE.md` | `AGENTS.md` | Cursor CLI rules, `AGENTS.md` where supported | `AGENTS.md`, `opencode.json` / `opencode.jsonc` instructions |
| Subagents / custom agents | `.claude/agents/*.md` | Codex subagents | blocked until Cursor publishes stable CLI file-format docs | `.opencode/agents/*.md`, `opencode.json` agent config |
| Skills | `.claude/skills/*/SKILL.md` | Agent Skills / `SKILL.md` folders | Agent Skills / `SKILL.md` folders | `.opencode/skills/*/SKILL.md`, `.agents/skills`, Claude-compatible skills |
| Hooks / lifecycle automation | Claude Code hooks in settings | blocked behavioral resource | blocked behavioral resource | OpenCode plugins and events |
| Commands | Claude skills / legacy commands | planned | planned | `.opencode/commands/*.md`, config commands |

Support levels:

- `read`: AgentSync can discover and display the resource.
- `write`: AgentSync can generate the native format.
- `sync`: AgentSync can track drift and reconcile changes.
- `partial`: Some fields cannot be represented exactly in the target format.

## Installation

Package manager support is planned. For now, install the Rust CLI from source.

```bash
# From source
git clone https://github.com/chasepd/AgentSync.git
cd AgentSync
cargo install --path crates/agentsync-cli
```

## Quick start

Run AgentSync in a repository:

```bash
agentsync scan
```

Example output:

```text
Project: ~/src/my-app

Rules / context
  AGENTS.md                      codex, opencode
  CLAUDE.md                      claude

Subagents
  security-reviewer              claude
  frontend-architect             claude, opencode
  migration-planner              codex

Skills
  pr-review                      claude, codex, opencode
  release-notes                  claude

Hooks
  block-env-reads                claude
  run-format-after-edit          claude, opencode
```

Sync every matching resource kind from the selected source:

```bash
agentsync sync --all --from claude --to codex,opencode
```

Or let AgentSync pick the changed source and render every target:

```bash
agentsync sync --all --from all --to all
```

Preview changes first:

```bash
agentsync sync --all --from claude --to codex,opencode --dry-run
```

Write the generated files:

```bash
agentsync sync --all --from claude --to codex,opencode --write
```

Check for drift later:

```bash
agentsync status
```

Example:

```text
Drift detected

  security-reviewer
    claude      changed 2 days ago
    codex       stale
    opencode    stale

Suggested action:
  agentsync sync subagent security-reviewer --from claude --to codex,opencode --write
```

## Common workflows

### Start from an existing Claude Code setup

```bash
agentsync scan
agentsync sync --all --from claude --to codex,opencode --dry-run
agentsync sync --all --from claude --to codex,opencode --write
```

### Keep AGENTS.md as the source of truth

```bash
agentsync sync rules --from agents-md --to claude,cursor,opencode --write
```

### Sync only one subagent

```bash
agentsync sync subagent security-reviewer --from claude --to codex,opencode --write
```

### Sync only skills

```bash
agentsync sync skills --from claude --to codex,cursor,opencode --write
```

### Scan personal config

```bash
agentsync scan --scope user
```

### Scan project config

```bash
agentsync scan --scope project
```

### Check drift in CI

```bash
agentsync status --check
```

## How it works

AgentSync does not try to make every agent use a single shared runtime.

Instead, it uses a four-step process:

1. **Discover**  
   Finds native agent files in user and project locations.

2. **Normalize**  
   Converts each resource into an internal intermediate representation.

3. **Compare**  
   Detects missing formats, changed source files, unsupported fields, and drift.

4. **Render**  
   Writes native files back out for each target agent.

This keeps each coding agent happy while giving humans a single workflow for managing the mess.

## User and project scopes

AgentSync understands two scopes:

| Scope | Purpose | Examples |
| --- | --- | --- |
| `project` | Team-shared repo config | `AGENTS.md`, `CLAUDE.md`, `.claude/`, `.cursor/`, `.opencode/` |
| `user` | Personal defaults | `~/.claude/`, `~/.codex/`, `~/.config/opencode/`, Cursor CLI user-level config |

Use `--scope project`, `--scope user`, or `--scope all` to control what gets scanned or synced.

```bash
agentsync scan --scope all
agentsync status --scope project
agentsync sync --scope user --from claude --to codex,opencode --write
```

## Sync metadata

AgentSync writes lightweight metadata so it can track relationships between generated files.

By default, project metadata is stored in:

```text
.agentsync/state.json
```

Generated files may also include a small comment or frontmatter marker when the target format allows it:

```yaml
agentsync:
  id: security-reviewer
  source: claude
  synced_at: 2026-05-20T00:00:00Z
```

AgentSync will not overwrite manually changed files without showing a diff.

## Conflict handling

When multiple formats change, AgentSync can reconcile them with an explicit strategy.

Use one format as source of truth:

```bash
agentsync sync --all --from claude --to codex,opencode --write
```

Sync from whichever tracked source changed:

```bash
agentsync sync --all --from all --to all --write
```

Prefer the newest changed file:

```bash
agentsync sync --all --strategy newest --write
```

Open an interactive conflict resolver:

```bash
agentsync sync --all --interactive
```

Refuse to overwrite drifted files:

```bash
agentsync sync --all --no-overwrite
```

AgentSync should be boringly safe by default:

- Dry run unless `--write` is passed.
- Show diffs before overwriting.
- Back up changed files.
- Preserve unknown fields where possible.
- Warn when a conversion loses semantics.

## Configuration

Create an optional config file:

```bash
agentsync init --write
```

`agentsync init` without `--write` previews the config and does not write files.
Existing `.agentsync/config.toml` files are left untouched.

Example `.agentsync/config.toml`:

```toml
schema_version = 1

[defaults]
scope = "project"
source = "agents-md"
targets = ["claude", "cursor", "opencode"]

[sync]
rules = true
skills = true
subagents = false
commands = false
hooks = false
```

With those defaults, `scan` and `status` can omit `--scope`, while `diff` and
`sync` can omit `--from` and `--to`:

```bash
agentsync scan
agentsync status
agentsync diff rules
agentsync sync rules --write
agentsync sync --all --write
```

`[sync]` toggles gate config-driven planning for that resource kind, so setting
`rules = false` or `skills = false` blocks those default-based commands before
any write. New configs default blocked behavioral resources to disabled:
`subagents = false`, `commands = false`, and `hooks = false`.

Use `source = "all"` and `targets = ["all"]` when contributors may edit
different native tools and you want `agentsync sync --all --write` to deploy the
changed source to every supported target. If multiple sources for the same
resource changed differently, AgentSync stops and asks for an explicit `--from`.
On repos without existing `.agentsync/state.json` metadata, use an explicit
`--from` for the first sync if native files already disagree.

## Conversion notes

Not every agent feature maps perfectly.

Examples:

- A Claude Code subagent written as Markdown frontmatter may not map perfectly to another tool's custom-agent schema.
- A sandbox or permission policy in one tool may not have an equivalent in another.
- Hook systems vary widely. Some are declarative config, while others are executable plugin code.
- Cursor CLI rules and `AGENTS.md` instructions may overlap but are not always equivalent.
- OpenCode `opencode.json` / `opencode.jsonc` rules are read from literal `instructions` file paths.
  Glob patterns are reported as partial until AgentSync grows deterministic glob expansion.
- Skills are most portable when they follow the open `SKILL.md` pattern, keep
  related assets inside the skill folder, and avoid agent-specific frontmatter.

When a conversion is lossy, AgentSync marks it clearly:

```text
WARN security-reviewer: target does not support field `color`.
WARN block-env-reads: target hook requires plugin wrapper generation.
```

## CLI reference

```bash
agentsync scan
agentsync status
agentsync diff
agentsync sync --all
agentsync init [--write]
agentsync doctor
```

### `scan`

Discover agent configuration.

```bash
agentsync scan
agentsync scan --scope user
agentsync scan --scope project
agentsync scan --json
agentsync scan --format json
```

### `status`

Show missing formats and drift.

```bash
agentsync status
agentsync status --format table
agentsync status --format json
agentsync status --check
```

### `diff`

Preview changes.

```bash
agentsync diff --from claude --to codex
agentsync diff rules --from claude --to codex --format json
agentsync diff subagent security-reviewer --from claude --to opencode
```

### `sync`

Generate or update target formats.

```bash
agentsync sync --all --from claude --to codex,opencode --dry-run
agentsync sync --all --from claude --to codex,opencode --write
agentsync sync --all --from all --to all --write
agentsync sync rules --from claude --to codex --format json
agentsync sync skill pr-review --from claude --to codex --write
agentsync sync --all --write
```

### `doctor`

Validate local setup.

```bash
agentsync doctor
agentsync doctor --format json
```

Checks include:

- Supported files are parseable.
- Generated files are not stale.
- Sync metadata is valid.
- Target directories exist.
- Potentially lossy conversions are flagged.

## Repository layout

Example repo using multiple agent formats:

```text
.
├── AGENTS.md
├── CLAUDE.md
├── .agentsync/
│   ├── config.toml
│   └── state.json
├── .claude/
│   ├── agents/
│   ├── skills/
│   └── settings.json
├── .cursor/
│   └── rules/
├── .opencode/
│   ├── agents/
│   ├── commands/
│   ├── plugins/
│   └── skills/
└── opencode.json
```

## CI usage

Use AgentSync in CI to prevent drift:

```bash
agentsync status --check
```

This repository includes `.github/workflows/agentsync-drift.yml`. Example
GitHub Action:

```yaml
name: AgentSync Drift

on:
  pull_request:
  push:
    branches: [main]

jobs:
  agentsync:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: 1.95.0
      - run: cargo install --path crates/agentsync-cli
      - run: agentsync status --check
```

### Reusable auto-sync PR workflow

AgentSync also ships a reusable workflow that can sync native agent files and
open or update a pull request when generated files change.

```yaml
name: AgentSync Auto Sync

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: write
  pull-requests: write

jobs:
  sync:
    uses: chasepd/AgentSync/.github/workflows/agentsync-sync-pr.yml@main
    with:
      branch: agentsync/auto-sync
      pr-title: Sync agent configuration
```

By default, the reusable workflow installs AgentSync from `chasepd/AgentSync`
at `main` and runs `agentsync sync --all --from all --to all --write`. Pin
`agentsync-ref` to a release tag or commit for stricter reproducibility. The
default `GITHUB_TOKEN` can create the sync branch and PR when the caller grants
`contents: write` and `pull-requests: write` and the repository enables
**Settings -> Actions -> General -> Workflow permissions -> Allow GitHub Actions
to create and approve pull requests**.

Pass a custom `token` secret if your repo cannot enable that setting or needs
PR-created workflows to trigger:

```yaml
    secrets:
      token: ${{ secrets.AGENTSYNC_SYNC_TOKEN }}
```

Run this only from trusted events, such as `push` to the default branch,
`schedule`, or `workflow_dispatch`; the configured `sync-command` runs with the
caller workflow's write token.

## Design principles

- Native files over proprietary lock-in.
- Dry-run first.
- Make drift visible.
- Never silently drop behavior.
- Prefer open formats where possible.
- Treat hooks as security-sensitive.
- Let teams choose their source of truth.
- Make multi-agent repos less annoying.

## Security

Hooks, plugins, commands, and executable skills can run code. AgentSync treats them as sensitive.

AgentSync will:

- Report hooks, plugins, permissions, and executable behavior as blocked.
- Preserve native fields where possible.
- Refuse to auto-run generated scripts.
- Sync only resources with safe render semantics.

Review blocked behavioral diagnostics before recreating those behaviors by hand.

## Roadmap

- [x] Read-only scanner for Claude Code, Codex CLI, Cursor CLI, and OpenCode.
- [x] Project and user scope discovery.
- [x] Rules/context sync.
- [x] Skills sync using `SKILL.md` plus portable text assets in the skill folder.
- [x] Resource-targeted `diff` and `sync`.
- [x] No-overwrite and conflict strategy safety controls.
- [x] Claude Code subagent to Codex/OpenCode conversion for portable fields.
- [x] OpenCode agent and prompt-only command rendering.
- [x] Hook, plugin, permission, and executable-command diagnostics with unsafe behavior blocked.
- [x] Interactive drift resolver.
- [x] CI mode for detecting stale generated agent config.
- [x] Adapter registry extension boundary for external adapters.
- [ ] Packaged npm/Homebrew releases.
- [ ] Full external adapter plugin loading with new agent identifiers.
- [ ] Support for Gemini CLI, GitHub Copilot instructions, Aider conventions, Zed, and VS Code agent config.

## Contributing

Contributions are welcome, especially format adapters for additional agents.

Useful contribution areas:

- Format parsers
- Renderers
- Test fixtures
- Conversion compatibility tables
- Real-world repo examples
- Safety checks for hooks and executable skills

## License

MIT
