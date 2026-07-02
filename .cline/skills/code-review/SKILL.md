---
description: Use when completing tasks, implementing major features, or before merging to verify AgentSync changes meet requirements
name: code-review
---

# Requesting Code Review

Dispatch a fresh code-review pass to catch correctness, safety, compatibility, and test gaps before changes land.

Core principle: review early, review concretely.

## When to Request Review

Mandatory:
- After completing a major implementation slice
- Before opening or merging a PR
- After fixing drift, sync, state, adapter, or write-safety behavior

Optional but useful:
- When a bug fix touches shared core logic
- Before refactoring adapters, state, reports, or CLI contracts
- When validation passes but behavior feels under-specified

## How to Request

1. Get the review range:

```bash
BASE_SHA=$(git merge-base HEAD origin/main)
HEAD_SHA=$(git rev-parse HEAD)
```

2. Run baseline context commands:

```bash
git diff --stat "$BASE_SHA..$HEAD_SHA"
git diff "$BASE_SHA..$HEAD_SHA"
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

3. Use `code-reviewer.md` in this directory as the reviewer prompt template.

Fill in:
- `{WHAT_WAS_IMPLEMENTED}`: concise description of the change
- `{PLAN_OR_REQUIREMENTS}`: intended behavior, acceptance criteria, or linked plan
- `{BASE_SHA}`: starting commit
- `{HEAD_SHA}`: ending commit
- `{DESCRIPTION}`: short implementation summary

4. Validate findings before acting on them.

Every finding must be grounded in code at `{HEAD_SHA}`. Drop or downgrade findings that cannot cite a concrete line and explain why it is wrong.

Validation checklist:
- Does the finding quote or reference the exact relevant code, not a paraphrase?
- Is the cited file read from the review SHA, not a stale working tree?
- Does the issue match AgentSync’s safety model: no writes without explicit `--write`, no silent loss of unsupported behavior, no unsafe drift overwrite?
- If it claims a missing check, did the reviewer trace one helper level deep?
- If it claims a test gap, is the behavior actually reachable through public APIs or CLI?

5. Fix important issues and re-review if needed.

Use a fresh reviewer after fixing Critical or Important findings. Do not pass prior findings to the next reviewer; compare whether the number of Critical/Important issues decreased.

## AgentSync-Specific Red Flags

Treat these as Important or Critical unless proven harmless:

- Generated files can be written without explicit `--write`.
- Dry-run, diff, scan, or status mutates user files or state.
- Drifted target files are overwritten without an explicit conflict strategy.
- Unsupported behavioral resources, executable hooks, commands, plugins, or subagents are silently dropped or downgraded.
- `.agentsync/state.json` becomes a source of truth instead of native files.
- `status --check` fails on valid missing state or untracked resources.
- Adapter output is nondeterministic, making snapshot-style tests flaky.
- JSON output is string-built instead of serde-backed.
- Tests assert implementation details but miss CLI/user-visible behavior.

## Reporting

Lead with findings, ordered by severity. If there are no issues, say so directly and list residual risks or test gaps.

Required finding format:
- File:line
- Severity: Critical | Important | Minor | Suggestion
- What is wrong
- Why it matters
- Concrete fix

Keep summaries short. Do not bury findings under praise.

## Validation Commands

Use the repository validation gate:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
