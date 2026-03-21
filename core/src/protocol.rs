use crate::game::{PlayerId, RoomSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMessage {
    JoinOrCreateRoom {
        #[serde(rename = "playerName")]
        player_name: Option<String>,
        #[serde(rename = "roomCode")]
        room_code: Option<String>,
        #[serde(rename = "gameMode")]
        game_mode: Option<String>,
    },
    RejoinRoom {
        #[serde(rename = "rejoinToken")]
        rejoin_token: String,
    },
    InputUpdate {
        text: String,
    },
    SubmitAttempt {
        text: String,
    },
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    Welcome {
        #[serde(rename = "playerId")]
        player_id: PlayerId,
        #[serde(rename = "roomCode")]
        room_code: String,
        #[serde(rename = "gameKey")]
        game_key: String,
        #[serde(rename = "minEatableSize")]
        min_eatable_size: f32,
        #[serde(rename = "rejoinToken")]
        rejoin_token: String,
    },
    RoomState {
        room: RoomSnapshot,
    },
    PromptState {
        #[serde(rename = "roomCode")]
        room_code: String,
        #[serde(rename = "roundId")]
        round_id: u64,
        prompt: String,
    },
    RaceProgress {
        #[serde(rename = "roomCode")]
        room_code: String,
        #[serde(rename = "playerId")]
        player_id: PlayerId,
        text: String,
    },
    RoundResult {
        #[serde(rename = "roomCode")]
        room_code: String,
        #[serde(rename = "roundId")]
        round_id: u64,
        #[serde(rename = "winnerPlayerId")]
        winner_player_id: PlayerId,
        #[serde(rename = "growthAwarded")]
        growth_awarded: f32,
        #[serde(rename = "consumedPlayerIds")]
        consumed_player_ids: Vec<PlayerId>,
        #[serde(rename = "matchWinner")]
        match_winner: Option<PlayerId>,
    },
    Error {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::ClientMessage;

    #[test]
    fn parses_all_supported_client_messages() {
        let join = r#"{"type":"joinOrCreateRoom","playerName":"Alice","roomCode":"ABCD","gameMode":"keyboarding"}"#;
        assert!(serde_json::from_str::<ClientMessage>(join).is_ok());

        let rejoin = r#"{"type":"rejoinRoom","rejoinToken":"abc123"}"#;
        assert!(serde_json::from_str::<ClientMessage>(rejoin).is_ok());

        let update = r#"{"type":"inputUpdate","text":"hel"}"#;
        assert!(serde_json::from_str::<ClientMessage>(update).is_ok());

        let submit = r#"{"type":"submitAttempt","text":"hello"}"#;
        assert!(serde_json::from_str::<ClientMessage>(submit).is_ok());
    }

    #[test]
    fn rejects_removed_ping_message() {
        let ping = r#"{"type":"ping","sentAtMs":123}"#;
        assert!(serde_json::from_str::<ClientMessage>(ping).is_err());
    }
}
