use core::{ServerConfig, run_server};
use edif_io_arithmetic_adapter::ArithmeticAdapter;
use edif_io_keyboarding_adapter::KeyboardingAdapter;
use std::sync::Arc;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:4000".to_string());
    let growth_per_round_win = std::env::var("GROWTH_PER_ROUND_WIN")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(4.0);

    let config = ServerConfig {
        bind_addr,
        growth_per_round_win,
    };
    run_server(
        vec![Arc::new(KeyboardingAdapter), Arc::new(ArithmeticAdapter)],
        config,
    )
    .await
    .map_err(std::io::Error::other)
}
