use std::collections::HashMap;
use std::sync::Arc;

pub trait GameAdapter: Send + Sync + 'static {
    fn game_key(&self) -> &'static str;
    fn next_prompt(&self, seed: u64) -> String;
    fn is_correct(&self, prompt: &str, attempt: &str) -> bool;
    fn normalize_progress(&self, raw_input: &str) -> String;
    fn score_for_prompt(&self, prompt: &str) -> f32;
}

pub type AdapterHandle = Arc<dyn GameAdapter>;
pub type AdapterRegistry = HashMap<String, AdapterHandle>;

pub fn build_adapter_registry(adapters: Vec<AdapterHandle>) -> Result<AdapterRegistry, String> {
    if adapters.is_empty() {
        return Err("at least one adapter must be registered".to_string());
    }

    let mut registry = HashMap::new();
    for adapter in adapters {
        let game_key = adapter.game_key().to_string();
        if registry.insert(game_key.clone(), adapter).is_some() {
            return Err(format!("duplicate adapter game key: {game_key}"));
        }
    }
    Ok(registry)
}
