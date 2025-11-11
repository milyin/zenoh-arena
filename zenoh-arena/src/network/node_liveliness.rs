//! Liveliness token management

use crate::error::Result;
use crate::network::keyexpr::KeyexprNode;
use crate::node::types::NodeId;
use futures::future::select_all;
use std::pin::Pin;
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
        keyexpr: KeyexprNode,
    ) -> Result<Self> {
        let node_id = keyexpr.node().clone().expect("node_id must be specified for liveliness token");
        let keyexpr: KeyExpr = keyexpr.into();

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

/// Watches for liveliness of nodes matching a keyexpr pattern
///
/// Subscribes to liveliness events and detects when nodes go offline.
/// Provides a `subscribe()` method to add subscribers and an `unsubscribe()` method to remove them.
/// The `disconnected()` method waits for any subscriber's disconnect using `select_all`.
///
/// This is used by:
/// - Clients to detect when their connected host goes offline (specific node_id)
/// - Hosts to detect when any client disconnects (wildcard pattern)
#[derive(Debug)]
pub struct NodeLivelinessWatch {
    subscribers: Vec<zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>>,
}

impl NodeLivelinessWatch {
    /// Create a new liveliness watch without any subscribers
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Subscribe to liveliness events for nodes matching the keyexpr
    ///
    /// Adds a new liveliness subscriber that tracks the presence of nodes matching the specified keyexpr.
    /// The subscriber will receive events when matching nodes' liveliness tokens are declared or undeclared.
    /// Multiple subscribers can be added via repeated calls to this method.
    ///
    /// The keyexpr can be:
    /// - Specific: with node() returning Some(id) to track a single node
    /// - Wildcard: with node() returning None to track all nodes matching the pattern
    pub async fn subscribe(
        &mut self,
        session: &zenoh::Session,
        keyexpr: KeyexprNode,
    ) -> Result<()>
    {
        let keyexpr: KeyExpr = keyexpr.into();

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
    /// indicating a node's liveliness token has been dropped and the node is
    /// no longer available. This method returns when any subscriber detects disconnection.
    ///
    /// The node ID is extracted by parsing the sample's keyexpr as KeyexprNode.
    ///
    /// Returns the node ID that disconnected.
    pub async fn disconnected(&mut self) -> Result<NodeId>
    {
        if self.subscribers.is_empty() {
            return Err(crate::error::ArenaError::LivelinessError(
                "No subscribers registered for liveliness watch".into(),
            ));
        }

        // Create a future for each subscriber
        let futures_vec: Vec<_> = self
            .subscribers
            .iter_mut()
            .map(|subscriber| {
                Box::pin(async move {
                    loop {
                        match subscriber.recv_async().await {
                            Ok(sample) => {
                                // Extract node_id from the sample's keyexpr by parsing it as KeyexprNode
                                let keyexpr_node = match KeyexprNode::try_from(sample.key_expr().clone().into_owned()) {
                                    Ok(k) => k,
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to parse keyexpr '{}': {}",
                                            sample.key_expr(),
                                            e
                                        );
                                        continue;
                                    }
                                };

                                let node_id = match keyexpr_node.node() {
                                    Some(id) => id.clone(),
                                    None => {
                                        tracing::warn!(
                                            "Received sample with wildcard node in keyexpr '{}'",
                                            sample.key_expr()
                                        );
                                        continue;
                                    }
                                };

                                match sample.kind() {
                                    SampleKind::Delete => {
                                        // Node went offline, liveliness lost
                                        tracing::info!(
                                            "Node '{}' liveliness lost - disconnecting",
                                            node_id
                                        );
                                        return Ok(node_id);
                                    }
                                    SampleKind::Put => {
                                        // Node came online or re-established liveliness, continue waiting
                                        tracing::debug!(
                                            "Node '{}' liveliness put event",
                                            node_id
                                        );
                                        // Continue waiting for Delete event
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Liveliness subscription error: {}",
                                    e
                                );
                                // Subscription error - we cannot extract node_id without a sample
                                // Continue to next iteration
                                continue;
                            }
                        }
                    }
                }) as Pin<Box<dyn std::future::Future<Output = Result<NodeId>> + Send>>
            })
            .collect();

        // Wait for any future to complete
        let (result, _index, _remaining) = select_all(futures_vec).await;
        result
    }

    /// Check if there are any subscribers registered
    pub fn has_subscribers(&self) -> bool {
        !self.subscribers.is_empty()
    }
}

