//! Configuration for a Node

/// Main configuration for a Node
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Optional node name (auto-generated if None)
    pub node_name: Option<String>,

    /// Zenoh configuration
    pub zenoh_config: zenoh::Config,

    /// Host discovery timeout (in milliseconds)
    pub discovery_timeout_ms: u64,

    /// Random jitter range for discovery timeout (0.0 - 1.0)
    pub discovery_jitter: f64,

    /// Maximum number of clients per host (None = unlimited)
    pub max_clients: Option<usize>,

    /// Whether to automatically become host if no hosts found
    pub auto_host: bool,

    /// Key expression prefix for arena communication
    pub keyexpr_prefix: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_name: None,
            zenoh_config: zenoh::Config::default(),
            discovery_timeout_ms: 5000,
            discovery_jitter: 0.3,
            max_clients: Some(4),
            auto_host: true,
            keyexpr_prefix: "zenoh/arena".to_string(),
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

    /// Set the Zenoh configuration
    pub fn with_zenoh_config(mut self, config: zenoh::Config) -> Self {
        self.zenoh_config = config;
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

    /// Set whether to automatically become host
    pub fn with_auto_host(mut self, auto_host: bool) -> Self {
        self.auto_host = auto_host;
        self
    }

    /// Set the key expression prefix
    pub fn with_keyexpr_prefix(mut self, prefix: String) -> Self {
        self.keyexpr_prefix = prefix;
        self
    }
}
