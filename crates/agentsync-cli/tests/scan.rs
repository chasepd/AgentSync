use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn scan_uses_config_scope_when_scope_is_omitted() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
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
        .args(["scan", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["scope"], "all");
    assert!(json["diagnostics"][0]["message"]
        .as_str()
        .unwrap()
        .contains("user scope scanning is not implemented yet"));
    assert!(json["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| resource["path"] == "AGENTS.md"));
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
fn scan_json_includes_normalized_subagent_fields() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
    fs::write(
        dir.path().join(".claude/agents/reviewer.md"),
        "---\nname: reviewer\ndescription: Review code\n---\nReview carefully.\n",
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
