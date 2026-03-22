use crate::adapter::{AdapterHandle, AdapterRegistry, build_adapter_registry};
use crate::game::{
    DEFAULT_START_SIZE, PlayerId, PlayerState, RoomState, apply_round_win,
    apply_wrong_answer_penalty, resolve_match_by_timer,
};
use crate::powerup::{
    ActivePowerUp, DISTRIBUTION_INTERVAL_SECS, PowerUpOffer, cleanup_expired, effect_duration,
    has_double_points, is_player_frozen, offer_duration, pick_powerup_kind, pick_powerup_recipient,
};
use crate::protocol::{ClientMessage, ServerMessage};
use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use rand::distr::Alphanumeric;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub growth_per_round_win: f32,
    pub shrink_per_wrong_answer: f32,
    pub match_duration_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:4000".to_string(),
            growth_per_round_win: 4.0,
            shrink_per_wrong_answer: 2.0,
            match_duration_secs: 60,
        }
    }
}

#[derive(Debug)]
struct RoomConnection {
    sender: mpsc::UnboundedSender<Message>,
}

struct SharedState {
    adapters: AdapterRegistry,
    default_game_key: String,
    config: ServerConfig,
    rooms: Mutex<HashMap<String, RoomState>>,
    connections: Mutex<HashMap<String, HashMap<PlayerId, RoomConnection>>>,
    rejoin_tokens: Mutex<HashMap<String, (String, PlayerId)>>,
    prompt_seed: AtomicU64,
}

pub async fn run_server(adapters: Vec<AdapterHandle>, config: ServerConfig) -> Result<(), String> {
    let default_game_key = adapters
        .first()
        .map(|adapter| adapter.game_key().to_string())
        .ok_or_else(|| "at least one adapter must be registered".to_string())?;
    let adapters = build_adapter_registry(adapters)?;
    let state = Arc::new(SharedState {
        adapters,
        default_game_key,
        config: config.clone(),
        rooms: Mutex::new(HashMap::new()),
        connections: Mutex::new(HashMap::new()),
        rejoin_tokens: Mutex::new(HashMap::new()),
        prompt_seed: AtomicU64::new(1),
    });

    let app = Router::new()
        .route("/healthz", get(health_handler))
        .route("/readyz", get(health_handler))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let listener = TcpListener::bind(&config.bind_addr)
        .await
        .map_err(|e| format!("failed to bind {}: {e}", config.bind_addr))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("server error: {e}"))
}

