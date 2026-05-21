use std::fs;

use assert_cmd::Command;
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
