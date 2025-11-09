//! Queryable for host discovery and connection acceptance
//!
//! ## Protocol Overview
//!
//! The host declares a SINGLE queryable on `<prefix>/host/<host_id>/*` pattern.
//! This queryable responds to both:
//!
//! 1. **Discovery Phase**: Glob queries from clients
//!    - Client query: `<prefix>/host/*/<client_id>` (glob host_id)
//!    - Matches queryable pattern: `<prefix>/host/<host_id>/*`
//!    - Queryable callback replies with ok to confirm presence (discovery phase detected)
//!
//! 2. **Connection Phase**: Specific connection requests
//!    - Client query: `<prefix>/host/<host_id>/<client_id>` (both specific)
//!    - Matches same queryable pattern: `<prefix>/host/<host_id>/*`
//!    - Queryable callback pushes NodeRequest to channel for host handler (connection phase detected)
//!    - Host calls accept() or reject() on the request
//!
//! ## Request Detection
//!
//! The callback distinguishes phases by checking the incoming query keyexpr:
//! - If it matches `<host_id>/*` pattern with specific client_id → Connection request (pushed to channel)
//! - If it matches `<host_id>/*` pattern with glob client_id → Discovery request (replied immediately)

use crate::error::Result;
use crate::network::keyexpr::HostClientKeyexpr;
use crate::types::NodeId;
use zenoh::key_expr::KeyExpr;
use zenoh::query::{Query, Queryable};

/// Request from a client for host to accept connection
///
/// Wraps a Zenoh Query with methods to accept or reject the connection request.
/// The host handler calls either `accept()` or `reject()` to respond to the client.
#[derive(Debug, Clone)]
pub struct HostRequest {
    query: Query,
    client_id: NodeId,
}

impl HostRequest {
    /// Create a new NodeRequest from a Query and client_id
    ///
    /// # Panics
    ///
    /// Panics if query keyexpr is not HostClientKeyexpr(Some, Some) with matching client_id.
    pub fn new(query: Query, client_id: NodeId) -> Self {
        let parsed = HostClientKeyexpr::try_from(query.key_expr().clone())
            .expect("Invalid HostClientKeyexpr");
        assert!(
            parsed.host_id().is_some(),
            "Expected specific host_id in query keyexpr: {}",
            query.key_expr().as_str()
        );
        assert_eq!(
            parsed.client_id().as_ref().unwrap_or_else(|| panic!(
                "Expected specific client_id in query keyexpr: {}",
                query.key_expr().as_str()
            )),
            &client_id,
            "Client ID mismatch: expected '{}', found '{}'",
            client_id,
            parsed.client_id().as_ref().unwrap()
        );

        Self { query, client_id }
    }

    /// Accept the connection request
    ///
    /// Sends a positive reply (ok) with empty payload to the querying client.
    /// This confirms that the host accepts this client's connection.
    ///
    /// Returns the client ID.
    pub async fn accept(self) -> Result<NodeId> {
        let keyexpr = self.query.key_expr();

        // Reply to the same keyexpr from the query. This is safe because NodeRequest
        // is only created for connection requests with specific client_id (no globs).
        self.query
            .reply(keyexpr, "")
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;

        Ok(self.client_id)
    }

    /// Reject the connection request
    ///
    /// Sends an error reply to the querying client.
    /// This tells the client this host cannot accept the connection.
    pub async fn reject(self, reason: &str) -> Result<()> {
        self.query
            .reply_err(reason)
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;

        Ok(())
    }

    /// Get the client ID
    pub fn client_id(&self) -> &NodeId {
        &self.client_id
    }
}