async fn health_handler() -> impl IntoResponse {
    "ok"
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<SharedState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<SharedState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (client_tx, mut client_rx) = mpsc::unbounded_channel::<Message>();

    let writer_task = tokio::spawn(async move {
        while let Some(msg) = client_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut player_id: Option<PlayerId> = None;
    let mut room_code: Option<String> = None;

    while let Some(Ok(msg)) = ws_rx.next().await {
        let Message::Text(raw_text) = msg else {
            continue;
        };

        let incoming = match serde_json::from_str::<ClientMessage>(&raw_text) {
            Ok(parsed) => parsed,
            Err(_) => {
                let _ = send_server_message(
                    &client_tx,
                    &ServerMessage::Error {
                        message: "Invalid message format".to_string(),
                    },
                );
                continue;
            }
        };

        match incoming {
            ClientMessage::JoinOrCreateRoom {
                player_name,
                room_code: requested_room_code,
                game_mode,
                match_duration_secs,
            } => {
                if player_id.is_some() {
                    continue;
                }

                let result = join_or_create_room(
                    &state,
                    player_name,
                    requested_room_code,
                    game_mode,
                    match_duration_secs,
                    client_tx.clone(),
                )
                .await;

                if let Some((code, token, assigned_player_id)) = result {
                    player_id = Some(assigned_player_id);
                    room_code = Some(code.clone());

                    {
                        let mut tokens = state.rejoin_tokens.lock().await;
                        tokens.insert(token.clone(), (code.clone(), assigned_player_id));
                    }

                    let adapter = adapter_for_room(&state, &code).await;
                    let _ = send_server_message(
                        &client_tx,
                        &ServerMessage::Welcome {
                            player_id: assigned_player_id,
                            room_code: code.clone(),
                            game_key: room_game_key(&state, &code)
                                .await
                                .unwrap_or_else(|| state.default_game_key.clone()),
                            input_placeholder: adapter
                                .map(|a| a.input_placeholder().to_string())
                                .unwrap_or_default(),
                            rejoin_token: token,
                        },
                    );

                    let _ = broadcast_room_state(&state, &code).await;
                } else {
                    let _ = send_server_message(
                        &client_tx,
                        &ServerMessage::Error {
                            message: "Unable to join room".to_string(),
                        },
                    );
                }
            }
            ClientMessage::RejoinRoom { rejoin_token } => {
                if player_id.is_some() {
                    continue;
                }

                let lookup = {
                    let tokens = state.rejoin_tokens.lock().await;
                    tokens.get(&rejoin_token).cloned()
                };

                let Some((found_code, found_pid)) = lookup else {
                    let _ = send_server_message(
                        &client_tx,
                        &ServerMessage::Error {
                            message: "Invalid rejoin token".to_string(),
                        },
                    );
                    continue;
                };

                let prompt_snapshot = {
                    let mut rooms = state.rooms.lock().await;
                    let Some(room) = rooms.get_mut(&found_code) else {
                        let mut tokens = state.rejoin_tokens.lock().await;
                        tokens.remove(&rejoin_token);
                        let _ = send_server_message(
                            &client_tx,
                            &ServerMessage::Error {
                                message: "Room no longer exists".to_string(),
                            },
                        );
                        continue;
                    };
                    let Some(player) = room.players.get_mut(&found_pid) else {
                        let mut tokens = state.rejoin_tokens.lock().await;
                        tokens.remove(&rejoin_token);
                        let _ = send_server_message(
                            &client_tx,
                            &ServerMessage::Error {
                                message: "Player no longer in room".to_string(),
                            },
                        );
                        continue;
                    };
                    player.connected = true;

                    if room.prompt.is_empty() {
                        None
                    } else {
                        Some((room.round_id, room.prompt.clone()))
                    }
                };

                {
                    let mut connections = state.connections.lock().await;
                    connections.entry(found_code.clone()).or_default().insert(
                        found_pid,
                        RoomConnection {
                            sender: client_tx.clone(),
                        },
                    );
                }

                player_id = Some(found_pid);
                room_code = Some(found_code.clone());

                let adapter = adapter_for_room(&state, &found_code).await;
                let _ = send_server_message(
                    &client_tx,
                    &ServerMessage::Welcome {
                        player_id: found_pid,
                        room_code: found_code.clone(),
                        game_key: room_game_key(&state, &found_code)
                            .await
                            .unwrap_or_else(|| state.default_game_key.clone()),
                        input_placeholder: adapter
                            .map(|a| a.input_placeholder().to_string())
                            .unwrap_or_default(),
                        rejoin_token,
                    },
                );

                let _ = broadcast_room_state(&state, &found_code).await;

                if let Some((round_id, prompt)) = prompt_snapshot {
                    let _ = send_server_message(
                        &client_tx,
                        &ServerMessage::PromptState {
                            room_code: found_code,
                            round_id,
                            prompt,
                        },
                    );
                }
            }
            ClientMessage::InputUpdate { text } => {
                if let (Some(pid), Some(code)) = (player_id, room_code.as_ref()) {
                    handle_progress_update(&state, code, pid, text).await;
                }
            }
            ClientMessage::SubmitAttempt { text } => {
                if let (Some(pid), Some(code)) = (player_id, room_code.as_ref()) {
                    handle_submission(&state, code, pid, text).await;
                }
            }
            ClientMessage::StartMatch => {
                if let (Some(pid), Some(code)) = (player_id, room_code.as_ref()) {
                    handle_start_match(&state, code, pid).await;
                }
            }
            ClientMessage::Rematch => {
                if let (Some(_pid), Some(code)) = (player_id, room_code.as_ref()) {
                    handle_rematch(&state, code).await;
                }
            }
        }
    }

    if let (Some(pid), Some(code)) = (player_id, room_code) {
        disconnect_player(&state, &code, pid).await;
    }

    writer_task.abort();
}

async fn join_or_create_room(
    state: &Arc<SharedState>,
    player_name: Option<String>,
    requested_room_code: Option<String>,
    requested_game_mode: Option<String>,
    requested_match_duration_secs: Option<u64>,
    sender: mpsc::UnboundedSender<Message>,
) -> Option<(String, String, PlayerId)> {
    let token = generate_rejoin_token();
    let mut rooms = state.rooms.lock().await;
    let mut connections = state.connections.lock().await;

    let room_code = match requested_room_code {
        Some(code) if rooms.contains_key(&code) => code,
        Some(_) => return None,
        None => {
            let requested = requested_game_mode
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let room_game_key = match requested {
                Some(game_key) => {
                    if state.adapters.contains_key(game_key) {
                        game_key.to_string()
                    } else {
                        return None;
                    }
                }
                None => state.default_game_key.clone(),
            };
            let generated = generate_room_code(&rooms);
            let duration = requested_match_duration_secs
                .filter(|&s| s > 0)
                .unwrap_or(state.config.match_duration_secs);
            rooms.insert(
                generated.clone(),
                RoomState {
                    room_code: generated.clone(),
                    game_key: room_game_key,
                    players: HashMap::new(),
                    prompt: String::new(),
                    round_id: 0,
                    match_winner: None,
                    match_deadline: None,
                    match_duration_secs: duration,
                    host_player_id: 1,
                    next_player_id: 1,
                    powerup_offers: Vec::new(),
                    active_powerups: Vec::new(),
                },
            );
            generated
        }
    };

    let room = rooms.get_mut(&room_code)?;

    let player_id = room.next_player_id;
    room.next_player_id += 1;

    room.players.insert(
        player_id,
        PlayerState {
            id: player_id,
            name: player_name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| format!("Player-{player_id}")),
            size: DEFAULT_START_SIZE,
            color: generate_color(player_id),
            connected: true,
            progress: String::new(),
            rejoin_token: token.clone(),
        },
    );

    connections
        .entry(room_code.clone())
        .or_default()
        .insert(player_id, RoomConnection { sender });

    Some((room_code, token, player_id))
}

async fn handle_progress_update(
    state: &Arc<SharedState>,
    room_code: &str,
    player_id: PlayerId,
    text: String,
) {
    let Some(adapter) = adapter_for_room(state, room_code).await else {
        return;
    };
    let normalized = adapter.normalize_progress(&text);

    {
        let mut rooms = state.rooms.lock().await;
        let Some(room) = rooms.get_mut(room_code) else {
            return;
        };
        let Some(player) = room.players.get_mut(&player_id) else {
            return;
        };
        player.progress = normalized.clone();
    }

    let _ = broadcast_to_room(
        state,
        room_code,
        &ServerMessage::RaceProgress {
            room_code: room_code.to_string(),
            player_id,
            text: normalized,
        },
    )
    .await;
}

async fn handle_submission(
    state: &Arc<SharedState>,
    room_code: &str,
    player_id: PlayerId,
    text: String,
) {
    let Some(adapter) = adapter_for_room(state, room_code).await else {
        return;
    };
    let mut should_advance_round = false;
    let mut round_result: Option<ServerMessage> = None;
    let mut wrong_answer_msg: Option<ServerMessage> = None;
    let mut earned_powerups: Vec<ServerMessage> = Vec::new();

    {
        let mut rooms = state.rooms.lock().await;
        let Some(room) = rooms.get_mut(room_code) else {
            return;
        };

        if room.match_winner.is_some() || room.prompt.is_empty() {
            return;
        }

        if is_player_frozen(&room.active_powerups, player_id) {
            return;
        }

        if !adapter.is_correct(&room.prompt, &text) {
            let penalty = state.config.shrink_per_wrong_answer;
            if let Some(shrink) = apply_wrong_answer_penalty(room, player_id, penalty) {
                wrong_answer_msg = Some(ServerMessage::WrongAnswer {
                    room_code: room_code.to_string(),
                    player_id,
                    shrink_applied: shrink,
                });
            }
        } else {
            let configured_growth = state.config.growth_per_round_win;
            let mut growth = adapter
                .score_for_prompt(&room.prompt)
                .max(configured_growth);

            if has_double_points(&room.active_powerups, player_id) {
                growth *= 2.0;
            }

            if let Some(resolution) = apply_round_win(room, player_id, growth) {
                round_result = Some(ServerMessage::RoundResult {
                    room_code: room_code.to_string(),
                    round_id: room.round_id,
                    winner_player_id: resolution.round_winner,
                    growth_awarded: growth,
                });
                should_advance_round = room.match_winner.is_none();

                let now = Instant::now();
                let mut i = 0;
                while i < room.powerup_offers.len() {
                    if room.powerup_offers[i].player_id == player_id
                        && room.powerup_offers[i].expires_at > now
                    {
                        let offer = room.powerup_offers.swap_remove(i);
                        let duration = effect_duration(offer.kind);
                        room.active_powerups.push(ActivePowerUp {
                            kind: offer.kind,
                            source_player_id: player_id,
                            expires_at: now + duration,
                        });
                        earned_powerups.push(ServerMessage::PowerUpActivated {
                            player_id,
                            kind: offer.kind,
                            duration_ms: duration.as_millis() as u64,
                        });
                    } else {
                        i += 1;
                    }
                }
            }
        }
    }

    if let Some(msg) = wrong_answer_msg {
        let _ = broadcast_to_room(state, room_code, &msg).await;
        let _ = broadcast_room_state(state, room_code).await;
        return;
    }

    let had_round_result = round_result.is_some();
    if let Some(msg) = round_result {
        let _ = broadcast_to_room(state, room_code, &msg).await;
    }

    for msg in &earned_powerups {
        let _ = broadcast_to_room(state, room_code, msg).await;
    }

    if had_round_result || !earned_powerups.is_empty() {
        let _ = broadcast_room_state(state, room_code).await;
    }

    if should_advance_round {
        let _ = ensure_prompt_for_room(state, room_code).await;
    }
}

async fn handle_start_match(state: &Arc<SharedState>, room_code: &str, player_id: PlayerId) {
    let duration_secs;
    {
        let mut rooms = state.rooms.lock().await;
        let Some(room) = rooms.get_mut(room_code) else {
            return;
        };
        if player_id != room.host_player_id || room.match_deadline.is_some() {
            return;
        }
        duration_secs = room.match_duration_secs;
        let deadline = Instant::now() + Duration::from_secs(duration_secs);
        room.match_deadline = Some(deadline);
    }

    start_match_timer(state.clone(), room_code.to_string(), duration_secs);
    start_powerup_timer(state.clone(), room_code.to_string(), duration_secs);

    let _ = broadcast_room_state(state, room_code).await;
    let _ = ensure_prompt_for_room(state, room_code).await;
}

async fn handle_rematch(state: &Arc<SharedState>, room_code: &str) {
    let duration_secs;
    {
        let mut rooms = state.rooms.lock().await;
        let Some(room) = rooms.get_mut(room_code) else {
            return;
        };
        if room.match_winner.is_none() {
            return;
        }
        room.reset_for_rematch();
        duration_secs = room.match_duration_secs;
        let deadline = Instant::now() + Duration::from_secs(duration_secs);
        room.match_deadline = Some(deadline);
    }

    start_match_timer(state.clone(), room_code.to_string(), duration_secs);
    start_powerup_timer(state.clone(), room_code.to_string(), duration_secs);

    let _ = broadcast_room_state(state, room_code).await;
    let _ = ensure_prompt_for_room(state, room_code).await;
}

async fn ensure_prompt_for_room(state: &Arc<SharedState>, room_code: &str) -> bool {
    let Some(adapter) = adapter_for_room(state, room_code).await else {
        return false;
    };
    let prompt_update;
    {
        let mut rooms = state.rooms.lock().await;
        let Some(room) = rooms.get_mut(room_code) else {
            return false;
        };
        if room.match_winner.is_some() || room.players.is_empty() || room.match_deadline.is_none() {
            return false;
        }
        let seed = state.prompt_seed.fetch_add(1, Ordering::Relaxed);
        room.round_id += 1;
        room.prompt = adapter.next_prompt(seed);
        for player in room.players.values_mut() {
            player.progress.clear();
        }
        prompt_update = (room.round_id, room.prompt.clone());
    }

    let (round_id, prompt) = prompt_update;
    let _ = broadcast_to_room(
        state,
        room_code,
        &ServerMessage::PromptState {
            room_code: room_code.to_string(),
            round_id,
            prompt,
        },
    )
    .await;

    true
}

fn start_match_timer(state: Arc<SharedState>, room_code: String, duration_secs: u64) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(duration_secs)).await;
        {
            let mut rooms = state.rooms.lock().await;
            let Some(room) = rooms.get_mut(&room_code) else {
                return;
            };
            resolve_match_by_timer(room);
        }
        let _ = broadcast_room_state(&state, &room_code).await;
    });
}

