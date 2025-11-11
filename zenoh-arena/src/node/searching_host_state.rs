/// SearchingHost state implementation
use std::sync::Arc;

use super::config::NodeConfig;
use crate::error::Result;
use crate::network::HostQuerier;
use super::game_engine::{EngineFactory, GameEngine};
use super::node::NodeCommand;
use super::types::{NodeId, NodeStateInternal};
use rand::Rng;

/// State while searching for available hosts
pub(crate) struct SearchingHostState;

impl SearchingHostState {
    /// Process the SearchingHost state - search for available hosts and attempt to connect
    ///
    /// Consumes self and returns the next state.
    /// Uses HostQuerier to find and connect to available hosts. If timeout expires or
    /// no hosts are available/accept connection, transitions to Host state.
    pub(crate) async fn step<E, F>(
        self,
        session: &zenoh::Session,
        config: &NodeConfig,
        node_id: &NodeId,
        command_rx: &flume::Receiver<NodeCommand<E::Action>>,
        get_engine: &F,
    ) -> Result<NodeStateInternal<E>>
    where
        E: GameEngine,
        F: EngineFactory<E>,
    {
        tracing::info!("Node '{}' searching for hosts...", node_id);

        // Add randomized jitter to prevent thundering herd when multiple clients
        // lose their host simultaneously
        if config.search_jitter_ms > 0 {
            let jitter_ms = rand::rng().random_range(0..config.search_jitter_ms);
            tracing::debug!(
                "Node '{}' waiting {}ms jitter before searching",
                node_id,
                jitter_ms
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(jitter_ms)).await;
        }

        let search_timeout = tokio::time::Duration::from_millis(config.search_timeout_ms);
        let sleep = tokio::time::sleep(search_timeout);
        tokio::pin!(sleep);

        // Wait for connection success or timeout
        // Returns None if should become host, Some(host_id) if connected
        let connected_host = loop {
            tokio::select! {
                // Search timeout elapsed - no successful connection, become host
                () = &mut sleep => {
                    tracing::info!(
                        "Node '{}' search timeout - no hosts accepted connection",
                        node_id
                    );
                    break None;
                }
                // Try to connect to available hosts
                connection_result = HostQuerier::connect(session, config.keyexpr_prefix.clone(), node_id.clone()) => {
                    match connection_result {
                        Ok(Some(host_id)) => {
                            // Successfully connected to a host
                            tracing::info!("Node '{}' connected to host: {}", node_id, host_id);
                            break Some(host_id);
                        }
                        Ok(None) => {
                            // No hosts available, become host
                            tracing::info!("Node '{}' no hosts available", node_id);
                            break None;
                        }
                        Err(e) => {
                            tracing::warn!("Node '{}' query error during search: {}", node_id, e);
                            return Err(e);
                        }
                    }
                }
                // Check for Stop command while searching
                result = command_rx.recv_async() => match result {
                    Err(_) => {
                        tracing::info!("Node '{}' command channel closed during search", node_id);
                        return Ok(NodeStateInternal::Stop);
                    }
                    Ok(NodeCommand::Stop) => {
                        tracing::info!("Node '{}' received Stop command during search, exiting", node_id);
                        return Ok(NodeStateInternal::Stop);
                    }
                    Ok(NodeCommand::GameAction(_)) => {
                        tracing::warn!(
                            "Node '{}' received action while searching for host, ignoring",
                            node_id
                        );
                        // Continue searching
                    }
                }
            }
        };

        // Handle connection result - state transition after select!
        if let Some(host_id) = connected_host {
            // Transition to Client state
            let next_state = NodeStateInternal::client(
                session,
                config.keyexpr_prefix.clone(),
                host_id,
                node_id.clone(),
            )
            .await?;
            Ok(next_state)
        } else {
            // Transition to Host state
            let next_state = NodeStateInternal::host(
                &*get_engine,
                session,
                config.keyexpr_prefix.clone(),
                node_id,
            )
            .await?;
            Ok(next_state)
        }
    }
}
