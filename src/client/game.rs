use crate::protocol::{ClientMessage, PlayerState, ServerMessage};
use rand::Rng;
use std::collections::HashMap;
use tokio::sync::mpsc;
use winit::keyboard::{Key, NamedKey};

use super::render::CircleInstance;

const BASE_RADIUS: f32 = 16.0;
const SCORE_RADIUS_STEP: f32 = 4.0;
const GRAVITY_TO_CENTER: f32 = 42.0;
const VELOCITY_DAMPING: f32 = 0.90;

#[derive(Debug, Clone)]
pub struct RenderPlayer {
    pub id: u64,
    pub name: String,
    pub score: u32,
    pub typed: String,
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub color: [f32; 3],
}

impl RenderPlayer {
    pub fn radius(&self) -> f32 {
        BASE_RADIUS + self.score as f32 * SCORE_RADIUS_STEP
    }
}

pub struct GameClient {
    pub local_name: String,
    pub local_player_id: Option<u64>,
    pub round: u32,
    pub current_word: String,
    pub typed_word: String,
    pub winner_last_round: Option<String>,
    pub players: HashMap<u64, RenderPlayer>,
    net_tx: mpsc::UnboundedSender<ClientMessage>,
}

impl GameClient {
    pub fn new(local_name: String, net_tx: mpsc::UnboundedSender<ClientMessage>) -> Self {
        Self {
            local_name,
            local_player_id: None,
            round: 1,
            current_word: "waiting".to_string(),
            typed_word: String::new(),
            winner_last_round: None,
            players: HashMap::new(),
            net_tx,
        }
    }

    pub fn apply_server_msg(&mut self, msg: ServerMessage, screen_size: [f32; 2]) {
        match msg {
            ServerMessage::Welcome { player_id } => {
                self.local_player_id = Some(player_id);
            }
            ServerMessage::State {
                round,
                current_word,
                players,
                winner_last_round,
            } => {
                if self.current_word != current_word {
                    self.typed_word.clear();
                }
                self.round = round;
                self.current_word = current_word;
                self.winner_last_round = winner_last_round;
                self.sync_players(players, screen_size);
            }
        }
    }

    fn sync_players(&mut self, incoming: Vec<PlayerState>, screen_size: [f32; 2]) {
        let mut rng = rand::thread_rng();
        let half_w = (screen_size[0] * 0.5).max(1.0);
        let half_h = (screen_size[1] * 0.5).max(1.0);

        let mut next_map = HashMap::new();
        for p in incoming {
            if let Some(existing) = self.players.remove(&p.id) {
                next_map.insert(
                    p.id,
                    RenderPlayer {
                        id: p.id,
                        name: p.name,
                        score: p.score,
                        typed: p.typed,
                        ..existing
                    },
                );
            } else {
                let x = rng.gen_range(-half_w * 0.6..half_w * 0.6);
                let y = rng.gen_range(-half_h * 0.6..half_h * 0.6);
                next_map.insert(
                    p.id,
                    RenderPlayer {
                        id: p.id,
                        name: p.name,
                        score: p.score,
                        typed: p.typed,
                        pos: [x, y],
                        vel: [0.0, 0.0],
                        color: color_from_id(p.id),
                    },
                );
            }
        }

        self.players = next_map;
    }