fn start_powerup_timer(state: Arc<SharedState>, room_code: String, match_duration_secs: u64) {
    tokio::spawn(async move {
        let match_end = Instant::now() + Duration::from_secs(match_duration_secs);
        loop {
            tokio::time::sleep(Duration::from_secs(DISTRIBUTION_INTERVAL_SECS)).await;

            let now = Instant::now();
            if now >= match_end {
                break;
            }

            let mut expired_offer_notifs: Vec<(PlayerId, ServerMessage)> = Vec::new();
            let mut expired_effect_broadcasts: Vec<ServerMessage> = Vec::new();
            let mut new_offer_notif: Option<(PlayerId, ServerMessage)> = None;

            {
                let mut rooms = state.rooms.lock().await;
                let Some(room) = rooms.get_mut(&room_code) else {
                    break;
                };
                if room.match_winner.is_some() {
                    break;
                }

                let expired =
                    cleanup_expired(&mut room.powerup_offers, &mut room.active_powerups, now);

                for offer in &expired.expired_offers {
                    expired_offer_notifs.push((
                        offer.player_id,
                        ServerMessage::PowerUpOfferExpired { kind: offer.kind },
                    ));
                }
                for effect in &expired.expired_effects {
                    expired_effect_broadcasts.push(ServerMessage::PowerUpEffectEnded {
                        player_id: effect.source_player_id,
                        kind: effect.kind,
                    });
                }

                let players: Vec<(PlayerId, f32)> = room
                    .players
                    .values()
                    .filter(|p| p.connected)
                    .map(|p| (p.id, p.size))
                    .collect();

                let mut rng = rand::rng();
                if let Some(recipient) = pick_powerup_recipient(&players, &mut rng) {
                    let kind = pick_powerup_kind(&mut rng);
                    let expires_at = now + offer_duration();
                    room.powerup_offers.push(PowerUpOffer {
                        kind,
                        player_id: recipient,
                        expires_at,
                    });
                    let expires_in_ms = offer_duration().as_millis() as u64;
                    new_offer_notif = Some((
                        recipient,
                        ServerMessage::PowerUpOffered {
                            kind,
                            expires_in_ms,
                        },
                    ));
                }
            }

            for (player_id, msg) in expired_offer_notifs {
                let _ = send_to_player(&state, &room_code, player_id, &msg).await;
            }
            let had_expired_effects = !expired_effect_broadcasts.is_empty();
            for msg in expired_effect_broadcasts {
                let _ = broadcast_to_room(&state, &room_code, &msg).await;
            }
            if had_expired_effects {
                let _ = broadcast_room_state(&state, &room_code).await;
            }
            if let Some((player_id, msg)) = new_offer_notif {
                let _ = send_to_player(&state, &room_code, player_id, &msg).await;
            }
        }
    });
}

