use crate::protocol::{ClientMessage, PlayerState, ServerMessage};
use crate::words;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;
use tower_http::services::ServeDir;

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
    state
        .players
        .retain(|_, p| p.tx.send(msg.clone()).is_ok());
}

type SharedState = Arc<Mutex<GameState>>;

pub async fn run_server(bind_addr: String) -> anyhow::Result<()> {
    let shared: SharedState = Arc::new(Mutex::new(GameState {
        next_player_id: 1,
        round: 1,
        current_word: words::choose_word(None),
        winner_last_round: None,
        players: HashMap::new(),
    }));

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new("static"))
        .with_state(shared);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    log::info!("server listening on http://{}", bind_addr);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(shared): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, shared))
}

async fn handle_socket(socket: WebSocket, shared: SharedState) {
    if let Err(err) = handle_socket_inner(socket, shared).await {
        log::warn!("websocket connection ended: {}", err);
    }
}

async fn handle_socket_inner(socket: WebSocket, shared: SharedState) -> anyhow::Result<()> {
    let (mut ws_write, mut ws_read) = socket.split();
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
            if ws_write.send(Message::Text(encoded.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(msg_result) = ws_read.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => {
                log::warn!("websocket read error: {}", e);
                break;
            }
        };

        let payload = match msg {
            Message::Text(text) => text.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };

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
