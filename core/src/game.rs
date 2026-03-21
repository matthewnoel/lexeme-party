use serde::Serialize;
use std::collections::HashMap;

pub const DEFAULT_START_SIZE: f32 = 10.0;
pub const MIN_EATABLE_SIZE: f32 = 18.0;

pub type PlayerId = u64;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub id: PlayerId,
    pub name: String,
    pub size: f32,
    pub color: String,
    pub connected: bool,
    pub progress: String,
}

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub id: PlayerId,
    pub name: String,
    pub size: f32,
    pub color: String,
    pub connected: bool,
    pub progress: String,
    pub rejoin_token: String,
}

impl PlayerState {
    pub fn to_snapshot(&self) -> PlayerSnapshot {
        PlayerSnapshot {
            id: self.id,
            name: self.name.clone(),
            size: self.size,
            color: self.color.clone(),
            connected: self.connected,
            progress: self.progress.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomSnapshot {
    pub room_code: String,
    pub players: Vec<PlayerSnapshot>,
    pub prompt: String,
    pub round_id: u64,
    pub match_winner: Option<PlayerId>,
}

#[derive(Debug, Clone)]
pub struct RoomState {
    pub room_code: String,
    pub game_key: String,
    pub players: HashMap<PlayerId, PlayerState>,
    pub prompt: String,
    pub round_id: u64,
    pub match_winner: Option<PlayerId>,
    pub next_player_id: u64,
}

impl RoomState {
    pub fn to_snapshot(&self) -> RoomSnapshot {
        let mut players: Vec<PlayerSnapshot> = self
            .players
            .values()
            .map(PlayerState::to_snapshot)
            .collect();
        players.sort_by_key(|p| p.id);

        RoomSnapshot {
            room_code: self.room_code.clone(),
            players,
            prompt: self.prompt.clone(),
            round_id: self.round_id,
            match_winner: self.match_winner,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundResolution {
    pub round_winner: PlayerId,
    pub consumed_player_ids: Vec<PlayerId>,
    pub match_winner: Option<PlayerId>,
}

pub fn apply_round_win(
    room: &mut RoomState,
    winner_id: PlayerId,
    awarded_growth: f32,
    min_eatable_size: f32,
) -> Option<RoundResolution> {
    let winner = room.players.get_mut(&winner_id)?;
    winner.size += awarded_growth;
    winner.progress.clear();

    let winner_size = winner.size;
    let consumed_player_ids = if winner_size >= min_eatable_size {
        room.players
            .values()
            .filter(|p| p.id != winner_id && p.size < winner_size)
            .map(|p| p.id)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    for player_id in &consumed_player_ids {
        room.players.remove(player_id);
    }

    room.match_winner = evaluate_match_winner(&room.players);
    Some(RoundResolution {
        round_winner: winner_id,
        consumed_player_ids,
        match_winner: room.match_winner,
    })
}

pub fn evaluate_match_winner(players: &HashMap<PlayerId, PlayerState>) -> Option<PlayerId> {
    if players.len() < 2 {
        return None;
    }

    let mut ranked: Vec<&PlayerState> = players.values().collect();
    ranked.sort_by(|a, b| b.size.total_cmp(&a.size));
    let largest = ranked[0];

    if ranked.len() == 2 {
        let other = ranked[1];
        if largest.size > other.size * 2.0 {
            return Some(largest.id);
        }
        return None;
    }

    let sum_others: f32 = ranked.iter().skip(1).map(|p| p.size).sum();
    if largest.size > sum_others {
        return Some(largest.id);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(id: PlayerId, size: f32) -> PlayerState {
        PlayerState {
            id,
            name: format!("p{id}"),
            size,
            color: "#ffffff".to_string(),
            connected: true,
            progress: String::new(),
            rejoin_token: String::new(),
        }
    }

    #[test]
    fn two_player_win_requires_double_size() {
        let mut players = HashMap::new();
        players.insert(1, player(1, 30.0));
        players.insert(2, player(2, 14.9));
        assert_eq!(evaluate_match_winner(&players), Some(1));

        players.insert(2, player(2, 15.1));
        assert_eq!(evaluate_match_winner(&players), None);
    }

    #[test]
    fn multi_player_win_requires_largest_gt_sum_of_others() {
        let mut players = HashMap::new();
        players.insert(1, player(1, 35.0));
        players.insert(2, player(2, 18.0));
        players.insert(3, player(3, 16.0));
        assert_eq!(evaluate_match_winner(&players), Some(1));

        players.insert(3, player(3, 18.0));
        assert_eq!(evaluate_match_winner(&players), None);
    }

    #[test]
    fn minimum_size_gate_blocks_consumption() {
        let mut room = RoomState {
            room_code: "ABCD".to_string(),
            game_key: "keyboarding".to_string(),
            players: HashMap::from([(1, player(1, 10.0)), (2, player(2, 9.0))]),
            prompt: "abc".to_string(),
            round_id: 1,
            match_winner: None,
            next_player_id: 3,
        };

        let resolution = apply_round_win(&mut room, 1, 1.0, MIN_EATABLE_SIZE).expect("resolution");
        assert!(resolution.consumed_player_ids.is_empty());
        assert!(room.players.contains_key(&2));
    }
}
