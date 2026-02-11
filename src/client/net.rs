use crate::protocol::{ClientMessage, ServerMessage};
use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use std::{sync::mpsc as std_mpsc, thread};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug)]
pub enum NetworkEvent {
    Server(ServerMessage),
    Disconnected(String),
}

pub fn spawn_network(
    ws_url: String,
    name: String,
) -> (
    mpsc::UnboundedSender<ClientMessage>,
    std_mpsc::Receiver<NetworkEvent>,
) {
    let (to_net_tx, to_net_rx) = mpsc::unbounded_channel::<ClientMessage>();
    let (to_ui_tx, to_ui_rx) = std_mpsc::channel::<NetworkEvent>();

    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                let _ = to_ui_tx.send(NetworkEvent::Disconnected(e.to_string()));
                return;
            }
        };

        let result = runtime.block_on(network_task(ws_url, name, to_net_rx, to_ui_tx.clone()));
        if let Err(err) = result {
            let _ = to_ui_tx.send(NetworkEvent::Disconnected(err.to_string()));
        }
    });

    (to_net_tx, to_ui_rx)
}

async fn network_task(
    ws_url: String,
    name: String,
    mut outbound_rx: mpsc::UnboundedReceiver<ClientMessage>,
    inbound_tx: std_mpsc::Sender<NetworkEvent>,
) -> anyhow::Result<()> {
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .with_context(|| format!("failed connecting to {}", ws_url))?;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    let join = serde_json::to_string(&ClientMessage::Join { name })?;
    ws_write.send(Message::Text(join)).await?;

    loop {
        tokio::select! {
            Some(outbound) = outbound_rx.recv() => {
                let payload = serde_json::to_string(&outbound)?;
                ws_write.send(Message::Text(payload)).await?;
            }
            incoming = ws_read.next() => {
                let Some(msg_result) = incoming else { break; };
                let msg = msg_result?;
                if !msg.is_text() {
                    continue;
                }
                let text = msg.into_text()?;
                let server_msg: ServerMessage = serde_json::from_str(&text)?;
                let _ = inbound_tx.send(NetworkEvent::Server(server_msg));
            }
        }
    }

    Ok(())
}
