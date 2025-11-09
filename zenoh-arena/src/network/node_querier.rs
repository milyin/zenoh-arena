//! Querier for connecting to available hosts
//!
//! ## Connection Protocol
//!
//! The connection process consists of two phases:
//!
//! ### Phase 1: Host Discovery
//! - Client sends query to `<prefix>/host/*` keyexpr
//! - All active hosts respond via queryables on this pattern
//! - Client collects all available host IDs from responses
//!
//! ### Phase 2: Connection Attempt
//! - For each discovered host, client sends query to `<prefix>/host/<host_id>` keyexpr
//! - Host responds via queryable on its specific keyexpr
//! - If response is Ok, connection is established with that host
//! - If no host responds positively, client returns None (will become host itself)
//!
//! ## Queryable Implementation (Future)
//!
//! Hosts will declare queryables to respond to these queries:
//! - Queryable on `<prefix>/host/<host_id>` - responds to connection attempts
//! - When receiving a query, host checks if it's accepting clients (has capacity)
//! - If accepting: sends positive response (host info)
//! - If at capacity: doesn't respond or sends rejection
//!
//! This allows clients to discover available hosts and establish connections.

use crate::types::NodeId;
use crate::error::Result;
use crate::network::keyexpr::{HostLookupKeyexpr, HostClientKeyexpr, HostKeyexpr};
use zenoh::key_expr::KeyExpr;

/// Helper for connecting to available hosts
///
/// Implements the two-phase connection protocol:
/// 1. Discover all available hosts
/// 2. Attempt connection to each host until one succeeds
#[derive(Debug)]
pub struct NodeQuerier;

impl NodeQuerier {
    /// Connect to an available host
    ///
    /// Performs two-phase discovery:
    /// 1. Queries `<prefix>/host/*` to discover all hosts
    /// 2. Queries each host at `<prefix>/host/<host_id>/<client_id>` for connection
    ///
    /// Returns:
    /// - `Ok(Some(host_id))` - Successfully connected to a host
    /// - `Ok(None)` - No hosts available, client should become host
    /// - `Err(_)` - Zenoh query error
    pub async fn connect(
        session: &zenoh::Session,
        prefix: &KeyExpr<'_>,
        client_id: NodeId,
    ) -> Result<Option<NodeId>> {
        tracing::debug!("Discovering available hosts...");

        // Phase 1: Discover all available hosts
        let discover_keyexpr = HostLookupKeyexpr::new(prefix);
        let discover_keyexpr: KeyExpr = discover_keyexpr.into();
        let discovery_replies = session.get(discover_keyexpr).await?;

        let mut host_ids: Vec<NodeId> = Vec::new();

        // Collect all host IDs from discovery responses
        while let Ok(reply) = discovery_replies.recv_async().await {
            // Parse the reply to extract host_id from key expression
            match reply.result() {
                Ok(sample) => {
                    let keyexpr = sample.key_expr().clone();
                    match HostKeyexpr::try_from(keyexpr.clone()) {
                        Ok(host_keyexpr) => {
                            let host_id = host_keyexpr.host_id().clone();
                            tracing::debug!("Discovered host: {}", host_id);
                            host_ids.push(host_id);
                        }
                        Err(e) => {
                            tracing::debug!("Failed to parse host_id from keyexpr {}: {}", keyexpr.as_str(), e);
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

        tracing::info!("Discovered {} host(s), attempting connections", host_ids.len());

        // Phase 2: Try connecting to each discovered host
        for host_id in host_ids {
            let connect_keyexpr = HostClientKeyexpr::new(prefix, host_id.clone(), client_id.clone());
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
