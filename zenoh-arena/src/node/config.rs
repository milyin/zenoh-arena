//! Configuration for a Node

use zenoh::key_expr::KeyExpr;

use crate::node::types::NodeId;

// Main configuration for a Node
#[derive(Debug, Clone)]
pub(crate) struct NodeConfig {
    /// Node identifier
    pub node_id: NodeId,

    /// Whether to force host mode (blocks Searching and Client states)
    pub force_host: bool,

    /// Timeout for step() method in milliseconds
    /// step() returns when either new game state is available or this timeout elapses
    pub step_timeout_ms: u64,

    /// Timeout for host search in milliseconds
    /// When in SearchingHost state, if no hosts are found within this timeout,
    /// the node transitions to Host state
    pub search_timeout_ms: u64,

    /// Key expression prefix for all arena operations
    pub keyexpr_prefix: KeyExpr<'static>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_id: NodeId::generate(),
            force_host: false,
            step_timeout_ms: 100,
            search_timeout_ms: 3000, // 3 seconds to search for hosts
            keyexpr_prefix: KeyExpr::try_from("zenoh/arena").unwrap().into_owned(),
        }
    }
}
