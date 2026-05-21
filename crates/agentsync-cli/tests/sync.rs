use std::fs;

use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn sync_dry_run_does_not_write_targets_or_state() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "sync",
            "rules",
            "--from",
            "agents-md",
            "--to",
            "claude",
            "--dry-run",
        ])
        .assert()
        .success();

    assert!(!dir.path().join("CLAUDE.md").exists());
    assert!(!dir.path().join(".agentsync/state.json").exists());
}

#[test]
fn sync_without_write_does_not_write_targets_or_state() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["sync", "rules", "--from", "agents-md", "--to", "claude"])
        .assert()
        .success();

    assert!(!dir.path().join("CLAUDE.md").exists());
    assert!(!dir.path().join(".agentsync/state.json").exists());
}

#[test]
fn sync_write_creates_target_and_state() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
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

    assert_eq!(
        fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
        "repo rules\n"
    );
    assert!(dir.path().join(".agentsync/state.json").exists());
}

#[test]
fn cursor_cli_alias_is_accepted_for_targets() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["diff", "rules", "--from", "agents-md", "--to", "cursor-cli"])
        .assert()
        .success();
}
