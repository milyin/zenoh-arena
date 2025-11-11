use zenoh_arena::{GameEngine, NodeId, Result as ArenaResult};
use crate::tetris::Action;
use crate::tetris_pair::{TetrisPair, PlayerSide};
use crate::state::TetrisPairState;

/// Tetris action wrapper
#[derive(Debug, Clone, Copy)]
pub struct TetrisAction {
    pub action: Action,
}

/// Game engine that manages a Tetris game for two players
pub struct TetrisEngine {
    tetris_pair: TetrisPair,
    player_id: Option<NodeId>,
    opponent_id: Option<NodeId>,
}

impl TetrisEngine {
    pub fn new() -> Self {
        let mut tetris_pair = TetrisPair::new(10, 20);
        // Setup game speed
        tetris_pair.set_fall_speed(1, 30);
        tetris_pair.set_drop_speed(1, 1);
        tetris_pair.set_line_remove_speed(3, 5);
        
        Self {
            tetris_pair,
            player_id: None,
            opponent_id: None,
        }
    }
}

impl GameEngine for TetrisEngine {
    type Action = TetrisAction;
    type State = TetrisPairState;

    fn process_action(
        &mut self,
        action: Self::Action,
        client_id: &NodeId,
    ) -> ArenaResult<Self::State> {
        // Assign player IDs on first action
        if self.player_id.is_none() {
            self.player_id = Some(client_id.clone());
        } else if self.opponent_id.is_none() && self.player_id.as_ref() != Some(client_id) {
            self.opponent_id = Some(client_id.clone());
        }

        // Determine which player sent the action
        let player_side = if self.player_id.as_ref() == Some(client_id) {
            PlayerSide::Player
        } else if self.opponent_id.as_ref() == Some(client_id) {
            PlayerSide::Opponent
        } else {
            // Unknown player, ignore
            return Ok(self.tetris_pair.get_state());
        };

        // Add action to the appropriate player
        self.tetris_pair.add_player_action(player_side, action.action);
        
        // Perform game step
        self.tetris_pair.step();
        
        // Return current state
        Ok(self.tetris_pair.get_state())
    }

    fn max_clients(&self) -> Option<usize> {
        Some(2)
    }
}

// Implement zenoh-ext serialization for TetrisAction
impl zenoh_ext::Serialize for TetrisAction {
    fn serialize(&self, serializer: &mut zenoh_ext::ZSerializer) {
        // Serialize action as u8
        let action_byte = match self.action {
            Action::MoveLeft => 0u8,
            Action::MoveRight => 1u8,
            Action::MoveDown => 2u8,
            Action::RotateLeft => 3u8,
            Action::RotateRight => 4u8,
            Action::Drop => 5u8,
            Action::BottomRefill => 6u8,
        };
        action_byte.serialize(serializer);
    }
}

impl zenoh_ext::Deserialize for TetrisAction {
    fn deserialize(deserializer: &mut zenoh_ext::ZDeserializer) -> Result<Self, zenoh_ext::ZDeserializeError> {
        let action_byte: u8 = u8::deserialize(deserializer)?;
        let action = match action_byte {
            0 => Action::MoveLeft,
            1 => Action::MoveRight,
            2 => Action::MoveDown,
            3 => Action::RotateLeft,
            4 => Action::RotateRight,
            5 => Action::Drop,
            6 => Action::BottomRefill,
            // Default to MoveDown for any invalid byte
            _ => Action::MoveDown,
        };
        Ok(TetrisAction { action })
    }
}

// Implement zenoh-ext serialization for TetrisPairState
impl zenoh_ext::Serialize for TetrisPairState {
    fn serialize(&self, serializer: &mut zenoh_ext::ZSerializer) {
        // Use serde_json for complex state serialization
        let json = serde_json::to_string(self).unwrap();
        json.serialize(serializer);
    }
}

impl zenoh_ext::Deserialize for TetrisPairState {
    fn deserialize(deserializer: &mut zenoh_ext::ZDeserializer) -> Result<Self, zenoh_ext::ZDeserializeError> {
        let json: String = String::deserialize(deserializer)?;
        // Use expect to panic on JSON errors - should not happen with valid data
        Ok(serde_json::from_str(&json).expect("Failed to deserialize TetrisPairState from JSON"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = TetrisEngine::new();
        assert_eq!(engine.tetris_pair.cols(), 10);
        assert_eq!(engine.tetris_pair.rows(), 20);
    }

    #[test]
    fn test_action_serialization() {
        let action = TetrisAction { action: Action::MoveLeft };
        
        // Serialize
        let zbytes = zenoh_ext::z_serialize(&action);
        
        // Deserialize
        let deserialized: TetrisAction = zenoh_ext::z_deserialize(&zbytes).unwrap();
        // Check that actions match
        assert!(matches!(deserialized.action, Action::MoveLeft));
    }

    #[test]
    fn test_state_serialization() {
        let engine = TetrisEngine::new();
        let state = engine.tetris_pair.get_state();
        
        // Serialize
        let zbytes = zenoh_ext::z_serialize(&state);
        
        // Deserialize
        let deserialized: TetrisPairState = zenoh_ext::z_deserialize(&zbytes).unwrap();
        assert_eq!(deserialized.player.game_over, state.player.game_over);
    }
}
