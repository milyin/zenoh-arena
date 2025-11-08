//! Configuration for a Node

/// Main configuration for a Node
#[derive(Debug, Clone)]
pub(crate) struct NodeConfig {
    /// Optional node name (auto-generated if None)
    pub node_name: Option<String>,

    /// Whether to force host mode (blocks Searching and Client states)
    pub force_host: bool,

    /// Timeout for step() method in milliseconds
    /// step() returns when either new game state is available or this timeout elapses
    pub step_timeout_ms: u64,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_name: None,
            force_host: false,
            step_timeout_ms: 100,
        }
    }
}

impl NodeConfig {
    /// Set the node name
    pub fn with_node_name(mut self, name: String) -> Self {
        self.node_name = Some(name);
        self
    }

    /// Set whether to force host mode (blocks Searching and Client states)
    pub fn with_force_host(mut self, force_host: bool) -> Self {
        self.force_host = force_host;
        self
    }

    /// Set the step timeout in milliseconds
    pub fn with_step_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.step_timeout_ms = timeout_ms;
        self
    }
}
