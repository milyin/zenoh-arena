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
pub struct BonjourEngine {
    input_tx: flume::Sender<(NodeId, BonjourAction)>,
    #[allow(dead_code)]
    input_rx: flume::Receiver<(NodeId, BonjourAction)>,
    #[allow(dead_code)]
    output_tx: flume::Sender<BonjourState>,
    output_rx: flume::Receiver<BonjourState>,
}

impl BonjourEngine {
    pub fn new() -> Self {
        let (input_tx, input_rx) = flume::unbounded();
        let (output_tx, output_rx) = flume::unbounded();
        
        let mut state = BonjourState::new();
        
        // Spawn a task to process actions
        let input_rx_clone = input_rx.clone();
        let output_tx_clone = output_tx.clone();
        std::thread::spawn(move || {
            while let Ok((_node_id, _action)) = input_rx_clone.recv() {
                // Increment counter on each Bonjour action
                state.counter += 1;
                let _ = output_tx_clone.send(state.clone());
            }
        });

        Self {
            input_tx,
            input_rx,
            output_tx,
            output_rx,
        }
    }
}

impl GameEngine for BonjourEngine {
    type Action = BonjourAction;
    type State = BonjourState;

    fn max_clients(&self) -> Option<usize> {
        Some(2)
    }

    fn input_sender(&self) -> flume::Sender<(NodeId, Self::Action)> {
        self.input_tx.clone()
    }

    fn output_receiver(&self) -> flume::Receiver<Self::State> {
        self.output_rx.clone()
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
        let engine = BonjourEngine::new();
        
        // Send actions through the input channel
        let input_sender = engine.input_sender();
        let output_receiver = engine.output_receiver();
        
        // Send first action
        input_sender.send((NodeId::generate(), BonjourAction)).unwrap();
        let state1 = output_receiver.recv().unwrap();
        assert_eq!(state1.counter, 1);
        
        // Send second action
        input_sender.send((NodeId::generate(), BonjourAction)).unwrap();
        let state2 = output_receiver.recv().unwrap();
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
