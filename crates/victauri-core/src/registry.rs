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
    pub intent: Option<String>,
    pub category: Option<String>,
    pub examples: Vec<String>,
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

    pub fn resolve(&self, query: &str) -> Vec<ScoredCommand> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        if query_words.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<ScoredCommand> = self
            .commands
            .read()
            .unwrap()
            .values()
            .filter_map(|cmd| {
                let score = score_command(cmd, &query_lower, &query_words);
                if score > 0.0 {
                    Some(ScoredCommand {
                        command: cmd.clone(),
                        score,
                    })
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredCommand {
    pub command: CommandInfo,
    pub score: f64,
}

fn score_command(cmd: &CommandInfo, query_lower: &str, query_words: &[&str]) -> f64 {
    let mut score = 0.0;
    let name_lower = cmd.name.to_lowercase();
    let name_words: Vec<&str> = name_lower.split('_').collect();

    if name_lower == query_lower.replace(' ', "_") {
        score += 10.0;
    }

    for word in query_words {
        if name_lower.contains(word) {
            score += 3.0;
        }
        if name_words.contains(word) {
            score += 2.0;
        }
    }

    if let Some(desc) = &cmd.description {
        let desc_lower = desc.to_lowercase();
        for word in query_words {
            if desc_lower.contains(word) {
                score += 1.5;
            }
        }
    }

    if let Some(intent) = &cmd.intent {
        let intent_lower = intent.to_lowercase();
        for word in query_words {
            if intent_lower.contains(word) {
                score += 2.5;
            }
        }
    }

    if let Some(category) = &cmd.category {
        let cat_lower = category.to_lowercase();
        for word in query_words {
            if cat_lower.contains(word) {
                score += 1.0;
            }
        }
    }

    for example in &cmd.examples {
        let ex_lower = example.to_lowercase();
        if ex_lower.contains(query_lower) {
            score += 4.0;
            break;
        }
        for word in query_words {
            if ex_lower.contains(word) {
                score += 0.5;
            }
        }
    }

    score
}
