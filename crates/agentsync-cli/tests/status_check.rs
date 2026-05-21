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
