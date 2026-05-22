use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn init_without_write_does_not_create_config() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();

    assert!(!dir.path().join(".agentsync/config.toml").exists());
    assert!(!dir.path().join(".agentsync").exists());
}

#[test]
fn init_write_creates_config() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["init", "--write"])
        .assert()
        .success();

    let config = fs::read_to_string(dir.path().join(".agentsync/config.toml")).unwrap();
    assert!(config.contains("schema_version = 1"));
    assert!(config.contains("source = \"agents-md\""));
    assert!(config.contains("targets = [\"claude\", \"cursor\", \"opencode\"]"));
    assert!(config.contains("rules = true"));
    assert!(config.contains("skills = true"));
}

#[test]
fn init_write_does_not_overwrite_existing_config() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
    fs::write(
        dir.path().join(".agentsync/config.toml"),
        "schema_version = 1\nexisting = true\n",
    )
    .unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["init", "--write"])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(dir.path().join(".agentsync/config.toml")).unwrap(),
        "schema_version = 1\nexisting = true\n"
    );
}

#[test]
fn init_json_outputs_structured_report_and_does_not_write() {
    let dir = tempdir().unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["init", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["path"], ".agentsync/config.toml");
    assert_eq!(json["action"], "create");
    assert!(json["contents"].as_str().unwrap().contains("[defaults]"));
    assert!(!dir.path().join(".agentsync/config.toml").exists());
}

#[test]
fn init_format_json_outputs_structured_report_and_does_not_write() {
    let dir = tempdir().unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["init", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["path"], ".agentsync/config.toml");
    assert_eq!(json["action"], "create");
    assert!(!dir.path().join(".agentsync/config.toml").exists());
}
