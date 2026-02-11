use rand::seq::SliceRandom;

const WORD_BANK: &[&str] = &[
    "apple", "bridge", "candle", "dragon", "ember", "forest", "galaxy", "harbor", "island",
    "jungle", "kitten", "lantern", "meteor", "nebula", "orange", "planet", "quartz", "rocket",
    "sunrise", "thunder", "violet", "whisper", "xylophone", "yonder", "zephyr",
];

/// Choose a random word from the bank, guaranteeing it differs from `current`.
/// If the bank has only one word (or is empty), the same word may be returned.
pub fn choose_word(current: Option<&str>) -> String {
    let mut rng = rand::thread_rng();

    match current {
        Some(cur) if WORD_BANK.len() > 1 => {
            loop {
                let pick = WORD_BANK
                    .choose(&mut rng)
                    .copied()
                    .unwrap_or("apple");
                if pick != cur {
                    return pick.to_string();
                }
            }
        }
        _ => WORD_BANK
            .choose(&mut rng)
            .copied()
            .unwrap_or("apple")
            .to_string(),
    }
}
