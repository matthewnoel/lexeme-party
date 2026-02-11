mod client;
mod protocol;
mod server;
mod words;

use std::env;

fn usage() -> &'static str {
    "Usage:
  cargo run -- server [bind_addr]
  cargo run -- client [ws_url] [player_name]

Defaults:
  bind_addr   = 127.0.0.1:9002
  ws_url      = ws://127.0.0.1:9002
  player_name = player"
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut args = env::args().skip(1);
    let Some(mode) = args.next() else {
        println!("{}", usage());
        return Ok(());
    };

    match mode.as_str() {
        "server" => {
            let bind_addr = args.next().unwrap_or_else(|| "127.0.0.1:9002".to_string());
            server::run_server(bind_addr).await
        }
        "client" => {
            let ws_url = args
                .next()
                .unwrap_or_else(|| "ws://127.0.0.1:9002".to_string());
            let player_name = args.next().unwrap_or_else(|| "player".to_string());
            client::run_client(ws_url, player_name)
        }
        _ => {
            println!("{}", usage());
            Ok(())
        }
    }
}
