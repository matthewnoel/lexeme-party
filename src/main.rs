mod protocol;
mod server;
mod words;

use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().skip(1).collect();
    let bind_addr = args.first().map_or("127.0.0.1:9002", |s| s.as_str());

    println!("Open http://{bind_addr}/ to play");

    server::run_server(bind_addr.to_string()).await
}
