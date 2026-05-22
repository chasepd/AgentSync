use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn status_check_succeeds_for_untracked_resources() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["status", "--check"])
        .assert()
        .success();
}

#[test]
fn status_check_fails_for_blocked_behavioral_resources() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".opencode/commands")).unwrap();
    fs::write(dir.path().join(".opencode/commands/deploy.md"), "deploy\n").unwrap();
    fs::create_dir_all(dir.path().join(".claude")).unwrap();
    fs::write(
        dir.path().join(".claude/settings.json"),
        r#"{"hooks":{"PreToolUse":[]}}"#,
    )
    .unwrap();
    fs::create_dir_all(dir.path().join(".opencode/plugins")).unwrap();
    fs::write(
        dir.path().join(".opencode/plugins/notify.js"),
        "export const Notify = async () => ({})\n",
    )
    .unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["status", "--check"])
        .assert()
        .failure();
}

#[test]
fn status_json_outputs_structured_report() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["status", "--json", "--check"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["scope"], "project");
    assert_eq!(json["items"][0]["state"], "untracked");
}

#[test]
fn status_format_json_outputs_structured_report() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["status", "--format", "json", "--check"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["scope"], "project");
    assert_eq!(json["items"][0]["state"], "untracked");
}

#[test]
fn status_uses_config_scope_when_scope_is_omitted() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
    fs::write(
        dir.path().join(".agentsync/config.toml"),
        r#"schema_version = 1

[defaults]
scope = "user"
"#,
    )
    .unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["status", "--json", "--check"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["scope"], "user");
    assert!(json["diagnostics"][0]["message"]
        .as_str()
        .unwrap()
        .contains("user scope scanning is not implemented yet"));
}

#[test]
fn explicit_status_scope_does_not_require_valid_config() {
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
        .args(["status", "--scope", "project", "--json", "--check"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["scope"], "project");
}

#[test]
fn status_check_reports_missing_synced_target() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
    sync_rules_to_claude(dir.path());
    fs::remove_file(dir.path().join("CLAUDE.md")).unwrap();

    let json = status_json_check_failure(dir.path());

    assert_eq!(json["items"][0]["state"], "missing_target");
    assert_eq!(
        json["items"][0]["suggested_command"],
        "agentsync diff rules --from agents-md --to claude"
    );
}

#[test]
fn status_check_reports_stale_synced_target() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
    sync_rules_to_claude(dir.path());
    fs::write(dir.path().join("CLAUDE.md"), "local target edit\n").unwrap();

    let json = status_json_check_failure(dir.path());

    assert_eq!(json["items"][0]["state"], "stale_target");
    assert_eq!(
        json["items"][0]["suggested_command"],
        "agentsync diff rules --from agents-md --to claude"
    );
}

#[test]
fn status_check_reports_changed_source() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
    sync_rules_to_claude(dir.path());
    fs::write(dir.path().join("AGENTS.md"), "new repo rules\n").unwrap();

    let json = status_json_check_failure(dir.path());

    assert_eq!(json["items"][0]["state"], "changed_source");
    assert_eq!(
        json["items"][0]["suggested_command"],
        "agentsync diff rules --from agents-md --to claude"
    );
}

fn sync_rules_to_claude(dir: &std::path::Path) {
    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir)
        .args([
            "sync",
            "rules",
            "--from",
            "agents-md",
            "--to",
            "claude",
            "--write",
        ])
        .assert()
        .success();
}

fn status_json_check_failure(dir: &std::path::Path) -> Value {
    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir)
        .args(["status", "--json", "--check"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}
