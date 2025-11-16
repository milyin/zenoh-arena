use zenoh_arena::{GameEngine, NodeId};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

/// Action type - Bonjour increments counter, Bonsoir decrements it
#[derive(Debug, Clone)]
pub enum BonjourAction {
    Bonjour,
    Bonsoir,
}

/// State type - tracks bonjours counter
#[derive(Debug, Clone)]
pub struct BonjourState {
    pub bonjours: i64,
}

impl BonjourState {
    pub fn new() -> Self {
        Self { 
            bonjours: 0,
        }
    }
}

impl Default for BonjourState {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for BonjourState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bonjours: {}", self.bonjours)
    }
}

/// Game engine that maintains a counter and modifies it based on actions
pub struct BonjourEngine {
    node_id: Mutex<Option<NodeId>>,
    input_tx: flume::Sender<(NodeId, BonjourAction)>,
    input_rx: flume::Receiver<(NodeId, BonjourAction)>,
    output_tx: flume::Sender<BonjourState>,
    output_rx: flume::Receiver<BonjourState>,
    stop_flag: Arc<AtomicBool>,
}

impl BonjourEngine {
    pub fn new() -> Self {
        let (input_tx, input_rx) = flume::unbounded();
        let (output_tx, output_rx) = flume::unbounded();
        
        Self {
            node_id: Mutex::new(None),
            input_tx,
            input_rx,
            output_tx,
            output_rx,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl GameEngine for BonjourEngine {
    type Action = BonjourAction;
    type State = BonjourState;

    fn max_clients(&self) -> Option<usize> {
        Some(2)
    }
    
    fn set_node_id(&self, node_id: NodeId) {
        *self.node_id.lock().unwrap() = Some(node_id);
    }
    
    fn run(&self, initial_state: Option<BonjourState>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let input_rx = self.input_rx.clone();
            let output_tx = self.output_tx.clone();
            let stop_flag = self.stop_flag.clone();
            let mut state = initial_state.unwrap_or_default();
            
            // Reset stop flag
            stop_flag.store(false, Ordering::Relaxed);
            
            // Spawn a task to process actions
            std::thread::spawn(move || {
                while !stop_flag.load(Ordering::Relaxed) {
                    match input_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                        Ok((_node_id, action)) => {
                            // Update bonjours counter based on action type
                            match action {
                                BonjourAction::Bonjour => state.bonjours += 1,
                                BonjourAction::Bonsoir => state.bonjours -= 1,
                            }
                            let _ = output_tx.send(state.clone());
                        }
                        Err(flume::RecvTimeoutError::Timeout) => continue,
                        Err(flume::RecvTimeoutError::Disconnected) => break,
                    }
                }
            });
        })
    }
    
    fn stop(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            self.stop_flag.store(true, Ordering::Relaxed);
        })
    }
    
    fn action_sender(&self) -> &flume::Sender<(NodeId, BonjourAction)> {
        &self.input_tx
    }
    
    fn state_receiver(&self) -> &flume::Receiver<BonjourState> {
        &self.output_rx
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
        self.bonjours.serialize(serializer);
    }
}

impl zenoh_ext::Deserialize for BonjourState {
    fn deserialize(deserializer: &mut zenoh_ext::ZDeserializer) -> Result<Self, zenoh_ext::ZDeserializeError> {
        let bonjours: i64 = i64::deserialize(deserializer)?;
        Ok(BonjourState { bonjours })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_increments_bonjours() {
        // Create engine
        let engine = BonjourEngine::new();
        engine.set_node_id(NodeId::generate());
        
        // Run engine
        engine.run(None).await;
        
        // Send first Bonjour action
        engine.action_sender().send((NodeId::generate(), BonjourAction::Bonjour)).unwrap();
        let state1 = engine.state_receiver().recv().unwrap();
        assert_eq!(state1.bonjours, 1);
        
        // Send second Bonjour action
        engine.action_sender().send((NodeId::generate(), BonjourAction::Bonjour)).unwrap();
        let state2 = engine.state_receiver().recv().unwrap();
        assert_eq!(state2.bonjours, 2);
        
        // Stop engine
        engine.stop().await;
    }

    #[tokio::test]
    async fn test_engine_decrements_bonjours() {
        // Create engine
        let engine = BonjourEngine::new();
        engine.set_node_id(NodeId::generate());
        
        // Run engine
        engine.run(None).await;
        
        // Send Bonsoir action
        engine.action_sender().send((NodeId::generate(), BonjourAction::Bonsoir)).unwrap();
        let state1 = engine.state_receiver().recv().unwrap();
        assert_eq!(state1.bonjours, -1);
        
        // Send another Bonsoir action
        engine.action_sender().send((NodeId::generate(), BonjourAction::Bonsoir)).unwrap();
        let state2 = engine.state_receiver().recv().unwrap();
        assert_eq!(state2.bonjours, -2);
        
        // Stop engine
        engine.stop().await;
    }

    #[tokio::test]
    async fn test_engine_mixed_actions() {
        // Create engine
        let engine = BonjourEngine::new();
        engine.set_node_id(NodeId::generate());
        
        // Run engine
        engine.run(None).await;
        
        // Bonjour +1
        engine.action_sender().send((NodeId::generate(), BonjourAction::Bonjour)).unwrap();
        let state = engine.state_receiver().recv().unwrap();
        assert_eq!(state.bonjours, 1);
        
        // Bonjour +1
        engine.action_sender().send((NodeId::generate(), BonjourAction::Bonjour)).unwrap();
        let state = engine.state_receiver().recv().unwrap();
        assert_eq!(state.bonjours, 2);
        
        // Bonsoir -1
        engine.action_sender().send((NodeId::generate(), BonjourAction::Bonsoir)).unwrap();
        let state = engine.state_receiver().recv().unwrap();
        assert_eq!(state.bonjours, 1);
        
        // Bonsoir -1
        engine.action_sender().send((NodeId::generate(), BonjourAction::Bonsoir)).unwrap();
        let state = engine.state_receiver().recv().unwrap();
        assert_eq!(state.bonjours, 0);
        
        // Bonsoir -1
        engine.action_sender().send((NodeId::generate(), BonjourAction::Bonsoir)).unwrap();
        let state = engine.state_receiver().recv().unwrap();
        assert_eq!(state.bonjours, -1);
        
        // Stop engine
        engine.stop().await;
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
        let state = BonjourState { bonjours: 42 };
        
        // Serialize
        let zbytes = zenoh_ext::z_serialize(&state);
        
        // Deserialize
        let deserialized: BonjourState = zenoh_ext::z_deserialize(&zbytes).unwrap();
        assert_eq!(deserialized.bonjours, 42);
    }

    #[test]
    fn test_state_serialization_negative() {
        let state = BonjourState { bonjours: -42 };
        
        // Serialize
        let zbytes = zenoh_ext::z_serialize(&state);
        
        // Deserialize
        let deserialized: BonjourState = zenoh_ext::z_deserialize(&zbytes).unwrap();
        assert_eq!(deserialized.bonjours, -42);
    }

    #[test]
    fn test_state_display() {
        let state = BonjourState { bonjours: 42 };
        assert_eq!(format!("{}", state), "Bonjours: 42");
    }
}
