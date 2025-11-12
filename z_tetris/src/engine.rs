use zenoh_arena::{GameEngine, NodeId};
use crate::tetris::Action;
use crate::tetris_pair::{TetrisPair, PlayerSide};
use crate::tetris::StepResult;
use crate::state::TetrisPairState;
use std::time;

/// Tetris action wrapper
#[derive(Debug, Clone, Copy)]
pub struct TetrisAction {
    pub action: Action,
}

/// Game engine that manages a Tetris game for two players
pub struct TetrisEngine;

impl TetrisEngine {
    pub fn new(
        host_id: NodeId,
        input_rx: flume::Receiver<(NodeId, TetrisAction)>,
        output_tx: flume::Sender<TetrisPairState>,
        _initial_state: Option<TetrisPairState>,
    ) -> Self {
        // Spawn a background task to process actions
        std::thread::spawn(move || {
            let mut tetris_pair = TetrisPair::new(10, 20);
            // Setup game speed
            let step_delay = time::Duration::from_millis(10);
            tetris_pair.set_fall_speed(1, 30);
            tetris_pair.set_drop_speed(1, 1);
            tetris_pair.set_line_remove_speed(3, 5);
            
            // Set player name initially
            tetris_pair.set_player_name(PlayerSide::Player, Some(host_id.to_string()));
            
            let mut opponent_id: Option<NodeId> = None;
            loop {
                let start = time::Instant::now();
                
                // Process all pending actions using try_recv
                while let Ok((client_id, action)) = input_rx.try_recv() {
                    // Determine which player this is based on host_id
                    let player_side = if client_id == host_id {
                        PlayerSide::Player
                    } else {
                        if opponent_id.is_none() {
                            tetris_pair.set_player_name(PlayerSide::Opponent, Some(client_id.to_string()));
                            opponent_id = Some(client_id);
                        }
                        PlayerSide::Opponent
                    };

                    // Add action to the appropriate player
                    tetris_pair.add_player_action(player_side, action.action);
                }
                
                // Perform game step and send state only if something changed
                if tetris_pair.step() != (StepResult::None, StepResult::None) {
                    let state = tetris_pair.get_state();
                    let _ = output_tx.send(state);
                }
                
                // Check for game over and exit thread if game is over
                if tetris_pair.is_game_over() {
                    break;
                }
                
                // Maintain consistent timing
                let elapsed = start.elapsed();
                if elapsed < step_delay {
                    std::thread::sleep(step_delay - elapsed);
                }
            }
        });

        Self
    }
}

impl GameEngine for TetrisEngine {
    type Action = TetrisAction;
    type State = TetrisPairState;

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
        // Test creating a TetrisPair directly since TetrisEngine now uses channels
        let tetris_pair = TetrisPair::new(10, 20);
        assert_eq!(tetris_pair.cols(), 10);
        assert_eq!(tetris_pair.rows(), 20);
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
        // Create a TetrisPair directly for testing
        let tetris_pair = TetrisPair::new(10, 20);
        let state = tetris_pair.get_state();
        
        // Serialize
        let zbytes = zenoh_ext::z_serialize(&state);
        
        // Deserialize
        let deserialized: TetrisPairState = zenoh_ext::z_deserialize(&zbytes).unwrap();
        assert_eq!(deserialized.player.game_over, state.player.game_over);
    }
}
