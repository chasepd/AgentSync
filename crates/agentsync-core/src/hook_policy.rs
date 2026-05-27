use crate::model::Agent;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookMappingSupport {
    Direct,
    ShimRequired,
    ReportOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentHookMapping {
    pub agent: Agent,
    pub native: Option<&'static str>,
    pub support: HookMappingSupport,
    pub note: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookEquivalence {
    pub canonical: &'static str,
    pub purpose: &'static str,
    pub mappings: &'static [AgentHookMapping],
}

const fn direct(agent: Agent, native: &'static str) -> AgentHookMapping {
    AgentHookMapping {
        agent,
        native: Some(native),
        support: HookMappingSupport::Direct,
        note: "",
    }
}

const fn shim(agent: Agent, native: &'static str, note: &'static str) -> AgentHookMapping {
    AgentHookMapping {
        agent,
        native: Some(native),
        support: HookMappingSupport::ShimRequired,
        note,
    }
}

const fn report_only(
    agent: Agent,
    native: Option<&'static str>,
    note: &'static str,
) -> AgentHookMapping {
    AgentHookMapping {
        agent,
        native,
        support: HookMappingSupport::ReportOnly,
        note,
    }
}

pub const HOOK_EVENT_EQUIVALENCE: &[HookEquivalence] = &[
    HookEquivalence {
        canonical: "tool.before",
        purpose: "before an agent tool executes; may block or rewrite input",
        mappings: &[
            direct(Agent::Codex, "PreToolUse"),
            direct(Agent::Claude, "PreToolUse"),
            shim(
                Agent::OpenCode,
                "tool.execute.before",
                "requires a generated plugin wrapper and payload adapter",
            ),
            report_only(
                Agent::CursorCli,
                None,
                "Cursor CLI has no documented hook surface",
            ),
        ],
    },
    HookEquivalence {
        canonical: "tool.after",
        purpose: "after an agent tool succeeds; useful for audit or follow-up validation",
        mappings: &[
            direct(Agent::Codex, "PostToolUse"),
            direct(Agent::Claude, "PostToolUse"),
            shim(
                Agent::OpenCode,
                "tool.execute.after",
                "requires a generated plugin wrapper and payload adapter",
            ),
            report_only(
                Agent::CursorCli,
                None,
                "Cursor CLI has no documented hook surface",
            ),
        ],
    },
    HookEquivalence {
        canonical: "permission.request",
        purpose: "when a tool permission or approval request is about to be shown",
        mappings: &[
            direct(Agent::Codex, "PermissionRequest"),
            direct(Agent::Claude, "PermissionRequest"),
            shim(
                Agent::OpenCode,
                "permission.asked",
                "permission decision contracts are not JSON-hook compatible",
            ),
            report_only(
                Agent::CursorCli,
                None,
                "Cursor CLI exposes permissions, not hooks",
            ),
        ],
    },
    HookEquivalence {
        canonical: "session.start",
        purpose: "when a new or resumed agent session starts",
        mappings: &[
            direct(Agent::Codex, "SessionStart"),
            direct(Agent::Claude, "SessionStart"),
            shim(
                Agent::OpenCode,
                "session.created",
                "startup source and payload shape differ",
            ),
            report_only(
                Agent::CursorCli,
                None,
                "Cursor CLI has no documented hook surface",
            ),
        ],
    },
    HookEquivalence {
        canonical: "prompt.submit",
        purpose: "when the user submits a prompt before the model turn begins",
        mappings: &[
            direct(Agent::Codex, "UserPromptSubmit"),
            direct(Agent::Claude, "UserPromptSubmit"),
            report_only(
                Agent::OpenCode,
                Some("tui.prompt.append"),
                "not equivalent to a submitted prompt interception hook",
            ),
            report_only(
                Agent::CursorCli,
                None,
                "Cursor CLI has no documented hook surface",
            ),
        ],
    },
    HookEquivalence {
        canonical: "compact.before",
        purpose: "before conversation compaction is generated",
        mappings: &[
            direct(Agent::Codex, "PreCompact"),
            direct(Agent::Claude, "PreCompact"),
            shim(
                Agent::OpenCode,
                "experimental.session.compacting",
                "experimental plugin hook; wrapper must adapt payload and output",
            ),
            report_only(
                Agent::CursorCli,
                None,
                "Cursor CLI has no documented hook surface",
            ),
        ],
    },
    HookEquivalence {
        canonical: "compact.after",
        purpose: "after conversation compaction completes",
        mappings: &[
            direct(Agent::Codex, "PostCompact"),
            report_only(
                Agent::Claude,
                None,
                "Claude Code has no documented PostCompact hook event",
            ),
            shim(
                Agent::OpenCode,
                "session.compacted",
                "plugin event is observable but not JSON-hook compatible",
            ),
            report_only(
                Agent::CursorCli,
                None,
                "Cursor CLI has no documented hook surface",
            ),
        ],
    },
    HookEquivalence {
        canonical: "agent.start",
        purpose: "when a nested or background agent starts",
        mappings: &[
            direct(Agent::Codex, "SubagentStart"),
            direct(Agent::Claude, "SubagentStart"),
            report_only(
                Agent::OpenCode,
                None,
                "no documented equivalent plugin event",
            ),
            report_only(
                Agent::CursorCli,
                None,
                "Cursor CLI has no documented hook surface",
            ),
        ],
    },
    HookEquivalence {
        canonical: "agent.stop",
        purpose: "when a nested or background agent finishes",
        mappings: &[
            direct(Agent::Codex, "SubagentStop"),
            direct(Agent::Claude, "SubagentStop"),
            report_only(
                Agent::OpenCode,
                None,
                "no documented equivalent plugin event",
            ),
            report_only(
                Agent::CursorCli,
                None,
                "Cursor CLI has no documented hook surface",
            ),
        ],
    },
    HookEquivalence {
        canonical: "session.stop",
        purpose: "when the agent is about to finish a turn or session",
        mappings: &[
            direct(Agent::Codex, "Stop"),
            direct(Agent::Claude, "Stop"),
            report_only(
                Agent::OpenCode,
                Some("session.idle"),
                "session idle is observable but not equivalent to a blocking stop hook",
            ),
            report_only(
                Agent::CursorCli,
                None,
                "Cursor CLI has no documented hook surface",
            ),
        ],
    },
];

pub const HOOK_TOOL_EQUIVALENCE: &[HookEquivalence] = &[
    HookEquivalence {
        canonical: "tool.shell",
        purpose: "shell command execution",
        mappings: &[
            direct(Agent::Codex, "Bash|exec_command"),
            direct(Agent::Claude, "Bash"),
            shim(
                Agent::OpenCode,
                "bash",
                "OpenCode plugin wrapper must adapt args.command into JSON hook stdin",
            ),
            report_only(Agent::CursorCli, None, "Cursor CLI has no documented hook matcher surface"),
        ],
    },
    HookEquivalence {
        canonical: "tool.file.read",
        purpose: "read file contents",
        mappings: &[
            direct(Agent::Codex, "Read"),
            direct(Agent::Claude, "Read"),
            shim(
                Agent::OpenCode,
                "read",
                "OpenCode plugin wrapper must adapt args.filePath into JSON hook stdin",
            ),
            report_only(Agent::CursorCli, None, "Cursor CLI has no documented hook matcher surface"),
        ],
    },
    HookEquivalence {
        canonical: "tool.file.search",
        purpose: "search file contents",
        mappings: &[
            direct(Agent::Codex, "Grep"),
            direct(Agent::Claude, "Grep"),
            shim(
                Agent::OpenCode,
                "grep",
                "OpenCode plugin wrapper must adapt grep arguments into JSON hook stdin",
            ),
            report_only(Agent::CursorCli, None, "Cursor CLI has no documented hook matcher surface"),
        ],
    },
    HookEquivalence {
        canonical: "tool.file.glob",
        purpose: "list files by glob pattern",
        mappings: &[
            direct(Agent::Codex, "Glob"),
            direct(Agent::Claude, "Glob"),
            shim(
                Agent::OpenCode,
                "glob",
                "OpenCode plugin wrapper must adapt glob arguments into JSON hook stdin",
            ),
            report_only(Agent::CursorCli, None, "Cursor CLI has no documented hook matcher surface"),
        ],
    },
    HookEquivalence {
        canonical: "tool.file.write",
        purpose: "create or modify files",
        mappings: &[
            direct(Agent::Codex, "apply_patch|Write|Edit"),
            direct(Agent::Claude, "Write|Edit|MultiEdit"),
            shim(
                Agent::OpenCode,
                "edit|write|apply_patch",
                "OpenCode uses lowercase tools and different edit/patch argument shapes",
            ),
            report_only(Agent::CursorCli, None, "Cursor CLI has no documented hook matcher surface"),
        ],
    },
    HookEquivalence {
        canonical: "tool.web.fetch",
        purpose: "fetch a specific URL or web page",
        mappings: &[
            direct(Agent::Codex, "WebFetch"),
            direct(Agent::Claude, "WebFetch"),
            shim(
                Agent::OpenCode,
                "webfetch",
                "OpenCode plugin wrapper must adapt webfetch arguments into JSON hook stdin",
            ),
            report_only(Agent::CursorCli, None, "Cursor CLI has no documented hook matcher surface"),
        ],
    },
    HookEquivalence {
        canonical: "tool.web.search",
        purpose: "search the web",
        mappings: &[
            direct(Agent::Codex, "WebSearch"),
            direct(Agent::Claude, "WebSearch"),
            shim(
                Agent::OpenCode,
                "websearch",
                "OpenCode plugin wrapper must adapt websearch arguments into JSON hook stdin",
            ),
            report_only(Agent::CursorCli, None, "Cursor CLI has no documented hook matcher surface"),
        ],
    },
    HookEquivalence {
        canonical: "tool.agent",
        purpose: "spawn or call a subagent",
        mappings: &[
            direct(Agent::Codex, "spawn_agent|Agent"),
            direct(Agent::Claude, "Agent"),
            report_only(Agent::OpenCode, None, "no stable documented equivalent tool matcher"),
            report_only(Agent::CursorCli, None, "Cursor CLI has no documented hook matcher surface"),
        ],
    },
    HookEquivalence {
        canonical: "tool.mcp",
        purpose: "tools exposed by MCP servers",
        mappings: &[
            direct(Agent::Codex, "mcp__<server>__<tool>"),
            direct(Agent::Claude, "mcp__<server>__<tool>"),
            shim(
                Agent::OpenCode,
                "<server>_<tool>",
                "OpenCode permission docs use server_tool wildcards; verify plugin input.tool before rendering",
            ),
            report_only(Agent::CursorCli, None, "Cursor CLI supports MCP but has no documented hook matcher surface"),
        ],
    },
];

pub fn hook_event_equivalence(canonical: &str) -> Option<&'static HookEquivalence> {
    HOOK_EVENT_EQUIVALENCE
        .iter()
        .find(|policy| policy.canonical == canonical)
}

pub fn hook_tool_equivalence(canonical: &str) -> Option<&'static HookEquivalence> {
    HOOK_TOOL_EQUIVALENCE
        .iter()
        .find(|policy| policy.canonical == canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_hook_policy_row_covers_all_supported_agents() {
        for policy in HOOK_EVENT_EQUIVALENCE
            .iter()
            .chain(HOOK_TOOL_EQUIVALENCE.iter())
        {
            for agent in Agent::ALL {
                assert!(
                    policy.mappings.iter().any(|mapping| mapping.agent == agent),
                    "{} is missing {:?}",
                    policy.canonical,
                    agent
                );
            }
        }
    }

    #[test]
    fn tool_events_map_directly_between_codex_and_claude() {
        let policy = hook_event_equivalence("tool.before").unwrap();

        assert!(policy.mappings.iter().any(|mapping| {
            mapping.agent == Agent::Codex
                && mapping.native == Some("PreToolUse")
                && mapping.support == HookMappingSupport::Direct
        }));
        assert!(policy.mappings.iter().any(|mapping| {
            mapping.agent == Agent::Claude
                && mapping.native == Some("PreToolUse")
                && mapping.support == HookMappingSupport::Direct
        }));
        assert!(policy.mappings.iter().any(|mapping| {
            mapping.agent == Agent::OpenCode
                && mapping.native == Some("tool.execute.before")
                && mapping.support == HookMappingSupport::ShimRequired
        }));
    }

    #[test]
    fn file_write_policy_preserves_patch_and_edit_matchers() {
        let policy = hook_tool_equivalence("tool.file.write").unwrap();

        assert!(policy.mappings.iter().any(|mapping| {
            mapping.agent == Agent::Codex
                && mapping.native == Some("apply_patch|Write|Edit")
                && mapping.support == HookMappingSupport::Direct
        }));
        assert!(policy.mappings.iter().any(|mapping| {
            mapping.agent == Agent::Claude
                && mapping.native == Some("Write|Edit|MultiEdit")
                && mapping.support == HookMappingSupport::Direct
        }));
    }

    #[test]
    fn shell_policy_keeps_codex_exec_command_alias() {
        let policy = hook_tool_equivalence("tool.shell").unwrap();

        assert!(policy.mappings.iter().any(|mapping| {
            mapping.agent == Agent::Codex
                && mapping.native == Some("Bash|exec_command")
                && mapping.support == HookMappingSupport::Direct
        }));
    }

    #[test]
    fn cursor_cli_hook_policy_is_report_only() {
        for policy in HOOK_EVENT_EQUIVALENCE
            .iter()
            .chain(HOOK_TOOL_EQUIVALENCE.iter())
        {
            let cursor = policy
                .mappings
                .iter()
                .find(|mapping| mapping.agent == Agent::CursorCli)
                .unwrap();
            assert_eq!(cursor.support, HookMappingSupport::ReportOnly);
        }
    }
}
