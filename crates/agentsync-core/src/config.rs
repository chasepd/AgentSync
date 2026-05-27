use std::fs;
use std::path::{Path, PathBuf};

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};

use crate::diagnostics::AgentSyncError;
use crate::model::{Agent, Scope, SourceAlias};

pub const CONFIG_PATH: &str = ".agentsync/config.toml";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ConfigFile {
    pub schema_version: u32,
    #[serde(default)]
    pub defaults: ConfigDefaults,
    #[serde(default)]
    pub sync: ConfigSync,
}

impl ConfigFile {
    pub fn validate(&self) -> Result<(), AgentSyncError> {
        if self.schema_version != 1 {
            return Err(AgentSyncError::InvalidArgument(format!(
                "unsupported config schema_version {}; expected 1",
                self.schema_version
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct ConfigDefaults {
    #[serde(default, deserialize_with = "deserialize_optional_scope")]
    pub scope: Option<Scope>,
    #[serde(default, deserialize_with = "deserialize_optional_source")]
    pub source: Option<SourceAlias>,
    #[serde(default, deserialize_with = "deserialize_agents")]
    pub targets: Vec<Agent>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct ConfigSync {
    pub rules: Option<bool>,
    pub skills: Option<bool>,
    pub subagents: Option<bool>,
    pub commands: Option<bool>,
    pub hooks: Option<bool>,
}

pub fn config_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join(CONFIG_PATH)
}

pub fn load_config(root: impl AsRef<Path>) -> Result<Option<ConfigFile>, AgentSyncError> {
    let path = config_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let config = toml::from_str::<ConfigFile>(&raw)?;
    config.validate()?;
    Ok(Some(config))
}

fn parse_scope(value: &str) -> Result<Scope, String> {
    match value {
        "project" => Ok(Scope::Project),
        "user" => Ok(Scope::User),
        "all" => Ok(Scope::All),
        _ => Err(format!("unsupported scope `{value}`")),
    }
}

fn parse_source(value: &str) -> Result<SourceAlias, String> {
    match value {
        "all" => Ok(SourceAlias::All),
        "agents-md" => Ok(SourceAlias::AgentsMd),
        "codex" => Ok(SourceAlias::Codex),
        "claude" => Ok(SourceAlias::Claude),
        "cursor" | "cursor-cli" => Ok(SourceAlias::CursorCli),
        "opencode" => Ok(SourceAlias::OpenCode),
        _ => Err(format!("unsupported source `{value}`")),
    }
}

fn parse_agent(value: &str) -> Result<Agent, String> {
    match value {
        "codex" => Ok(Agent::Codex),
        "claude" => Ok(Agent::Claude),
        "cursor" | "cursor-cli" => Ok(Agent::CursorCli),
        "opencode" => Ok(Agent::OpenCode),
        _ => Err(format!("unsupported target `{value}`")),
    }
}

fn deserialize_optional_scope<'de, D>(deserializer: D) -> Result<Option<Scope>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .map(|value| parse_scope(&value).map_err(de::Error::custom))
        .transpose()
}

fn deserialize_optional_source<'de, D>(deserializer: D) -> Result<Option<SourceAlias>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .map(|value| parse_source(&value).map_err(de::Error::custom))
        .transpose()
}

fn deserialize_agents<'de, D>(deserializer: D) -> Result<Vec<Agent>, D::Error>
where
    D: Deserializer<'de>,
{
    struct AgentsVisitor;

    impl<'de> Visitor<'de> for AgentsVisitor {
        type Value = Vec<Agent>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a list of target agent names or all")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut agents = Vec::new();
            while let Some(value) = seq.next_element::<String>()? {
                if value == "all" {
                    for agent in Agent::ALL {
                        if !agents.contains(&agent) {
                            agents.push(agent);
                        }
                    }
                } else {
                    let agent = parse_agent(&value).map_err(de::Error::custom)?;
                    if !agents.contains(&agent) {
                        agents.push(agent);
                    }
                }
            }
            Ok(agents)
        }
    }

    deserializer.deserialize_seq(AgentsVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn config_accepts_cli_agent_names() {
        let config = toml::from_str::<ConfigFile>(
            r#"schema_version = 1

[defaults]
scope = "project"
source = "agents-md"
targets = ["claude", "cursor-cli", "opencode"]
"#,
        )
        .unwrap();

        assert_eq!(config.defaults.scope, Some(Scope::Project));
        assert_eq!(config.defaults.source, Some(SourceAlias::AgentsMd));
        assert_eq!(
            config.defaults.targets,
            vec![Agent::Claude, Agent::CursorCli, Agent::OpenCode]
        );
    }

    #[test]
    fn config_accepts_all_source_and_targets() {
        let config = toml::from_str::<ConfigFile>(
            r#"schema_version = 1

[defaults]
source = "all"
targets = ["all"]
"#,
        )
        .unwrap();

        assert_eq!(config.defaults.source, Some(SourceAlias::All));
        assert_eq!(config.defaults.targets, Agent::ALL.to_vec());
    }

    #[test]
    fn config_accepts_sync_toggles_for_all_selectable_resources() {
        let config = toml::from_str::<ConfigFile>(
            r#"schema_version = 1

[sync]
rules = true
skills = true
subagents = false
commands = false
hooks = false
"#,
        )
        .unwrap();

        assert_eq!(config.sync.rules, Some(true));
        assert_eq!(config.sync.skills, Some(true));
        assert_eq!(config.sync.subagents, Some(false));
        assert_eq!(config.sync.commands, Some(false));
        assert_eq!(config.sync.hooks, Some(false));
    }

    #[test]
    fn missing_config_loads_as_none() {
        let dir = tempdir().unwrap();

        assert_eq!(load_config(dir.path()).unwrap(), None);
    }

    #[test]
    fn unsupported_schema_version_fails() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
        fs::write(
            dir.path().join(CONFIG_PATH),
            "schema_version = 2\n[defaults]\nsource = \"agents-md\"\n",
        )
        .unwrap();

        assert!(load_config(dir.path())
            .unwrap_err()
            .to_string()
            .contains("unsupported config schema_version"));
    }
}
