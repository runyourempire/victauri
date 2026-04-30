//! Thread-safe command registry with substring search and
//! natural-language-to-command resolution.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

/// Metadata for a registered Tauri command, including intent and schema information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandInfo {
    /// Fully qualified command name (e.g. "get_settings").
    pub name: String,
    /// Plugin namespace, if the command belongs to a Tauri plugin.
    pub plugin: Option<String>,
    /// Human-readable description of what the command does.
    pub description: Option<String>,
    /// Ordered list of arguments the command accepts.
    pub args: Vec<CommandArg>,
    /// Rust return type as a string (e.g. "Result<Settings, Error>").
    pub return_type: Option<String>,
    /// Whether the command handler is async.
    pub is_async: bool,
    /// Natural-language intent phrase for NL-to-command resolution.
    pub intent: Option<String>,
    /// Grouping category (e.g. "settings", "counter").
    pub category: Option<String>,
    /// Example natural-language queries that should resolve to this command.
    pub examples: Vec<String>,
}

/// Schema for a single argument of a registered command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandArg {
    /// Argument name as declared in the Rust function signature.
    pub name: String,
    /// Rust type name (e.g. "String", "`Option<u32>`").
    pub type_name: String,
    /// Whether the argument must be provided (not `Option`).
    pub required: bool,
    /// Optional JSON Schema for the argument's expected shape.
    pub schema: Option<serde_json::Value>,
}

/// Thread-safe registry of known Tauri commands, indexed by name.
#[derive(Debug, Clone)]
pub struct CommandRegistry {
    commands: Arc<RwLock<BTreeMap<String, CommandInfo>>>,
}

impl CommandRegistry {
    /// Creates an empty command registry.
    ///
    /// ```
    /// use victauri_core::CommandRegistry;
    ///
    /// let registry = CommandRegistry::new();
    /// assert_eq!(registry.count(), 0);
    /// assert!(registry.list().is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            commands: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Registers a command, replacing any existing entry with the same name.
    ///
    /// ```
    /// use victauri_core::{CommandRegistry, CommandInfo};
    ///
    /// let registry = CommandRegistry::new();
    /// registry.register(CommandInfo {
    ///     name: "greet".to_string(),
    ///     plugin: None,
    ///     description: Some("Say hello".to_string()),
    ///     args: vec![],
    ///     return_type: None,
    ///     is_async: false,
    ///     intent: None,
    ///     category: None,
    ///     examples: vec![],
    /// });
    /// assert_eq!(registry.count(), 1);
    /// assert!(registry.get("greet").is_some());
    /// ```
    pub fn register(&self, info: CommandInfo) {
        self.commands
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(info.name.clone(), info);
    }

    /// Looks up a command by exact name.
    pub fn get(&self, name: &str) -> Option<CommandInfo> {
        self.commands
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
            .cloned()
    }

    /// Returns all registered commands in alphabetical order.
    pub fn list(&self) -> Vec<CommandInfo> {
        self.commands
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect()
    }

    /// Returns the number of registered commands.
    pub fn count(&self) -> usize {
        self.commands
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// Searches commands by substring match on name or description (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<CommandInfo> {
        let query_lower = query.to_lowercase();
        self.commands
            .read()
            .unwrap_or_else(|e| e.into_inner())
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

    /// Resolves a natural-language query to commands ranked by relevance score.
    pub fn resolve(&self, query: &str) -> Vec<ScoredCommand> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        if query_words.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<ScoredCommand> = self
            .commands
            .read()
            .unwrap_or_else(|e| e.into_inner())
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

        scored.sort_by(|a, b| b.score.total_cmp(&a.score));
        scored
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A command paired with its relevance score from natural-language resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredCommand {
    /// The matched command metadata.
    pub command: CommandInfo,
    /// Relevance score (higher is better); 0 means no match.
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
