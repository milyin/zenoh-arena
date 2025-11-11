use zenoh_arena::{GameEngine, NodeId};

/// Action type - Bonjour increments counter, Bonsoir decrements it
#[derive(Debug, Clone)]
pub enum BonjourAction {
    Bonjour,
    Bonsoir,
}

/// State type - counter value (signed to allow negative values)
#[derive(Debug, Clone)]
pub struct BonjourState {
    pub counter: i64,
}

impl BonjourState {
    pub fn new() -> Self {
        Self { counter: 0 }
    }
}

impl Default for BonjourState {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for BonjourState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Counter = {}", self.counter)
    }
}

/// Game engine that maintains a counter and modifies it based on actions
pub struct BonjourEngine;

impl BonjourEngine {
    pub fn new(
        input_rx: flume::Receiver<(NodeId, BonjourAction)>,
        output_tx: flume::Sender<BonjourState>,
        initial_state: Option<BonjourState>,
    ) -> Self {
        let mut state = initial_state.unwrap_or_default();
        
        // Spawn a task to process actions
        std::thread::spawn(move || {
            while let Ok((_node_id, action)) = input_rx.recv() {
                // Update counter based on action type
                match action {
                    BonjourAction::Bonjour => state.counter += 1,
                    BonjourAction::Bonsoir => state.counter -= 1,
                }
                let _ = output_tx.send(state.clone());
            }
        });

        Self
    }
}

impl GameEngine for BonjourEngine {
    type Action = BonjourAction;
    type State = BonjourState;

    fn max_clients(&self) -> Option<usize> {
        Some(2)
    }
}

// Implement zenoh-ext serialization for BonjourAction
impl zenoh_ext::Serialize for BonjourAction {
    fn serialize(&self, serializer: &mut zenoh_ext::ZSerializer) {
        // Serialize with tag byte: 0 = Bonjour, 1 = Bonsoir
        let tag: u8 = match self {
            BonjourAction::Bonjour => 0,
            BonjourAction::Bonsoir => 1,
        };
        tag.serialize(serializer);
    }
}

impl zenoh_ext::Deserialize for BonjourAction {
    fn deserialize(deserializer: &mut zenoh_ext::ZDeserializer) -> Result<Self, zenoh_ext::ZDeserializeError> {
        let tag: u8 = u8::deserialize(deserializer)?;
        match tag {
            0 => Ok(BonjourAction::Bonjour),
            1 => Ok(BonjourAction::Bonsoir),
            // Default to Bonjour for any invalid byte
            _ => Ok(BonjourAction::Bonjour),
        }
    }
}

// Implement zenoh-ext serialization for BonjourState
impl zenoh_ext::Serialize for BonjourState {
    fn serialize(&self, serializer: &mut zenoh_ext::ZSerializer) {
        self.counter.serialize(serializer);
    }
}

impl zenoh_ext::Deserialize for BonjourState {
    fn deserialize(deserializer: &mut zenoh_ext::ZDeserializer) -> Result<Self, zenoh_ext::ZDeserializeError> {
        let counter: i64 = i64::deserialize(deserializer)?;
        Ok(BonjourState { counter })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_increments_counter() {
        // Create channels
        let (input_tx, input_rx) = flume::unbounded();
        let (output_tx, output_rx) = flume::unbounded();
        
        // Create engine with no initial state
        let _engine = BonjourEngine::new(input_rx, output_tx, None);
        
        // Send first Bonjour action
        input_tx.send((NodeId::generate(), BonjourAction::Bonjour)).unwrap();
        let state1 = output_rx.recv().unwrap();
        assert_eq!(state1.counter, 1);
        
        // Send second Bonjour action
        input_tx.send((NodeId::generate(), BonjourAction::Bonjour)).unwrap();
        let state2 = output_rx.recv().unwrap();
        assert_eq!(state2.counter, 2);
    }

    #[test]
    fn test_engine_decrements_counter() {
        // Create channels
        let (input_tx, input_rx) = flume::unbounded();
        let (output_tx, output_rx) = flume::unbounded();
        
        // Create engine with no initial state
        let _engine = BonjourEngine::new(input_rx, output_tx, None);
        
        // Send Bonsoir action
        input_tx.send((NodeId::generate(), BonjourAction::Bonsoir)).unwrap();
        let state1 = output_rx.recv().unwrap();
        assert_eq!(state1.counter, -1);
        
        // Send another Bonsoir action
        input_tx.send((NodeId::generate(), BonjourAction::Bonsoir)).unwrap();
        let state2 = output_rx.recv().unwrap();
        assert_eq!(state2.counter, -2);
    }

    #[test]
    fn test_engine_mixed_actions() {
        // Create channels
        let (input_tx, input_rx) = flume::unbounded();
        let (output_tx, output_rx) = flume::unbounded();
        
        // Create engine with no initial state
        let _engine = BonjourEngine::new(input_rx, output_tx, None);
        
        // Bonjour +1
        input_tx.send((NodeId::generate(), BonjourAction::Bonjour)).unwrap();
        let state = output_rx.recv().unwrap();
        assert_eq!(state.counter, 1);
        
        // Bonjour +1
        input_tx.send((NodeId::generate(), BonjourAction::Bonjour)).unwrap();
        let state = output_rx.recv().unwrap();
        assert_eq!(state.counter, 2);
        
        // Bonsoir -1
        input_tx.send((NodeId::generate(), BonjourAction::Bonsoir)).unwrap();
        let state = output_rx.recv().unwrap();
        assert_eq!(state.counter, 1);
        
        // Bonsoir -1
        input_tx.send((NodeId::generate(), BonjourAction::Bonsoir)).unwrap();
        let state = output_rx.recv().unwrap();
        assert_eq!(state.counter, 0);
        
        // Bonsoir -1
        input_tx.send((NodeId::generate(), BonjourAction::Bonsoir)).unwrap();
        let state = output_rx.recv().unwrap();
        assert_eq!(state.counter, -1);
    }

    #[test]
    fn test_action_serialization_bonjour() {
        let action = BonjourAction::Bonjour;
        
        // Serialize
        let zbytes = zenoh_ext::z_serialize(&action);
        
        // Deserialize
        let deserialized: BonjourAction = zenoh_ext::z_deserialize(&zbytes).unwrap();
        matches!(deserialized, BonjourAction::Bonjour);
    }

    #[test]
    fn test_action_serialization_bonsoir() {
        let action = BonjourAction::Bonsoir;
        
        // Serialize
        let zbytes = zenoh_ext::z_serialize(&action);
        
        // Deserialize
        let deserialized: BonjourAction = zenoh_ext::z_deserialize(&zbytes).unwrap();
        matches!(deserialized, BonjourAction::Bonsoir);
    }

    #[test]
    fn test_state_serialization() {
        let state = BonjourState { counter: 42 };
        
        // Serialize
        let zbytes = zenoh_ext::z_serialize(&state);
        
        // Deserialize
        let deserialized: BonjourState = zenoh_ext::z_deserialize(&zbytes).unwrap();
        assert_eq!(deserialized.counter, 42);
    }

    #[test]
    fn test_state_serialization_negative() {
        let state = BonjourState { counter: -42 };
        
        // Serialize
        let zbytes = zenoh_ext::z_serialize(&state);
        
        // Deserialize
        let deserialized: BonjourState = zenoh_ext::z_deserialize(&zbytes).unwrap();
        assert_eq!(deserialized.counter, -42);
    }

    #[test]
    fn test_state_display() {
        let state = BonjourState { counter: 42 };
        assert_eq!(format!("{}", state), "Counter = 42");
    }
}
