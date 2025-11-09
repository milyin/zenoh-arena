//! Querier for discovering available hosts

use zenoh::key_expr::KeyExpr;
use zenoh::query::Reply;
use zenoh::handlers::FifoChannelHandler;

/// Wrapper around Zenoh's Querier for discovering available hosts
///
/// The querier is used to find active nodes via normal query requests.
/// A corresponding queryable will be declared by hosts to respond to these queries.
#[derive(Debug)]
pub struct NodeQuerier {
    receiver: FifoChannelHandler<Reply>,
}

impl NodeQuerier {
    /// Declare a new querier for discovering hosts
    pub async fn declare(
        session: &zenoh::Session,
        prefix: &KeyExpr<'_>,
    ) -> Result<Self, zenoh::Error> {
        let keyexpr = crate::network::keyexpr::query_nodes_keyexpr(prefix);
        
        // Use normal get query to find active nodes
        // Hosts will need to declare a queryable to respond to this
        let receiver = session
            .get(keyexpr)
            .await?;

        Ok(Self { receiver })
    }

    /// Get a reference to the receiver to await responses
    pub fn receiver(&self) -> &FifoChannelHandler<Reply> {
        &self.receiver
    }
}
