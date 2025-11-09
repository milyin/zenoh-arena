//! Liveliness token management

use crate::error::Result;
use crate::network::keyexpr::{NodeKeyexpr, Role};
use crate::types::NodeId;
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
    ///
    /// Before creating the token, performs a liveliness.get() request to check if
    /// another token with the same keyexpr already exists in the network.
    /// If a conflict is detected, returns a LivelinessTokenConflict error.
    pub async fn declare(
        session: &zenoh::Session,
        prefix: impl Into<KeyExpr<'static>>,
        role: Role,
        node_id: NodeId,
    ) -> Result<Self> {
        let node_keyexpr = NodeKeyexpr::new(prefix, role, Some(node_id.clone()), None);
        let keyexpr: KeyExpr = node_keyexpr.into();

        // Check if another token with the same keyexpr already exists
        let replies = session
            .liveliness()
            .get(keyexpr.clone())
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;

        // If we receive any liveliness tokens, it means another token already exists
        if (replies.recv_async().await).is_ok() {
            return Err(crate::error::ArenaError::LivelinessTokenConflict(format!(
                "Another liveliness token already exists for keyexpr: {}",
                keyexpr
            )));
        }

        // No existing token found, declare the new one
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
/// Provides a `subscribe()` method to add subscribers and an `unsubscribe()` method to remove them.
/// The `disconnected()` method waits for any subscriber's disconnect using `select_all`.
///
/// This is used by clients to detect when their connected host goes offline and needs
/// to return to the host search stage.
pub struct NodeLivelinessWatch {
    subscribers: Vec<
        zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>,
    >,
    host_id: NodeId,
}

impl std::fmt::Debug for NodeLivelinessWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeLivelinessWatch")
            .field("host_id", &self.host_id)
            .field("num_subscribers", &self.subscribers.len())
            .finish()
    }
}

impl NodeLivelinessWatch {
    /// Create a new liveliness watch for a node without any subscribers
    pub fn new(node_id: NodeId) -> Self {
        Self {
            subscribers: Vec::new(),
            host_id: node_id,
        }
    }

    /// Subscribe to liveliness events for a node
    ///
    /// Adds a new liveliness subscriber that tracks the presence of the specified node.
    /// The subscriber will receive events when the node's liveliness token is declared or undeclared.
    /// Multiple subscribers can be added via repeated calls to this method.
    pub async fn subscribe(
        &mut self,
        session: &zenoh::Session,
        prefix: impl Into<KeyExpr<'static>>,
        role: Role,
        node_id: &NodeId,
    ) -> Result<()> {
        let node_keyexpr = NodeKeyexpr::new(prefix, role, Some(node_id.clone()), None);
        let keyexpr: KeyExpr = node_keyexpr.into();

        let subscriber = session
            .liveliness()
            .declare_subscriber(keyexpr)
            .history(true)
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;

        self.subscribers.push(subscriber);
        Ok(())
    }

    /// Unsubscribe from liveliness events
    ///
    /// Removes the most recently added subscriber from the watch.
    /// Returns `true` if a subscriber was removed, `false` if there are no subscribers.
    #[allow(dead_code)]
    pub fn unsubscribe(&mut self) -> bool {
        self.subscribers.pop().is_some()
    }

    /// Wait for any subscriber to disconnect (liveliness lost)
    ///
    /// Uses `select_all` to wait for any of the subscribers to receive a "Delete" event,
    /// indicating the host's liveliness token has been dropped and the host is
    /// no longer available. This method returns when any subscriber detects disconnection.
    ///
    /// Returns the node ID that disconnected.
    pub async fn disconnected(&mut self) -> Result<NodeId> {
        if self.subscribers.is_empty() {
            return Err(crate::error::ArenaError::LivelinessError(
                "No subscribers registered for liveliness watch".into(),
            ));
        }

        // Process all subscribers and wait for any to disconnect
        loop {
            // Check each subscriber for disconnect event
            for subscriber in self.subscribers.iter_mut() {
                match subscriber.try_recv() {
                    Ok(Some(sample)) => {
                        match sample.kind() {
                            SampleKind::Delete => {
                                // Host went offline, liveliness lost
                                tracing::info!(
                                    "Host '{}' liveliness lost - disconnecting",
                                    self.host_id
                                );
                                return Ok(self.host_id.clone());
                            }
                            SampleKind::Put => {
                                // Host came online or re-established liveliness, continue waiting
                                tracing::debug!(
                                    "Host '{}' liveliness put event",
                                    self.host_id
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        // No message available (non-blocking)
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Liveliness subscription error for host '{}': {}",
                            self.host_id,
                            e
                        );
                        // Subscription error is treated as disconnect
                        return Ok(self.host_id.clone());
                    }
                }
            }

            // Wait a bit before checking again to avoid busy-spinning
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    /// Get the host ID being watched
    #[allow(dead_code)]
    pub fn host_id(&self) -> &NodeId {
        &self.host_id
    }
}
