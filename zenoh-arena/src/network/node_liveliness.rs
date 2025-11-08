//! Liveliness token management

use crate::types::NodeId;
use zenoh::key_expr::KeyExpr;
use zenoh::liveliness::LivelinessToken;

/// Wrapper around Zenoh's LivelinessToken for a node
///
/// The token is automatically undeclared when dropped.
#[derive(Debug)]
pub struct NodeLivelinessToken {
    #[allow(dead_code)]
    token: LivelinessToken,
    #[allow(dead_code)]
    node_id: NodeId,
}

impl NodeLivelinessToken {
    /// Declare a new liveliness token for a node
    pub async fn declare(
        session: &zenoh::Session,
        prefix: &KeyExpr<'_>,
        node_id: NodeId,
    ) -> Result<Self, zenoh::Error> {
        let keyexpr = crate::network::keyexpr::node_keyexpr(prefix, &node_id);
        let token = session
            .liveliness()
            .declare_token(keyexpr)
            .await?;

        Ok(Self { token, node_id })
    }
}