async fn send_to_player(
    state: &Arc<SharedState>,
    room_code: &str,
    player_id: PlayerId,
    message: &ServerMessage,
) -> bool {
    let connections = state.connections.lock().await;
    let Some(room_connections) = connections.get(room_code) else {
        return false;
    };
    let Some(conn) = room_connections.get(&player_id) else {
        return false;
    };
    send_server_message(&conn.sender, message).is_ok()
}

async fn disconnect_player(state: &Arc<SharedState>, room_code: &str, player_id: PlayerId) {
    {
        let mut connections = state.connections.lock().await;
        if let Some(room_connections) = connections.get_mut(room_code) {
            room_connections.remove(&player_id);
            if room_connections.is_empty() {
                connections.remove(room_code);
            }
        }
    }

    let all_disconnected;
    {
        let mut rooms = state.rooms.lock().await;
        if let Some(room) = rooms.get_mut(room_code) {
            if let Some(player) = room.players.get_mut(&player_id) {
                player.connected = false;
            }
            all_disconnected = room.players.values().all(|p| !p.connected);
            if all_disconnected {
                rooms.remove(room_code);
            }
        } else {
            all_disconnected = true;
        }
    }

    if all_disconnected {
        let mut tokens = state.rejoin_tokens.lock().await;
        tokens.retain(|_, (rc, _)| rc != room_code);
    } else {
        let _ = broadcast_room_state(state, room_code).await;
    }
}

