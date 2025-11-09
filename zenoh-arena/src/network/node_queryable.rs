//! Queryable for host discovery

use crate::types::NodeId;
use zenoh::key_expr::KeyExpr;
use zenoh::query::{Queryable, Query};
use zenoh::handlers::FifoChannelHandler;

/// Wrapper around Zenoh's Queryable for host discovery
///
/// The queryable is declared on the host's keyexpr to allow clients
/// to discover and connect to this host via queries.
#[derive(Debug)]
pub struct NodeQueryable {
    queryable: Queryable<FifoChannelHandler<Query>>,
    #[allow(dead_code)]
    node_id: NodeId,
}

impl NodeQueryable {
    /// Declare a new queryable for a host node
    pub async fn declare(
        session: &zenoh::Session,
        prefix: &KeyExpr<'_>,
        node_id: NodeId,
    ) -> Result<Self, zenoh::Error> {
        let keyexpr = crate::network::keyexpr::node_keyexpr(prefix, &node_id);
        
        // Declare a queryable that will respond to queries on the host keyexpr
        let queryable = session
            .declare_queryable(keyexpr)
            .await?;

        Ok(Self { queryable, node_id })
    }

    /// Get a receiver for incoming queries
    pub fn receiver(&self) -> &FifoChannelHandler<Query> {
        self.queryable.handler()
    }
}
