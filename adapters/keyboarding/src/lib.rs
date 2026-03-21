use core::GameAdapter;

const WORDS: &[&str] = &[
    "function",
    "variable",
    "compiler",
    "iterator",
    "closure",
    "boolean",
    "borrow",
    "ownership",
    "trait",
    "module",
    "pattern",
    "syntax",
];

#[derive(Debug, Clone, Default)]
pub struct KeyboardingAdapter;

impl GameAdapter for KeyboardingAdapter {
    fn game_key(&self) -> &'static str {
        "keyboarding"
    }

    fn next_prompt(&self, seed: u64) -> String {
        let idx = (seed as usize) % WORDS.len();
        WORDS[idx].to_string()
    }

    fn is_correct(&self, prompt: &str, attempt: &str) -> bool {
        prompt == attempt.trim()
    }

    fn normalize_progress(&self, raw_input: &str) -> String {
        raw_input.to_string()
    }

    fn score_for_prompt(&self, prompt: &str) -> f32 {
        (prompt.len() as f32 / 3.0).max(4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_exact_word_match() {
        let adapter = KeyboardingAdapter;
        assert!(adapter.is_correct("rust", "rust"));
        assert!(!adapter.is_correct("rust", "Rust"));
    }
}
