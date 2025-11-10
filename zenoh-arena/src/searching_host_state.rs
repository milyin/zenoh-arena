/// SearchingHost state implementation
use crate::config::NodeConfig;
use crate::error::Result;
use crate::network::HostQuerier;
use crate::node::NodeCommand;
use crate::types::{NodeId, NodeStateInternal};

/// State while searching for available hosts
pub(crate) struct SearchingHostState;

impl SearchingHostState {
    /// Process the SearchingHost state - search for available hosts and attempt to connect
    ///
    /// Consumes self and returns the next state.
    /// Uses HostQuerier to find and connect to available hosts. If timeout expires or
    /// no hosts are available/accept connection, transitions to Host state.
    pub(crate) async fn run<E>(
        self,
        session: &zenoh::Session,
        config: &NodeConfig,
        node_id: &NodeId,
        command_rx: &flume::Receiver<NodeCommand<E::Action>>,
        get_engine: &dyn Fn() -> E,
    ) -> Result<Option<NodeStateInternal<E>>>
    where
        E: crate::node::GameEngine,
    {
        tracing::info!("Node '{}' searching for hosts...", node_id);

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
                        return Ok(None);
                    }
                    Ok(crate::node::NodeCommand::Stop) => {
                        tracing::info!("Node '{}' received Stop command during search, exiting", node_id);
                        return Ok(None);
                    }
                    Ok(crate::node::NodeCommand::GameAction(_)) => {
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
            let mut next_state = NodeStateInternal::SearchingHost;
            next_state
                .client(
                    session,
                    config.keyexpr_prefix.clone(),
                    host_id,
                    node_id.clone(),
                )
                .await?;
            Ok(Some(next_state))
        } else {
            // Transition to Host state
            let mut next_state = NodeStateInternal::SearchingHost;
            next_state
                .host(
                    get_engine(),
                    session,
                    config.keyexpr_prefix.clone(),
                    node_id,
                )
                .await?;
            Ok(Some(next_state))
        }
    }
}
