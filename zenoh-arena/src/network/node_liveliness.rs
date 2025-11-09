//! Liveliness token management

use crate::error::Result;
use crate::types::NodeId;
use crate::network::keyexpr::{NodeKeyexpr, Role};
use zenoh::key_expr::KeyExpr;
use zenoh::liveliness::LivelinessToken;
use zenoh::sample::SampleKind;

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
        prefix: impl Into<KeyExpr<'static>>,
        role: Role,
        node_id: NodeId,
    ) -> Result<Self> {
        let node_keyexpr = NodeKeyexpr::new(prefix, role, Some(node_id.clone()), None);
        let keyexpr: KeyExpr = node_keyexpr.into();
        let token = session
            .liveliness()
            .declare_token(keyexpr)
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;

        Ok(Self { token, node_id })
    }
}

/// Watches for liveliness of a specific host node
///
/// Subscribes to liveliness events for a host and detects when the host goes offline.
/// Provides a `disconnected()` method that waits for the liveliness token to be dropped
/// (signaling the host is no longer active).
///
/// This is used by clients to detect when their connected host goes offline and needs
/// to return to the host search stage.
pub struct NodeLivelinessWatch {
    subscriber: zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>,
    host_id: NodeId,
}

impl std::fmt::Debug for NodeLivelinessWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeLivelinessWatch")
            .field("host_id", &self.host_id)
            .finish()
    }
}

impl NodeLivelinessWatch {
    /// Subscribe to liveliness events for a node
    ///
    /// Creates a liveliness subscriber that tracks the presence of the specified node.
    /// The subscriber will receive events when the node's liveliness token is declared or undeclared.
    pub async fn subscribe(
        session: &zenoh::Session,
        prefix: impl Into<KeyExpr<'static>>,
        role: Role,
        node_id: NodeId,
    ) -> Result<Self> {
        let node_keyexpr = NodeKeyexpr::new(prefix, role, Some(node_id.clone()), None);
        let keyexpr: KeyExpr = node_keyexpr.into();
        
        let subscriber = session
            .liveliness()
            .declare_subscriber(keyexpr)
            .history(true)
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;
        
        Ok(Self {
            subscriber,
            host_id: node_id,
        })
    }

    /// Wait for the host to disconnect (liveliness lost)
    ///
    /// Continuously receives liveliness events. When a "Delete" event is received,
    /// it indicates the host's liveliness token has been dropped and the host is
    /// no longer available. This method returns when the host disconnects.
    ///
    /// Similar to `NodeQueryable::expect_connection()`, this loops until the
    /// expected event (liveliness delete) is detected.
    pub async fn disconnected(&mut self) -> Result<()> {
        loop {
            match self.subscriber.recv_async().await {
                Ok(sample) => {
                    match sample.kind() {
                        SampleKind::Put => {
                            // Host came online or re-established liveliness, continue waiting
                            tracing::debug!(
                                "Host '{}' liveliness put event",
                                self.host_id
                            );
                        }
                        SampleKind::Delete => {
                            // Host went offline, liveliness lost
                            tracing::info!(
                                "Host '{}' liveliness lost - disconnecting",
                                self.host_id
                            );
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Liveliness subscription error for host '{}': {}",
                        self.host_id,
                        e
                    );
                    // Subscription error is treated as disconnect
                    return Err(crate::error::ArenaError::Zenoh(e));
                }
            }
        }
    }

    /// Get the host ID being watched
    #[allow(dead_code)]
    pub fn host_id(&self) -> &NodeId {
        &self.host_id
    }
}
