use std::fs;

use assert_cmd::Command;
use serde_json::Value;
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
fn sync_skills_write_creates_text_assets() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".claude/skills/review/assets")).unwrap();
    fs::write(
        dir.path().join(".claude/skills/review/SKILL.md"),
        "---\nname: review\n---\nBody\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".claude/skills/review/assets/guide.md"),
        "asset body\n",
    )
    .unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "sync", "skills", "--from", "claude", "--to", "codex", "--write",
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(dir.path().join(".codex/skills/review/assets/guide.md")).unwrap(),
        "asset body\n"
    );
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

#[test]
fn diff_json_outputs_structured_plan() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "diff",
            "rules",
            "--from",
            "agents-md",
            "--to",
            "claude",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["actions"][0]["action"], "create");
    assert!(json["actions"][0]["diff"]
        .as_str()
        .unwrap()
        .contains("@@ -0,0 +1,1 @@"));
}

#[test]
fn diff_format_json_outputs_structured_plan() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "diff",
            "rules",
            "--from",
            "agents-md",
            "--to",
            "claude",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["actions"][0]["action"], "create");
}

#[test]
fn diff_uses_config_defaults_when_from_and_to_are_omitted() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
    fs::write(
        dir.path().join(".agentsync/config.toml"),
        r#"schema_version = 1

[defaults]
source = "agents-md"
targets = ["claude"]
"#,
    )
    .unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["diff", "rules", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["actions"][0]["path"], "CLAUDE.md");
    assert_eq!(json["actions"][0]["action"], "create");
}

#[test]
fn sync_uses_config_defaults_and_still_requires_write() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
    fs::write(
        dir.path().join(".agentsync/config.toml"),
        r#"schema_version = 1

[defaults]
source = "agents-md"
targets = ["claude"]
"#,
    )
    .unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["sync", "rules"])
        .assert()
        .success();

    assert!(!dir.path().join("CLAUDE.md").exists());
    assert!(!dir.path().join(".agentsync/state.json").exists());

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["sync", "rules", "--write"])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
        "repo rules\n"
    );
}

#[test]
fn diff_config_defaults_respect_disabled_rules_sync() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
    fs::write(
        dir.path().join(".agentsync/config.toml"),
        r#"schema_version = 1

[defaults]
source = "agents-md"
targets = ["claude"]

[sync]
rules = false
"#,
    )
    .unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["diff", "rules"])
        .assert()
        .failure();
}

#[test]
fn sync_config_defaults_respect_disabled_skills_sync_without_writing() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
    fs::create_dir_all(dir.path().join(".claude/skills/review")).unwrap();
    fs::write(
        dir.path().join(".claude/skills/review/SKILL.md"),
        "---\nname: review\n---\nBody\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".agentsync/config.toml"),
        r#"schema_version = 1

[defaults]
source = "claude"
targets = ["codex"]

[sync]
skills = false
"#,
    )
    .unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["sync", "skills", "--write"])
        .assert()
        .failure();

    assert!(!dir.path().join(".codex/skills/review/SKILL.md").exists());
    assert!(!dir.path().join(".agentsync/state.json").exists());
}

#[test]
fn diff_without_cli_args_or_config_defaults_fails() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["diff", "rules"])
        .assert()
        .failure();
}

#[test]
fn explicit_diff_args_do_not_require_valid_config() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
    fs::write(
        dir.path().join(".agentsync/config.toml"),
        "schema_version = 2\n",
    )
    .unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args(["diff", "rules", "--from", "agents-md", "--to", "claude"])
        .assert()
        .success();
}

#[test]
fn sync_json_without_write_outputs_json_and_does_not_write() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "sync",
            "rules",
            "--from",
            "agents-md",
            "--to",
            "claude",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["actions"][0]["action"], "create");
    assert!(!dir.path().join("CLAUDE.md").exists());
    assert!(!dir.path().join(".agentsync/state.json").exists());
}

#[test]
fn sync_format_json_outputs_json_and_does_not_write() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "sync",
            "rules",
            "--from",
            "agents-md",
            "--to",
            "claude",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["actions"][0]["action"], "create");
    assert!(!dir.path().join("CLAUDE.md").exists());
    assert!(!dir.path().join(".agentsync/state.json").exists());
}

#[test]
fn diff_subagent_returns_blocked_plan() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
    fs::write(
        dir.path().join(".claude/agents/reviewer.md"),
        "---\nname: reviewer\n---\nReview carefully.\n",
    )
    .unwrap();

    let output = Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "diff", "subagent", "--from", "claude", "--to", "codex", "--format", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["actions"][0]["action"], "block");
    assert_eq!(json["actions"][0]["resource_id"], "subagents:reviewer");
    assert!(json["diagnostics"][0]["message"]
        .as_str()
        .unwrap()
        .contains("subagent rendering is blocked"));
}

#[test]
fn sync_subagent_write_is_blocked_before_state_or_target_write() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
    fs::write(
        dir.path().join(".claude/agents/reviewer.md"),
        "---\nname: reviewer\n---\nReview carefully.\n",
    )
    .unwrap();

    Command::cargo_bin("agentsync")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "sync",
            "subagents",
            "--from",
            "claude",
            "--to",
            "codex",
            "--write",
        ])
        .assert()
        .failure();

    assert!(!dir.path().join(".codex/agents/reviewer.md").exists());
    assert!(!dir.path().join(".agentsync/state.json").exists());
}