async fn broadcast_room_state(state: &Arc<SharedState>, room_code: &str) -> bool {
    let snapshot = {
        let rooms = state.rooms.lock().await;
        let Some(room) = rooms.get(room_code) else {
            return false;
        };
        room.to_snapshot()
    };

    broadcast_to_room(
        state,
        room_code,
        &ServerMessage::RoomState { room: snapshot },
    )
    .await
}

async fn broadcast_to_room(
    state: &Arc<SharedState>,
    room_code: &str,
    message: &ServerMessage,
) -> bool {
    let connections = state.connections.lock().await;
    let Some(room_connections) = connections.get(room_code) else {
        return false;
    };

    room_connections
        .values()
        .for_each(|conn| drop(send_server_message(&conn.sender, message)));
    true
}

fn send_server_message<T: Serialize>(
    sender: &mpsc::UnboundedSender<Message>,
    message: &T,
) -> Result<(), String> {
    let encoded = serde_json::to_string(message).map_err(|e| format!("encode error: {e}"))?;
    sender
        .send(Message::Text(encoded.into()))
        .map_err(|e| format!("send error: {e}"))
}

async fn room_game_key(state: &Arc<SharedState>, room_code: &str) -> Option<String> {
    let rooms = state.rooms.lock().await;
    rooms.get(room_code).map(|room| room.game_key.clone())
}

