use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn doctor_json_reports_missing_state_as_non_blocking() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["doctor", "--json", "--check"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["state_present"], false);
    assert_eq!(json["state_valid"], true);
    assert_eq!(json["resource_count"], 3);
}

#[test]
fn doctor_check_fails_for_invalid_state() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
    fs::write(dir.path().join(".agentsync/state.json"), "not json\n").unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["doctor", "--json", "--check"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["state_present"], true);
    assert_eq!(json["state_valid"], false);
    assert!(json["diagnostics"][0]["message"]
        .as_str()
        .unwrap()
        .contains("failed to load"));
}
