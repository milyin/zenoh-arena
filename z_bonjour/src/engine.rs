use zenoh_arena::{GameEngine, NodeId, Result as ArenaResult};

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
    state: BonjourState,
}

impl BonjourEngine {
    pub fn new() -> Self {
        Self {
            state: BonjourState::new(),
        }
    }
}

impl GameEngine for BonjourEngine {
    type Action = BonjourAction;
    type State = BonjourState;

    fn process_action(
        &mut self,
        _action: Self::Action,
        _client_id: &NodeId,
    ) -> ArenaResult<Self::State> {
        // Increment counter on each Bonjour action
        self.state.counter += 1;
        Ok(self.state.clone())
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
        let mut engine = BonjourEngine::new();
        
        // Initial state should be 0
        assert_eq!(engine.state.counter, 0);
        
        // Process action should increment
        let state1 = engine.process_action(BonjourAction, &NodeId::generate()).unwrap();
        assert_eq!(state1.counter, 1);
        
        // Process another action
        let state2 = engine.process_action(BonjourAction, &NodeId::generate()).unwrap();
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
