use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub plugin: Option<String>,
    pub description: Option<String>,
    pub args: Vec<CommandArg>,
    pub return_type: Option<String>,
    pub is_async: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandArg {
    pub name: String,
    pub type_name: String,
    pub required: bool,
    pub schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct CommandRegistry {
    commands: Arc<RwLock<BTreeMap<String, CommandInfo>>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    pub fn register(&self, info: CommandInfo) {
        self.commands
            .write()
            .unwrap()
            .insert(info.name.clone(), info);
    }

    pub fn get(&self, name: &str) -> Option<CommandInfo> {
        self.commands.read().unwrap().get(name).cloned()
    }

    pub fn list(&self) -> Vec<CommandInfo> {
        self.commands.read().unwrap().values().cloned().collect()
    }

    pub fn count(&self) -> usize {
        self.commands.read().unwrap().len()
    }

    pub fn search(&self, query: &str) -> Vec<CommandInfo> {
        let query_lower = query.to_lowercase();
        self.commands
            .read()
            .unwrap()
            .values()
            .filter(|cmd| {
                cmd.name.to_lowercase().contains(&query_lower)
                    || cmd
                        .description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
