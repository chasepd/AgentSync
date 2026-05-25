use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn scan_uses_config_scope_when_scope_is_omitted() {
    let dir = tempdir().unwrap();
    let user = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
    fs::create_dir_all(user.path().join(".codex")).unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
    fs::write(user.path().join(".codex/AGENTS.md"), "user rules\n").unwrap();
    fs::write(
        dir.path().join(".agentsync/config.toml"),
        r#"schema_version = 1

[defaults]
scope = "all"
"#,
    )
    .unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .env("HOME", user.path())
        .args(["scan", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["scope"], "all");
    assert!(json["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| resource["path"] == "AGENTS.md"));
    assert!(json["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| { resource["path"] == ".codex/AGENTS.md" && resource["scope"] == "user" }));
}

#[test]
fn explicit_scan_scope_does_not_require_valid_config() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
    fs::write(
        dir.path().join(".agentsync/config.toml"),
        "schema_version = 2\n",
    )
    .unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["scan", "--scope", "project", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["scope"], "project");
}

#[test]
fn scan_format_json_outputs_structured_report() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["scan", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["scope"], "project");
    assert!(json["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| resource["path"] == "AGENTS.md"));
}

#[test]
fn scan_table_groups_resources_by_kind() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
    fs::create_dir_all(dir.path().join(".claude/skills/review")).unwrap();
    fs::write(
        dir.path().join(".claude/skills/review/SKILL.md"),
        "---\nname: review\n---\nBody\n",
    )
    .unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["scan"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();

    let rules = text.find("\nrules\n").unwrap();
    let skills = text.find("\nskills\n").unwrap();
    assert!(rules < skills);
    assert!(text.contains("  codex    AGENTS.md\n"));
    assert!(text.contains("  claude   .claude/skills/review/SKILL.md\n"));
    assert!(!text.contains("rules      codex"));
}

#[test]
fn scan_json_includes_normalized_subagent_fields() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
    fs::write(
        dir.path().join(".claude/agents/reviewer.md"),
        r#"---
name: reviewer
description: Review code
model: sonnet
tools:
  - Read
  - Grep
---
Review carefully.
"#,
    )
    .unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["scan", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let subagent = json["normalized"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["id"] == "subagents:reviewer")
        .unwrap();

    assert_eq!(subagent["kind"], "subagent");
    assert_eq!(subagent["support"], "blocked");
    assert_eq!(subagent["subagent"]["name"], "reviewer");
    assert_eq!(subagent["subagent"]["description"], "Review code");
    assert_eq!(subagent["subagent"]["body"], "Review carefully.\n");
    assert_eq!(subagent["subagent"]["instructions"], "Review carefully.\n");
    assert_eq!(subagent["subagent"]["model"], "sonnet");
    assert_eq!(subagent["subagent"]["tools"]["allow"][0], "Read");
    assert!(subagent["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["message"] == "subagent.model: partial"));
    assert!(subagent["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["message"]
            .as_str()
            .unwrap()
            .contains("subagent.tools: blocked")));
}

#[test]
fn scan_json_includes_opencode_config_commands_as_blocked() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("opencode.json"),
        r#"{"command":{"deploy":{"template":"Deploy the app"}}}"#,
    )
    .unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["scan", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let command = json["normalized"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["id"] == "commands:opencode:opencode.json")
        .unwrap();

    assert_eq!(command["kind"], "command");
    assert_eq!(command["support"], "blocked");
    assert!(command["native_extensions"]["native.raw"]
        .as_str()
        .unwrap()
        .contains("\"command\""));
}

#[test]
fn scan_json_includes_opencode_plugins_as_blocked() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".opencode/plugins")).unwrap();
    fs::write(
        dir.path().join(".opencode/plugins/notify.js"),
        "export const Notify = async () => ({})\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("opencode.json"),
        r#"{"plugin":["opencode-wakatime"]}"#,
    )
    .unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["scan", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let normalized = json["normalized"].as_array().unwrap();

    assert!(normalized.iter().any(|resource| {
        resource["id"] == "plugins:opencode:.opencode/plugins/notify.js"
            && resource["kind"] == "plugin"
            && resource["support"] == "blocked"
    }));
    assert!(normalized.iter().any(|resource| {
        resource["id"] == "plugins:opencode:opencode.json"
            && resource["native_extensions"]["native.raw"]
                .as_str()
                .unwrap()
                .contains("\"plugin\"")
    }));
}

#[test]
fn scan_json_includes_opencode_jsonc_config_resources() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("opencode.jsonc"),
        r#"{
  // OpenCode supports comments and trailing commas here.
  "instructions": ["docs/rules.md"],
  "agent": {
    "code-reviewer": {
      "description": "Reviews code",
      "tools": {
        "write": false,
      },
    },
  },
  "command": {
    "deploy": {
      "template": "Deploy the app",
    },
  },
  "plugin": ["opencode-wakatime"],
}"#,
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::write(dir.path().join("docs/rules.md"), "project rules\n").unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["scan", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let normalized = json["normalized"].as_array().unwrap();

    assert!(normalized
        .iter()
        .any(|resource| resource["id"] == "rules:opencode:opencode.jsonc"));
    assert!(normalized
        .iter()
        .any(|resource| resource["id"] == "subagents:opencode:opencode.jsonc"));
    assert!(normalized
        .iter()
        .any(|resource| resource["id"] == "commands:opencode:opencode.jsonc"));
    assert!(normalized
        .iter()
        .any(|resource| resource["id"] == "plugins:opencode:opencode.jsonc"));
}

#[test]
fn scan_json_includes_opencode_config_agents_as_blocked() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("opencode.json"),
        r#"{"agent":{"code-reviewer":{"description":"Reviews code","tools":{"write":false}}}}"#,
    )
    .unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["scan", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let subagent = json["normalized"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["id"] == "subagents:opencode:opencode.json")
        .unwrap();

    assert_eq!(subagent["kind"], "subagent");
    assert_eq!(subagent["support"], "blocked");
    assert!(subagent["native_extensions"]["native.raw"]
        .as_str()
        .unwrap()
        .contains("\"agent\""));
}

#[test]
fn scan_json_includes_permission_config_as_blocked() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".claude")).unwrap();
    fs::write(
        dir.path().join(".claude/settings.json"),
        r#"{"permissions":{"allow":["Bash(git status)"]}}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("opencode.jsonc"),
        r#"{
  "permission": {
    "edit": "ask",
  },
}"#,
    )
    .unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["scan", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let normalized = json["normalized"].as_array().unwrap();

    assert!(normalized.iter().any(|resource| {
        resource["id"] == "permissions:claude:.claude/settings.json"
            && resource["kind"] == "permission"
            && resource["support"] == "blocked"
    }));
    assert!(normalized.iter().any(|resource| {
        resource["id"] == "permissions:opencode:opencode.jsonc"
            && resource["native_extensions"]["native.raw"]
                .as_str()
                .unwrap()
                .contains("\"permission\"")
    }));
}
