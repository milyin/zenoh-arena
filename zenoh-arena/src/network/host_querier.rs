//! Querier for connecting to available hosts
//!
//! ## Connection Protocol
//!
//! The connection process consists of two phases:
//!
//! ### Phase 1: Host Discovery
//! - Client sends query to `<prefix>/host/*/client_id` keyexpr (glob on host_id)
//! - All active hosts respond via their single queryable on `<prefix>/host/<host_id>/*`
//! - This acts as a presence confirmation: "I am a host, here's my ID"
//! - Client collects all available host IDs from responses
//!
//! ### Phase 2: Connection Attempt
//! - For each discovered host, client sends query to `<prefix>/host/<host_id>/<client_id>` keyexpr
//! - Host's same queryable on `<prefix>/host/<host_id>/*` responds to this specific keyexpr
//! - This acts as a connection confirmation: "I accept your specific connection request"
//! - If response is Ok, connection is established with that host
//! - If no host responds positively, client returns None (will become host itself)
//!
//! ## Queryable Implementation (Future)
//!
//! Hosts will declare a SINGLE queryable on `<prefix>/host/<host_id>/*` that responds to:
//!
//! 1. **Glob queries**: `<prefix>/host/<host_id>/*`
//!    - Matches incoming glob discovery queries: `<prefix>/host/<host_id>/<client_id>`
//!    - Keyexpr pattern: client_id is a wildcard
//!    - Response: Confirms host presence (for discovery phase)
//!
//! 2. **Specific queries**: `<prefix>/host/<host_id>/<specific_client_id>`
//!    - Matches incoming specific connection queries: `<prefix>/host/<host_id>/<specific_client_id>`
//!    - Keyexpr pattern: client_id matches exactly
//!    - Response: Confirms connection acceptance (for connection phase)
//!
//! The queryable can distinguish between phases by checking the incoming query keyexpr:
//! - If client_id is None/wildcard in the query → discovery phase (return basic host info)
//! - If client_id is specific in the query → connection phase (check capacity and accept/reject)
//!
//! This design allows a single queryable to handle both phases efficiently.

use crate::error::Result;
use crate::network::keyexpr::{NodeKeyexpr, Role};
use crate::types::NodeId;
use zenoh::key_expr::KeyExpr;

/// Helper for connecting to available hosts
///
/// Implements the two-phase connection protocol:
/// 1. Discover all available hosts
/// 2. Attempt connection to each host until one succeeds
#[derive(Debug)]
pub struct HostQuerier;

impl HostQuerier {
    /// Connect to an available host
    ///
    /// Performs two-phase discovery and connection:
    ///
    /// **Phase 1: Host Discovery**
    /// - Queries `<prefix>/host/*/<client_id>` (glob on host_id, specific client_id)
    /// - All available hosts respond to this glob pattern
    /// - Collects all discovered host IDs
    ///
    /// **Phase 2: Connection Establishment**
    /// - For each discovered host, queries `<prefix>/host/<host_id>/<client_id>`
    /// - Host confirms it accepts this specific connection request
    /// - Returns the ID of the first host that accepts the connection
    ///
    /// Returns:
    /// - `Ok(Some(host_id))` - Successfully connected to a host
    /// - `Ok(None)` - No hosts available, client should become host
    /// - `Err(_)` - Zenoh query error
    pub async fn connect(
        session: &zenoh::Session,
        prefix: impl Into<KeyExpr<'static>>,
        client_id: NodeId,
    ) -> Result<Option<NodeId>> {
        tracing::debug!("Discovering available hosts...");

        let prefix = prefix.into();

        // Phase 1: Discover all available hosts
        // Query: <prefix>/link/*/client_id (glob on own_id, specific remote_id)
        // This queries all hosts in the arena, asking them to confirm presence
        let discover_keyexpr =
            NodeKeyexpr::new(prefix.clone(), Role::Link, None, Some(client_id.clone()));
        let discover_keyexpr: KeyExpr = discover_keyexpr.into();
        let discovery_replies = session.get(discover_keyexpr).await?;

        let mut host_ids: Vec<NodeId> = Vec::new();

        // Collect all host IDs from discovery responses
        while let Ok(reply) = discovery_replies.recv_async().await {
            // Parse the reply to extract host_id from key expression
            match reply.result() {
                Ok(sample) => {
                    let keyexpr = sample.key_expr().clone();
                    match NodeKeyexpr::try_from(keyexpr.clone()) {
                        Ok(parsed) => {
                            if let Some(host_id) = parsed.own_id() {
                                tracing::debug!("Discovered host: {}", host_id);
                                host_ids.push(host_id.clone());
                            }
                        }
                        Err(e) => {
                            tracing::debug!("Failed to parse keyexpr {}: {}", keyexpr.as_str(), e);
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Discovery reply error: {}", e);
                }
            }
        }

        if host_ids.is_empty() {
            tracing::info!("No hosts discovered");
            return Ok(None);
        }

        tracing::info!(
            "Discovered {} host(s), attempting connections",
            host_ids.len()
        );

        // Phase 2: Try connecting to each discovered host
        // Query: <prefix>/link/<host_id>/<client_id> (specific own_id and remote_id)
        // This requests the specific host to confirm it accepts this client's connection
        for host_id in host_ids {
            let connect_keyexpr = NodeKeyexpr::new(
                prefix.clone(),
                Role::Link,
                Some(host_id.clone()),
                Some(client_id.clone()),
            );
            let connect_keyexpr: KeyExpr = connect_keyexpr.into();

            match session.get(connect_keyexpr).await {
                Ok(connection_replies) => {
                    // Try to receive a positive response
                    match connection_replies.recv_async().await {
                        Ok(_reply) => {
                            // Positive response received, connection established
                            tracing::info!("Successfully connected to host: {}", host_id);
                            return Ok(Some(host_id));
                        }
                        Err(_) => {
                            // Host rejected or no response
                            tracing::debug!("No response from host {}", host_id);
                            continue;
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Connection query to host {} failed: {}", host_id, e);
                    continue;
                }
            }
        }

        tracing::info!("No host accepted connection");
        Ok(None)
    }
}
