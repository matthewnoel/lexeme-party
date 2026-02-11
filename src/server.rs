use crate::protocol::{ClientMessage, PlayerState, ServerMessage};
use crate::words;
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

const INDEX_HTML: &str = include_str!("../static/index.html");

#[derive(Clone)]
struct PlayerConnection {
    name: String,
    score: u32,
    typed: String,
    tx: mpsc::UnboundedSender<ServerMessage>,
}

struct GameState {
    next_player_id: u64,
    round: u32,
    current_word: String,
    winner_last_round: Option<String>,
    players: HashMap<u64, PlayerConnection>,
}

fn snapshot_message(state: &GameState) -> ServerMessage {
    let mut players: Vec<PlayerState> = state
        .players
        .iter()
        .map(|(id, p)| PlayerState {
            id: *id,
            name: p.name.clone(),
            score: p.score,
            typed: p.typed.clone(),
        })
        .collect();
    players.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.id.cmp(&b.id)));

    ServerMessage::State {
        round: state.round,
        current_word: state.current_word.clone(),
        players,
        winner_last_round: state.winner_last_round.clone(),
    }
}

fn broadcast_state(state: &mut GameState) {
    let msg = snapshot_message(state);
    state.players.retain(|_, p| p.tx.send(msg.clone()).is_ok());
}

pub async fn run_server(bind_addr: String) -> anyhow::Result<()> {
    let listener = TcpListener::bind(&bind_addr).await?;
    log::info!("server listening on {}", bind_addr);
    log::info!("open http://{} in your browser to play", bind_addr);

    let shared = Arc::new(Mutex::new(GameState {
        next_player_id: 1,
        round: 1,
        current_word: words::choose_word(None),
        winner_last_round: None,
        players: HashMap::new(),
    }));

    loop {
        let (stream, addr) = listener.accept().await?;
        let shared_clone = Arc::clone(&shared);
        tokio::spawn(async move {
            if let Err(err) = handle_tcp_connection(stream, shared_clone).await {
                log::warn!("connection {} ended: {}", addr, err);
            }
        });
    }
}

/// Peek at an incoming TCP connection to determine whether it is a WebSocket
/// upgrade or a plain HTTP request, then route accordingly.
async fn handle_tcp_connection(
    stream: TcpStream,
    shared: Arc<Mutex<GameState>>,
) -> anyhow::Result<()> {
    let mut peek_buf = [0u8; 8192];
    let n = stream.peek(&mut peek_buf).await?;
    if n == 0 {
        return Ok(());
    }

    let request_text = std::str::from_utf8(&peek_buf[..n]).unwrap_or("");
    let lower = request_text.to_ascii_lowercase();

    if lower.contains("upgrade: websocket") {
        handle_websocket(stream, shared).await
    } else {
        serve_http(stream).await
    }
}

/// Serve static HTTP responses (the web client page).
async fn serve_http(mut stream: TcpStream) -> anyhow::Result<()> {
    // Consume the request from the TCP buffer.
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;
    let request = std::str::from_utf8(&buf[..n]).unwrap_or("");

    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    log::info!("HTTP {} {}", "GET", path);

    let (status, content_type, body) = match path {
        "/" | "/index.html" => ("200 OK", "text/html; charset=utf-8", INDEX_HTML),
        _ => ("404 Not Found", "text/plain; charset=utf-8", "404 Not Found"),
    };

    let body_bytes = body.as_bytes();
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        content_type,
        body_bytes.len(),
    );

    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body_bytes).await?;
    stream.shutdown().await?;

    Ok(())
}

/// Handle a WebSocket connection (game session).
async fn handle_websocket(
    stream: TcpStream,
    shared: Arc<Mutex<GameState>>,
) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<ServerMessage>();

    let player_id = {
        let mut state = shared
            .lock()
            .map_err(|_| anyhow::anyhow!("game state mutex poisoned"))?;
        let id = state.next_player_id;
        state.next_player_id += 1;
        state.players.insert(
            id,
            PlayerConnection {
                name: format!("player-{}", id),
                score: 0,
                typed: String::new(),
                tx: out_tx.clone(),
            },
        );

        let _ = out_tx.send(ServerMessage::Welcome { player_id: id });
        let snapshot = snapshot_message(&state);
        let _ = out_tx.send(snapshot);
        broadcast_state(&mut state);
        id
    };

    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let encoded = match serde_json::to_string(&msg) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("failed encoding server message: {}", e);
                    continue;
                }
            };
            if ws_write.send(Message::Text(encoded)).await.is_err() {
                break;
            }
        }
    });

    while let Some(msg_result) = ws_read.next().await {
        let msg = msg_result?;
        if !msg.is_text() {
            continue;
        }
        let payload = msg.into_text()?;
        let client_msg: ClientMessage = match serde_json::from_str(&payload) {
            Ok(m) => m,
            Err(err) => {
                log::warn!("bad client message: {}", err);
                continue;
            }
        };

        let mut state = shared
            .lock()
            .map_err(|_| anyhow::anyhow!("game state mutex poisoned"))?;

        match client_msg {
            ClientMessage::Join { name } => {
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.name = name;
                }
                broadcast_state(&mut state);
            }
            ClientMessage::TypedProgress { typed } => {
                let sanitized: String = typed
                    .chars()
                    .filter(|c| c.is_ascii_alphabetic())
                    .map(|c| c.to_ascii_lowercase())
                    .take(state.current_word.chars().count())
                    .collect();
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.typed = sanitized;
                }
                broadcast_state(&mut state);
            }
            ClientMessage::SubmitWord { word } => {
                let current = state.current_word.clone();
                if word.trim().eq_ignore_ascii_case(&current) {
                    let winner_name = if let Some(player) = state.players.get_mut(&player_id) {
                        player.score = player.score.saturating_add(1);
                        player.name.clone()
                    } else {
                        continue;
                    };
                    state.round = state.round.saturating_add(1);
                    state.current_word = words::choose_word(Some(&current));
                    state.winner_last_round = Some(winner_name);
                    for player in state.players.values_mut() {
                        player.typed.clear();
                    }
                    broadcast_state(&mut state);
                }
            }
        }
    }

    {
        let mut state = shared
            .lock()
            .map_err(|_| anyhow::anyhow!("game state mutex poisoned"))?;
        state.players.remove(&player_id);
        broadcast_state(&mut state);
    }

    writer.abort();
    Ok(())
}
