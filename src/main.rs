mod protocol;
mod server;
mod words;

use qrcode::{QrCode, render::unicode};
use std::env;
use std::net::{SocketAddr, UdpSocket};

fn detect_lan_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    Some(addr.ip().to_string())
}

fn print_qr(url: &str) {
    if let Ok(code) = QrCode::new(url.as_bytes()) {
        let image = code.render::<unicode::Dense1x2>().quiet_zone(true).build();
        println!("\nScan this QR code on your phone:\n{image}\n{url}\n");
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().skip(1).collect();
    let bind_addr = args.first().map_or("0.0.0.0:9002", |s| s.as_str());

    if let Ok(parsed) = bind_addr.parse::<SocketAddr>() {
        if parsed.ip().is_unspecified() {
            println!(
                "Open http://localhost:{}/ to play on this machine",
                parsed.port()
            );
            if let Some(lan_ip) = detect_lan_ip() {
                let lan_url = format!("http://{}:{}/", lan_ip, parsed.port());
                println!(
                    "Open http://{}:{}/ from other devices on your local network",
                    lan_ip,
                    parsed.port()
                );
                print_qr(&lan_url);
            } else {
                println!(
                    "Open http://<your-lan-ip>:{}/ from other devices on your local network",
                    parsed.port()
                );
            }
        } else {
            println!("Open http://{bind_addr}/ to play");
            if !parsed.ip().is_loopback() {
                print_qr(&format!("http://{bind_addr}/"));
            }
        }
    } else {
        println!("Open http://{bind_addr}/ to play");
    }

    server::run_server(bind_addr.to_string()).await
}
