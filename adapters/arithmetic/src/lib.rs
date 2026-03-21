use core::GameAdapter;

#[derive(Debug, Clone, Default)]
pub struct ArithmeticAdapter;

impl GameAdapter for ArithmeticAdapter {
    fn game_key(&self) -> &'static str {
        "arithmetic"
    }

    fn next_prompt(&self, seed: u64) -> String {
        let left = (seed % 12 + 1) as i32;
        let right = ((seed / 3) % 12 + 1) as i32;
        format!("{left} + {right}")
    }

    fn is_correct(&self, prompt: &str, attempt: &str) -> bool {
        let expected = eval_sum_prompt(prompt);
        match attempt.trim().parse::<i32>() {
            Ok(value) => expected == Some(value),
            Err(_) => false,
        }
    }

    fn normalize_progress(&self, raw_input: &str) -> String {
        raw_input.trim().to_string()
    }

    fn score_for_prompt(&self, _prompt: &str) -> f32 {
        5.0
    }
}

fn eval_sum_prompt(prompt: &str) -> Option<i32> {
    let mut parts = prompt.split('+').map(str::trim);
    let left = parts.next()?.parse::<i32>().ok()?;
    let right = parts.next()?.parse::<i32>().ok()?;
    Some(left + right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_expected_sum() {
        let adapter = ArithmeticAdapter;
        assert!(adapter.is_correct("2 + 9", "11"));
        assert!(!adapter.is_correct("2 + 9", "12"));
    }
}