    pub fn step_physics(&mut self, dt: f32, screen_size: [f32; 2]) {
        if self.players.is_empty() {
            return;
        }

        let ids: Vec<u64> = self.players.keys().copied().collect();
        for id in &ids {
            if let Some(p) = self.players.get_mut(id) {
                let fx = -p.pos[0] * GRAVITY_TO_CENTER;
                let fy = -p.pos[1] * GRAVITY_TO_CENTER;
                p.vel[0] += fx * dt;
                p.vel[1] += fy * dt;
                p.vel[0] *= VELOCITY_DAMPING;
                p.vel[1] *= VELOCITY_DAMPING;
                p.pos[0] += p.vel[0] * dt;
                p.pos[1] += p.vel[1] * dt;
            }
        }

        let mut pairs = Vec::new();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                pairs.push((ids[i], ids[j]));
            }
        }

        for (a_id, b_id) in pairs {
            let (a_pos, b_pos, a_r, b_r) =
                if let (Some(a), Some(b)) = (self.players.get(&a_id), self.players.get(&b_id)) {
                    (a.pos, b.pos, a.radius(), b.radius())
                } else {
                    continue;
                };

            let dx = b_pos[0] - a_pos[0];
            let dy = b_pos[1] - a_pos[1];
            let dist_sq = dx * dx + dy * dy;
            let min_dist = a_r + b_r + 2.0;
            if dist_sq <= f32::EPSILON {
                continue;
            }
            let dist = dist_sq.sqrt();
            if dist >= min_dist {
                continue;
            }

            let nx = dx / dist;
            let ny = dy / dist;
            let push = (min_dist - dist) * 0.5;

            if let Some(a) = self.players.get_mut(&a_id) {
                a.pos[0] -= nx * push;
                a.pos[1] -= ny * push;
            }
            if let Some(b) = self.players.get_mut(&b_id) {
                b.pos[0] += nx * push;
                b.pos[1] += ny * push;
            }
        }

        let limit_x = (screen_size[0] * 0.5).max(1.0);
        let limit_y = (screen_size[1] * 0.5).max(1.0);
        for p in self.players.values_mut() {
            let r = p.radius();
            p.pos[0] = p.pos[0].clamp(-limit_x + r, limit_x - r);
            p.pos[1] = p.pos[1].clamp(-limit_y + r, limit_y - r);
        }
    }

    pub fn handle_key(&mut self, key: &Key) {
        let mut changed = false;
        match key {
            Key::Named(NamedKey::Backspace) => {
                if self.typed_word.pop().is_some() {
                    changed = true;
                }
            }
            Key::Named(NamedKey::Enter) => {
                self.try_submit();
            }
            Key::Character(s) => {
                for c in s.chars() {
                    if c.is_ascii_alphabetic() {
                        self.typed_word.push(c.to_ascii_lowercase());
                        changed = true;
                    }
                }
                if self.typed_word.eq_ignore_ascii_case(&self.current_word) {
                    self.try_submit();
                }
            }
            _ => {}
        }
        if changed {
            self.send_typed_progress();
        }
    }

    fn try_submit(&mut self) {
        if self.typed_word.eq_ignore_ascii_case(&self.current_word) && !self.current_word.is_empty()
        {
            let _ = self.net_tx.send(ClientMessage::SubmitWord {
                word: self.typed_word.clone(),
            });
            self.typed_word.clear();
            self.send_typed_progress();
        }
    }

    fn send_typed_progress(&self) {
        let _ = self.net_tx.send(ClientMessage::TypedProgress {
            typed: self.typed_word.clone(),
        });
    }

    pub fn build_instances(&self) -> Vec<CircleInstance> {
        let mut list = Vec::with_capacity(self.players.len());
        for player in self.players.values() {
            let mut color = player.color;
            if Some(player.id) == self.local_player_id {
                color = [1.0, 0.95, 0.35];
            }
            list.push(CircleInstance {
                pos: player.pos,
                radius: player.radius(),
                color,
                _pad: 0.0,
            });
        }
        list
    }

    pub fn update_window_title(&self, window: &winit::window::Window) {
        let my_score = self
            .local_player_id
            .and_then(|id| self.players.get(&id).map(|p| p.score))
            .unwrap_or(0);
        let winner = self
            .winner_last_round
            .as_ref()
            .map_or("none".to_string(), |w| w.clone());
        let title = format!(
            "Lexeme Party | Round {} | Word: {} | Typed: {} | You: {} ({}) | Last winner: {}",
            self.round,
            self.current_word,
            self.typed_word,
            self.local_name,
            my_score,
            winner
        );
        window.set_title(&title);
    }

    pub fn build_letter_colors(&self) -> Vec<[u8; 4]> {
        let word_chars: Vec<char> = self.current_word.chars().collect();
        let mut colors = vec![[170, 170, 170, 255]; word_chars.len()];
        if word_chars.is_empty() {
            return colors;
        }

        let local_typed: Vec<char> = self.typed_word.chars().collect();
        for (idx, typed_c) in local_typed.iter().enumerate() {
            if idx >= word_chars.len() {
                break;
            }
            colors[idx] = if typed_c.eq_ignore_ascii_case(&word_chars[idx]) {
                [100, 230, 120, 255]
            } else {
                [235, 90, 90, 255]
            };
        }

        let mut crowd_correct_counts = vec![0u32; word_chars.len()];
        for p in self.players.values() {
            if Some(p.id) == self.local_player_id {
                continue;
            }
            let typed_chars: Vec<char> = p.typed.chars().collect();
            let mut prefix = 0usize;
            while prefix < typed_chars.len() && prefix < word_chars.len() {
                if typed_chars[prefix].eq_ignore_ascii_case(&word_chars[prefix]) {
                    prefix += 1;
                } else {
                    break;
                }
            }
            for count in crowd_correct_counts.iter_mut().take(prefix) {
                *count += 1;
            }
        }

        for i in 0..word_chars.len() {
            if crowd_correct_counts[i] == 0 {
                continue;
            }
            let boost = (crowd_correct_counts[i] * 32).min(120) as u8;
            let base = colors[i];
            colors[i] = [
                base[0].saturating_add(boost / 3),
                base[1].saturating_add(boost / 2),
                base[2].saturating_add(boost),
                255,
            ];
        }

        colors
    }

    pub fn build_leaderboard_lines(&self) -> Vec<(String, [u8; 4])> {
        let mut rows: Vec<(u64, u32)> = self
            .players
            .values()
            .map(|p| (p.id, p.score))
            .collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let mut lines = Vec::with_capacity(rows.len() + 1);
        lines.push(("LEADERBOARD".to_string(), [220, 220, 255, 255]));
        for (id, score) in rows {
            let name = self
                .players
                .get(&id)
                .map(|p| {
                    if p.name.is_empty() {
                        format!("player-{}", id)
                    } else {
                        p.name.clone()
                    }
                })
                .unwrap_or_else(|| format!("player-{}", id));
            let text = format!("{}: {}", name, score);
            let color = if Some(id) == self.local_player_id {
                [255, 235, 120, 255]
            } else {
                [200, 200, 200, 255]
            };
            lines.push((text, color));
        }
        lines
    }
}

fn color_from_id(id: u64) -> [f32; 3] {
    let mut x = id.wrapping_mul(0x9E37_79B1_85EB_CA87);
    x ^= x >> 33;
    let r = ((x & 0xFF) as f32 / 255.0) * 0.6 + 0.25;
    let g = (((x >> 8) & 0xFF) as f32 / 255.0) * 0.6 + 0.25;
    let b = (((x >> 16) & 0xFF) as f32 / 255.0) * 0.6 + 0.25;
    [r.min(1.0), g.min(1.0), b.min(1.0)]
}
