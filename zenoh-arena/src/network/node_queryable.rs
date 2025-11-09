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
use zenoh::query::{Queryable, Query};

/// Request from a client for host to accept connection
///
/// Wraps a Zenoh Query with methods to accept or reject the connection request.
/// The host handler calls either `accept()` or `reject()` to respond to the client.
#[derive(Debug, Clone)]
pub struct NodeRequest {
    query: Query,
}

impl NodeRequest {
    /// Create a new NodeRequest from a Query
    pub fn new(query: Query) -> Self {
        Self { query }
    }

    /// Accept the connection request
    ///
    /// Sends a positive reply (ok) with empty payload to the querying client.
    /// This confirms that the host accepts this client's connection.
    ///
    /// Returns the client ID extracted from the query keyexpr.
    pub async fn accept(self) -> Result<NodeId> {
        let keyexpr = self.query.key_expr().clone();
        
        // Parse the keyexpr to extract client_id
        let client_keyexpr = HostClientKeyexpr::try_from(keyexpr)?;
        let client_id = client_keyexpr.client_id()
            .as_ref()
            .cloned()
            .ok_or(crate::error::ArenaError::Internal(
                "Connection request missing client_id in keyexpr".to_string(),
            ))?;

        // Reply with ok to confirm acceptance
        self.query
            .reply(client_keyexpr.prefix().to_string(), "")
            .await
            .map_err(|e| crate::error::ArenaError::Zenoh(e))?;

        Ok(client_id)
    }

    /// Reject the connection request
    ///
    /// Sends an error reply to the querying client.
    /// This tells the client this host cannot accept the connection.
    pub async fn reject(self, reason: &str) -> Result<()> {
        self.query
            .reply_err(reason)
            .await
            .map_err(|e| crate::error::ArenaError::Zenoh(e))?;

        Ok(())
    }
}

/// Wrapper around Zenoh's Queryable for host discovery and connection
///
/// Declares a queryable on `<prefix>/host/<host_id>/*` to respond to:
/// - Discovery queries: `<prefix>/host/*/<client_id>` (glob on host_id)
///   → Callback replies immediately with ok (presence confirmation)
/// - Connection queries: `<prefix>/host/<host_id>/<client_id>` (specific both)
///   → Callback pushes NodeRequest to channel for host to accept/reject
///
/// The callback uses a channel to distinguish request types and forward connection
/// requests to the host handler while immediately responding to discovery queries.
#[derive(Debug)]
pub struct NodeQueryable {
    queryable: Queryable<flume::Receiver<Query>>,
    node_id: NodeId,
    /// Channel sender for connection requests (not discovery)
    request_tx: flume::Sender<NodeRequest>,
    /// Channel receiver for connection requests
    request_rx: flume::Receiver<NodeRequest>,
}

impl NodeQueryable {
    /// Declare a new queryable for a host node
    ///
    /// Declares queryable on `<prefix>/host/<host_id>/*` pattern with a callback that:
    /// 1. Checks incoming query keyexpr
    /// 2. For discovery queries (glob client_id): replies ok immediately
    /// 3. For connection queries (specific client_id): pushes NodeRequest to channel
    pub async fn declare(
        session: &zenoh::Session,
        prefix: &KeyExpr<'_>,
        node_id: NodeId,
    ) -> Result<Self> {
        // Create channel for connection requests (not discovery)
        let (request_tx, request_rx) = flume::bounded(32);
        
        // Create channel for raw queries from Zenoh
        let (query_tx, query_rx) = flume::bounded(32);

        // Declare on pattern: <prefix>/host/<host_id>/*
        let host_client_keyexpr = HostClientKeyexpr::new(prefix, Some(node_id.clone()), None);
        let keyexpr: KeyExpr = host_client_keyexpr.into();
        let prefix_str = prefix.to_string();
        
        // Declare queryable with channel handler to receive all queries
        let queryable = session
            .declare_queryable(&keyexpr)
            .with((query_tx, query_rx.clone()))
            .await
            .map_err(crate::error::ArenaError::Zenoh)?;

        // Start a background task to process queries and separate discovery from connection
        let request_tx_clone = request_tx.clone();
        let node_id_clone = node_id.clone();
        
        tokio::spawn(async move {
            while let Ok(query) = query_rx.recv_async().await {
                // Parse the incoming query keyexpr to determine if it's discovery or connection
                let query_keyexpr = query.key_expr().clone();
                
                // Try to parse as HostClientKeyexpr to extract client_id
                match HostClientKeyexpr::try_from(query_keyexpr.clone()) {
                    Ok(parsed_keyexpr) => {
                        match parsed_keyexpr.client_id() {
                            Some(_client_id) => {
                                // Connection request (specific client_id): push to channel
                                // Host will call accept() or reject()
                                let request = NodeRequest::new(query);
                                let _ = request_tx_clone.try_send(request);
                            }
                            None => {
                                // Discovery request (glob client_id): reply ok immediately
                                // This just confirms host presence for discovery phase
                                let reply_keyexpr = format!("{}/host/{}/", 
                                    prefix_str, node_id_clone.as_str());
                                // Send reply asynchronously
                                if let Err(e) = query.reply(reply_keyexpr, "").await {
                                    tracing::debug!("Failed to reply to discovery query: {}", e);
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Failed to parse keyexpr, ignore
                        tracing::debug!(
                            "Failed to parse query keyexpr: {}",
                            query_keyexpr.as_str()
                        );
                    }
                }
            }
        });

        Ok(Self {
            queryable,
            node_id,
            request_tx,
            request_rx,
        })
    }

    /// Wait for and retrieve the next connection request
    ///
    /// Returns a NodeRequest that the host handler should accept() or reject().
    /// This method blocks until a connection request arrives.
    pub async fn expect_connection(&self) -> Result<NodeRequest> {
        self.request_rx
            .recv_async()
            .await
            .map_err(|_| crate::error::ArenaError::Internal(
                "Connection request channel closed".to_string(),
            ))
    }

    /// Try to retrieve a connection request without blocking
    ///
    /// Returns Some(NodeRequest) if available, None if channel is empty.
    pub fn try_expect_connection(&self) -> Option<NodeRequest> {
        self.request_rx.try_recv().ok()
    }
}
