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

use crate::types::NodeId;
use crate::error::Result;
use crate::network::keyexpr::HostClientKeyexpr;
use zenoh::key_expr::KeyExpr;
use zenoh::query::{Query, Queryable};

/// Request from a client for host to accept connection
///
/// Wraps a Zenoh Query with methods to accept or reject the connection request.
/// The host handler calls either `accept()` or `reject()` to respond to the client.
#[derive(Debug, Clone)]
pub struct NodeRequest {
    query: Query,
    client_id: NodeId,
}

impl NodeRequest {
    /// Create a new NodeRequest from a Query and client_id
    /// 
    /// # Panics
    /// 
    /// Panics if query keyexpr is not HostClientKeyexpr(Some, Some) with matching client_id.
    pub fn new(query: Query, client_id: NodeId) -> Self {
        let parsed = HostClientKeyexpr::try_from(query.key_expr().clone())
            .expect("Invalid HostClientKeyexpr");
        assert!(parsed.host_id().is_some(), "Glob host_id");
        assert_eq!(
            parsed.client_id().as_ref().expect("Glob client_id"),
            &client_id,
            "Client ID mismatch"
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
}

/// Wrapper for host discovery and connection requests
///
/// Holds a queryable declared on `<prefix>/host/<host_id>/*` to respond to:
/// - Discovery queries: `<prefix>/host/*/<client_id>` (glob on host_id)
///   → Replies immediately with ok (presence confirmation)
/// - Connection queries: `<prefix>/host/<host_id>/<client_id>` (specific both)
///   → Returns NodeRequest for host to accept/reject
#[derive(Debug)]
pub struct NodeQueryable {
    /// The zenoh queryable that receives queries
    queryable: Queryable<zenoh::handlers::FifoChannelHandler<Query>>,
    /// Node ID for formatting replies
    node_id: NodeId,
    /// Prefix for formatting replies
    prefix: KeyExpr<'static>,
}

impl NodeQueryable {
    /// Declare a new queryable for a host node
    ///
    /// Declares queryable on `<prefix>/host/<host_id>/*` pattern.
    pub async fn declare(
        session: &zenoh::Session,
        prefix: &KeyExpr<'_>,
        node_id: NodeId,
    ) -> Result<Self> {
        // Declare on pattern: <prefix>/host/<host_id>/*
        let host_client_keyexpr = HostClientKeyexpr::new(prefix, Some(node_id.clone()), None);
        let keyexpr: KeyExpr = host_client_keyexpr.into();
        
        // Declare queryable without callback
        let queryable = session
            .declare_queryable(&keyexpr)
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;

        Ok(Self {
            queryable,
            node_id,
            prefix: prefix.clone().into_owned(),
        })
    }

    /// Wait for and retrieve the next connection request
    ///
    /// Loops receiving queries from the queryable. For each query:
    /// - If it's a discovery query (glob client_id): replies with ok
    /// - If it's a connection query (specific client_id): returns NodeRequest
    pub async fn expect_connection(&self) -> Result<NodeRequest> {
        loop {
            // Receive next query from queryable
            let query = self.queryable
                .recv_async()
                .await
                .map_err(|_| crate::error::ArenaError::Internal(
                    "Queryable channel closed".to_string(),
                ))?;
            
            // Parse the incoming query keyexpr to determine if it's discovery or connection
            let query_keyexpr = query.key_expr().clone();
            
            // Try to parse as HostClientKeyexpr to extract client_id
            match HostClientKeyexpr::try_from(query_keyexpr.clone()) {
                Ok(parsed_keyexpr) => {
                    match parsed_keyexpr.client_id() {
                        Some(client_id) => {
                            // Connection request (specific client_id): return it
                            return Ok(NodeRequest::new(query, client_id.clone()));
                        }
                        None => {
                            // Discovery request (glob client_id): reply ok immediately
                            // This just confirms host presence for discovery phase
                            let reply_host_client = HostClientKeyexpr::new(
                                &self.prefix,
                                Some(self.node_id.clone()),
                                None, // glob on client_id
                            );
                            let reply_keyexpr: KeyExpr = reply_host_client.into();
                            
                            if let Err(e) = query.reply(&reply_keyexpr, "").await {
                                tracing::debug!("Failed to reply to discovery query: {}", e);
                            }
                            // Continue loop to wait for connection request
                        }
                    }
                }
                Err(_) => {
                    // Failed to parse keyexpr, ignore and continue
                    tracing::debug!(
                        "Failed to parse query keyexpr: {}",
                        query_keyexpr.as_str()
                    );
                }
            }
        }
    }
}
