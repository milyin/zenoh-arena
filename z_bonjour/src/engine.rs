use zenoh_arena::{GameEngine, NodeId};

/// Action type - simple Bonjour message that increments counter
#[derive(Debug, Clone)]
pub struct BonjourAction;

/// State type - counter value
#[derive(Debug, Clone)]
pub struct BonjourState {
    pub counter: u64,
}

impl BonjourState {
    pub fn new() -> Self {
        Self { counter: 0 }
    }
}

impl std::fmt::Display for BonjourState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Counter = {}", self.counter)
    }
}

/// Game engine that maintains a counter and increments it on each Bonjour action
pub struct BonjourEngine;

impl BonjourEngine {
    pub fn new(input_rx: flume::Receiver<(NodeId, BonjourAction)>, output_tx: flume::Sender<BonjourState>) -> Self {
        let mut state = BonjourState::new();
        
        // Spawn a task to process actions
        std::thread::spawn(move || {
            while let Ok((_node_id, _action)) = input_rx.recv() {
                // Increment counter on each Bonjour action
                state.counter += 1;
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
        // Minimal serialization - just write a tag byte (0 = Bonjour)
        0u8.serialize(serializer);
    }
}

impl zenoh_ext::Deserialize for BonjourAction {
    fn deserialize(deserializer: &mut zenoh_ext::ZDeserializer) -> Result<Self, zenoh_ext::ZDeserializeError> {
        let _tag: u8 = u8::deserialize(deserializer)?;
        Ok(BonjourAction)
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
        let counter: u64 = u64::deserialize(deserializer)?;
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
        
        // Create engine
        let _engine = BonjourEngine::new(input_rx, output_tx);
        
        // Send first action
        input_tx.send((NodeId::generate(), BonjourAction)).unwrap();
        let state1 = output_rx.recv().unwrap();
        assert_eq!(state1.counter, 1);
        
        // Send second action
        input_tx.send((NodeId::generate(), BonjourAction)).unwrap();
        let state2 = output_rx.recv().unwrap();
        assert_eq!(state2.counter, 2);
    }

    #[test]
    fn test_action_serialization() {
        let action = BonjourAction;
        
        // Serialize
        let zbytes = zenoh_ext::z_serialize(&action);
        
        // Deserialize
        let deserialized: BonjourAction = zenoh_ext::z_deserialize(&zbytes).unwrap();
        // Actions are unit structs, so just check they deserialize
        let _ = deserialized;
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
    fn test_state_display() {
        let state = BonjourState { counter: 42 };
        assert_eq!(format!("{}", state), "Counter = 42");
    }
}