async fn adapter_for_room(state: &Arc<SharedState>, room_code: &str) -> Option<AdapterHandle> {
    let game_key = room_game_key(state, room_code).await?;
    state.adapters.get(&game_key).cloned()
}

fn generate_rejoin_token() -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn generate_room_code(rooms: &HashMap<String, RoomState>) -> String {
    let mut rng = rand::rng();
    loop {
        let code = (0..4)
            .map(|_| (b'A' + rng.random_range(0..26)) as char)
            .collect::<String>();
        if !rooms.contains_key(&code) {
            return code;
        }
    }
}

fn generate_color(player_id: PlayerId) -> String {
    let palette = [
        "#38bdf8", "#a78bfa", "#34d399", "#f472b6", "#fbbf24", "#fb7185", "#22d3ee",
    ];
    let idx = (player_id as usize) % palette.len();
    palette[idx].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::GameAdapter;

    #[derive(Debug)]
    struct TestAdapter {
        key: &'static str,
        prompt_prefix: &'static str,
        score: f32,
    }

    impl GameAdapter for TestAdapter {
        fn game_key(&self) -> &'static str {
            self.key
        }

        fn next_prompt(&self, seed: u64) -> String {
            format!("{}-{seed}", self.prompt_prefix)
        }

        fn is_correct(&self, prompt: &str, attempt: &str) -> bool {
            prompt == attempt.trim()
        }

        fn normalize_progress(&self, raw_input: &str) -> String {
            raw_input.trim().to_string()
        }

        fn score_for_prompt(&self, _prompt: &str) -> f32 {
            self.score
        }
    }

    fn test_state() -> Arc<SharedState> {
        let adapters = build_adapter_registry(vec![
            Arc::new(TestAdapter {
                key: "keyboarding",
                prompt_prefix: "kbd",
                score: 3.0,
            }),
            Arc::new(TestAdapter {
                key: "arithmetic",
                prompt_prefix: "math",
                score: 9.0,
            }),
        ])
        .expect("adapter registry");

        Arc::new(SharedState {
            adapters,
            default_game_key: "keyboarding".to_string(),
            config: ServerConfig::default(),
            rooms: Mutex::new(HashMap::new()),
            connections: Mutex::new(HashMap::new()),
            rejoin_tokens: Mutex::new(HashMap::new()),
            prompt_seed: AtomicU64::new(1),
        })
    }

    #[tokio::test]
    async fn creates_room_with_requested_game_mode() {
        let state = test_state();
        let (sender, _) = mpsc::unbounded_channel::<Message>();

        let (room_code, _token, _pid) = join_or_create_room(
            &state,
            Some("Alice".to_string()),
            None,
            Some("arithmetic".to_string()),
            None,
            sender,
        )
        .await
        .expect("room created");

        let rooms = state.rooms.lock().await;
        let room = rooms.get(&room_code).expect("room exists");
        assert_eq!(room.game_key, "arithmetic");
    }

    #[tokio::test]
    async fn rejects_unknown_game_mode_on_room_create() {
        let state = test_state();
        let (sender, _) = mpsc::unbounded_channel::<Message>();

        let result = join_or_create_room(
            &state,
            Some("Alice".to_string()),
            None,
            Some("unknown-mode".to_string()),
            None,
            sender,
        )
        .await;

        assert!(result.is_none());
        assert!(state.rooms.lock().await.is_empty());
    }

    #[tokio::test]
    async fn join_existing_room_ignores_requested_game_mode() {
        let state = test_state();
        let (sender_1, _) = mpsc::unbounded_channel::<Message>();
        let (sender_2, _) = mpsc::unbounded_channel::<Message>();

        let (room_code, _token, _pid) = join_or_create_room(
            &state,
            Some("Alice".to_string()),
            None,
            Some("keyboarding".to_string()),
            None,
            sender_1,
        )
        .await
        .expect("room created");

        let (joined_room_code, _token, _pid) = join_or_create_room(
            &state,
            Some("Bob".to_string()),
            Some(room_code.clone()),
            Some("arithmetic".to_string()),
            None,
            sender_2,
        )
        .await
        .expect("joined room");

        assert_eq!(joined_room_code, room_code);
        let rooms = state.rooms.lock().await;
        let room = rooms.get(&room_code).expect("room exists");
        assert_eq!(room.game_key, "keyboarding");
        assert_eq!(room.players.len(), 2);
    }

    #[tokio::test]
    async fn uses_room_adapter_for_prompt_and_scoring() {
        let state = test_state();
        let (sender, _) = mpsc::unbounded_channel::<Message>();
        let (room_code, _token, pid) = join_or_create_room(
            &state,
            Some("Alice".to_string()),
            None,
            Some("arithmetic".to_string()),
            None,
            sender,
        )
        .await
        .expect("room created");

        handle_start_match(&state, &room_code, pid).await;
        let has_prompt = {
            let rooms = state.rooms.lock().await;
            let room = rooms.get(&room_code).expect("room exists");
            assert!(room.match_deadline.is_some());
            !room.prompt.is_empty()
        };
        assert!(has_prompt);
        let prompt = {
            let rooms = state.rooms.lock().await;
            rooms.get(&room_code).expect("room exists").prompt.clone()
        };
        assert!(prompt.starts_with("math-"));

        handle_submission(&state, &room_code, pid, prompt).await;
        let rooms = state.rooms.lock().await;
        let player = rooms
            .get(&room_code)
            .and_then(|room| room.players.get(&pid))
            .expect("player exists");
        assert_eq!(player.size, DEFAULT_START_SIZE + 9.0);
    }

    #[tokio::test]
    async fn frozen_player_cannot_submit() {
        use crate::powerup::PowerUpKind;

        let state = test_state();
        let (sender, _) = mpsc::unbounded_channel::<Message>();
        let (room_code, _token, pid) =
            join_or_create_room(&state, Some("Alice".to_string()), None, None, None, sender)
                .await
                .expect("room created");

        handle_start_match(&state, &room_code, pid).await;

        {
            let mut rooms = state.rooms.lock().await;
            let room = rooms.get_mut(&room_code).expect("room exists");
            room.active_powerups.push(ActivePowerUp {
                kind: PowerUpKind::FreezeAllCompetitors,
                source_player_id: 999,
                expires_at: Instant::now() + Duration::from_secs(15),
            });
        }

        let prompt = {
            let rooms = state.rooms.lock().await;
            rooms.get(&room_code).expect("room exists").prompt.clone()
        };
        handle_submission(&state, &room_code, pid, prompt).await;

        let rooms = state.rooms.lock().await;
        let player = rooms
            .get(&room_code)
            .and_then(|r| r.players.get(&pid))
            .expect("player exists");
        assert_eq!(
            player.size, DEFAULT_START_SIZE,
            "frozen player should not gain points"
        );
    }

    #[tokio::test]
    async fn double_points_doubles_growth() {
        use crate::powerup::PowerUpKind;

        let state = test_state();
        let (sender, _) = mpsc::unbounded_channel::<Message>();
        let (room_code, _token, pid) = join_or_create_room(
            &state,
            Some("Alice".to_string()),
            None,
            Some("arithmetic".to_string()),
            None,
            sender,
        )
        .await
        .expect("room created");

        handle_start_match(&state, &room_code, pid).await;

        {
            let mut rooms = state.rooms.lock().await;
            let room = rooms.get_mut(&room_code).expect("room exists");
            room.active_powerups.push(ActivePowerUp {
                kind: PowerUpKind::DoublePoints,
                source_player_id: pid,
                expires_at: Instant::now() + Duration::from_secs(30),
            });
        }

        let prompt = {
            let rooms = state.rooms.lock().await;
            rooms.get(&room_code).expect("room exists").prompt.clone()
        };
        handle_submission(&state, &room_code, pid, prompt).await;

        let rooms = state.rooms.lock().await;
        let player = rooms
            .get(&room_code)
            .and_then(|r| r.players.get(&pid))
            .expect("player exists");
        assert_eq!(
            player.size,
            DEFAULT_START_SIZE + 9.0 * 2.0,
            "growth should be doubled with double-points active"
        );
    }

    #[tokio::test]
    async fn winning_round_earns_pending_powerup() {
        use crate::powerup::PowerUpKind;

        let state = test_state();
        let (sender, _) = mpsc::unbounded_channel::<Message>();
        let (room_code, _token, pid) =
            join_or_create_room(&state, Some("Alice".to_string()), None, None, None, sender)
                .await
                .expect("room created");

        handle_start_match(&state, &room_code, pid).await;

        {
            let mut rooms = state.rooms.lock().await;
            let room = rooms.get_mut(&room_code).expect("room exists");
            room.powerup_offers.push(PowerUpOffer {
                kind: PowerUpKind::FreezeAllCompetitors,
                player_id: pid,
                expires_at: Instant::now() + Duration::from_secs(30),
            });
        }

        let prompt = {
            let rooms = state.rooms.lock().await;
            rooms.get(&room_code).expect("room exists").prompt.clone()
        };
        handle_submission(&state, &room_code, pid, prompt).await;

        let rooms = state.rooms.lock().await;
        let room = rooms.get(&room_code).expect("room exists");
        assert!(room.powerup_offers.is_empty(), "offer should be consumed");
        assert_eq!(room.active_powerups.len(), 1, "one active power-up");
        assert_eq!(
            room.active_powerups[0].kind,
            PowerUpKind::FreezeAllCompetitors
        );
        assert_eq!(room.active_powerups[0].source_player_id, pid);
    }
}
