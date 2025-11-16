//! Subscriber for node data with deserialization

use crate::error::Result;
use crate::network::keyexpr::{KeyexprLink, LinkType};
use crate::node::stats::StatsTracker;
use crate::node::types::NodeId;
use std::sync::Arc;
use zenoh::key_expr::KeyExpr;

/// Subscribes to a Zenoh key expression and deserializes received data
///
/// This subscriber automatically deserializes received samples into type T.
/// Uses a glob subscription pattern: subscribes to `<prefix>/action/*/<receiver_id>`
/// to receive messages from any sender to the specified receiver.
/// The `recv()` method returns both the sender ID and the deserialized value.
pub struct NodeSubscriber<T> {
    subscriber: zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>,
    stats_tracker: Option<Arc<StatsTracker>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> std::fmt::Debug for NodeSubscriber<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeSubscriber")
            .field("type", &std::any::type_name::<T>())
            .field("key_expr", &self.subscriber.key_expr())
            .finish()
    }
}

impl<T> NodeSubscriber<T>
where
    T: zenoh_ext::Deserialize,
{
    /// Create a new subscriber for a Link keyexpr with receiver_id
    ///
    /// Declares a Zenoh subscriber for the link keyexpr pattern:
    /// `<prefix>/<link_type>/*/<receiver_id>` (sender_id=wildcard, receiver_id=node_id)
    /// to receive all messages sent to the specified receiver from any sender.
    pub async fn new(
        session: &zenoh::Session,
        prefix: impl Into<KeyExpr<'static>>,
        link_type: LinkType,
        receiver_node_id: &NodeId,
        stats_tracker: Option<Arc<StatsTracker>>,
    ) -> Result<Self> {
        // Construct Link keyexpr: <prefix>/<link_type>/*/<receiver_id> (sender_id=*, receiver_id)
        let node_keyexpr = KeyexprLink::new(prefix, link_type, None, Some(receiver_node_id.clone()));
        let keyexpr: KeyExpr = node_keyexpr.into();

        let subscriber = session
            .declare_subscriber(keyexpr)
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;

        Ok(Self {
            subscriber,
            stats_tracker,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Receive and deserialize the next value with sender information
    ///
    /// Waits for the next sample from the subscriber and:
    /// 1. Parses the sample's keyexpr as KeyexprLink to extract the sender_id
    /// 2. Deserializes the payload into type T
    ///
    /// Returns a tuple of (sender_id, value).
    /// Returns an error if reception, keyexpr parsing, or deserialization fails.
    pub async fn recv(&mut self) -> Result<(NodeId, T)> {
        let sample = self
            .subscriber
            .recv_async()
            .await
            .map_err(|e| crate::error::ArenaError::Internal(format!("Failed to receive sample: {}", e)))?;

        // Track input bytes if stats tracker is available
        if let Some(tracker) = &self.stats_tracker {
            tracker.add_input_bytes(sample.payload().len());
        }

        // Parse the keyexpr to extract sender_id (node_src)
        let keyexpr_link = KeyexprLink::try_from(sample.key_expr().clone().into_owned())?;
        let sender_id = keyexpr_link.node_src()
            .clone()
            .ok_or_else(|| crate::error::ArenaError::Internal(
                format!("Received sample with wildcard sender_id in keyexpr '{}'", sample.key_expr())
            ))?;

        // Deserialize the payload
        let value: T = zenoh_ext::z_deserialize(sample.payload())
            .map_err(|e| crate::error::ArenaError::Serialization(format!("Failed to deserialize: {}", e)))?;

        Ok((sender_id, value))
    }
}