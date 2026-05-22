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
