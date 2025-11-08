//! Configuration for a Node

/// Main configuration for a Node
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Optional node name (auto-generated if None)
    pub node_name: Option<String>,

    /// Host discovery timeout (in milliseconds)
    pub discovery_timeout_ms: u64,

    /// Random jitter range for discovery timeout (0.0 - 1.0)
    pub discovery_jitter: f64,

    /// Maximum number of clients per host (None = unlimited)
    pub max_clients: Option<usize>,

    /// Whether to force host mode (blocks Searching and Client states)
    pub force_host: bool,

    /// Key expression prefix for arena communication
    pub keyexpr_prefix: String,

    /// Timeout for step() method in milliseconds
    /// step() returns when either new game state is available or this timeout elapses
    pub step_timeout_ms: u64,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_name: None,
            discovery_timeout_ms: 5000,
            discovery_jitter: 0.3,
            max_clients: Some(4),
            force_host: false,
            keyexpr_prefix: "zenoh/arena".to_string(),
            step_timeout_ms: 100,
        }
    }
}

impl NodeConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the node name
    pub fn with_node_name(mut self, name: String) -> Self {
        self.node_name = Some(name);
        self
    }

    /// Set the discovery timeout in milliseconds
    pub fn with_discovery_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.discovery_timeout_ms = timeout_ms;
        self
    }

    /// Set the discovery jitter (0.0 - 1.0)
    pub fn with_discovery_jitter(mut self, jitter: f64) -> Self {
        self.discovery_jitter = jitter.clamp(0.0, 1.0);
        self
    }

    /// Set the maximum number of clients
    pub fn with_max_clients(mut self, max_clients: Option<usize>) -> Self {
        self.max_clients = max_clients;
        self
    }

    /// Set whether to force host mode (blocks Searching and Client states)
    pub fn with_force_host(mut self, force_host: bool) -> Self {
        self.force_host = force_host;
        self
    }

    /// Set the key expression prefix
    pub fn with_keyexpr_prefix(mut self, prefix: String) -> Self {
        self.keyexpr_prefix = prefix;
        self
    }

    /// Set the step timeout in milliseconds
    pub fn with_step_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.step_timeout_ms = timeout_ms;
        self
    }
}