/// Wrapper for host discovery and connection requests
///
/// Holds a queryable declared on `<prefix>/host/<host_id>/*` to respond to:
/// - Discovery queries: `<prefix>/host/*/<client_id>` (glob on host_id)
///   → Replies immediately with ok (presence confirmation)
/// - Connection queries: `<prefix>/host/<host_id>/<client_id>` (specific both)
///   → Returns NodeRequest for host to accept/reject
#[derive(Debug)]
pub struct HostQueryable {
    /// The zenoh queryable that receives queries
    queryable: Queryable<zenoh::handlers::FifoChannelHandler<Query>>,
    /// Node ID for formatting replies
    node_id: NodeId,
    /// Prefix for formatting replies
    prefix: KeyExpr<'static>,
}

impl HostQueryable {
    /// Declare a new queryable for a host node
    ///
    /// Declares queryable on `<prefix>/host/<host_id>/*` pattern.
    pub async fn declare(
        session: &zenoh::Session,
        prefix: impl Into<KeyExpr<'static>>,
        node_id: NodeId,
    ) -> Result<Self> {
        let prefix = prefix.into();
        // Declare on pattern: <prefix>/host/<host_id>/*
        let host_client_keyexpr = HostClientKeyexpr::new(prefix.clone(), Some(node_id.clone()), None);
        let keyexpr: KeyExpr = host_client_keyexpr.into();

        // Declare queryable without callback
        let queryable = session
            .declare_queryable(&keyexpr)
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;

        Ok(Self {
            queryable,
            node_id,
            prefix,
        })
    }

    /// Wait for and retrieve the next connection request
    ///
    /// Loops receiving queries from the queryable. For each query:
    /// - If it's a discovery query (glob client_id): replies with ok
    /// - If it's a connection query (specific client_id): returns NodeRequest
    pub async fn expect_connection(&self) -> Result<HostRequest> {
        loop {
            // Receive next query from queryable
            let query = self.queryable.recv_async().await.map_err(|_| {
                crate::error::ArenaError::Internal("Queryable channel closed".to_string())
            })?;

            // Parse the incoming query keyexpr to determine if it's discovery or connection
            let query_keyexpr = query.key_expr().clone();

            // Try to parse as HostClientKeyexpr to extract host_id and client_id
            match HostClientKeyexpr::try_from(query_keyexpr.clone()) {
                Ok(parsed) => {
                    match (parsed.host_id(), parsed.client_id()) {
                        (Some(host_id), Some(client_id)) => {
                            assert_eq!(
                                host_id,
                                &self.node_id,
                                "Host ID mismatch: expected '{}', found '{}'",
                                self.node_id,
                                host_id
                            );
                            // Connection request (specific host id and client_id): return it
                            return Ok(HostRequest::new(query, client_id.clone()));
                        }
                        (Some(host_id), None) => {
                            // ignore invalid case: specific host_id but glob client_id
                            tracing::debug!(
                                "Invalid query with specific host_id '{}' but glob client_id: {}",
                                host_id,
                                query_keyexpr.as_str()
                            );
                        }
                        (None, Some(client_id)) => {
                            // request from specific client_id but glob host_id - corect discovery case
                            // Trace and reply with ok, confirming presence
                            tracing::debug!(
                                "Discovery request from client_id '{}' with glob host_id: {}",
                                client_id,
                                query_keyexpr.as_str()
                            );
                            let reply_host_client = HostClientKeyexpr::new(
                                self.prefix.clone(),
                                Some(self.node_id.clone()),
                                Some(client_id.clone()),
                            );
                            let reply_keyexpr: KeyExpr = reply_host_client.into();
                            if let Err(e) = query.reply(&reply_keyexpr, "").await {
                                tracing::debug!("Failed to reply to discovery query: {}", e);
                            }
                        }
                        (None, None) => {
                            // ignore invalid case: glob host_id and glob client_id
                            tracing::debug!(
                                "Invalid query with glob host_id and glob client_id: {}",
                                query_keyexpr.as_str()
                            );
                        }
                    }
                }
                Err(_) => {
                    // Failed to parse keyexpr, ignore and continue
                    tracing::debug!("Failed to parse query keyexpr: {}", query_keyexpr.as_str());
                }
            }
        }
    }
}
