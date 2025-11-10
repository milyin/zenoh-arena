/// Client state implementation
use crate::config::NodeConfig;
use crate::error::Result;
use crate::types::{NodeId, NodeState, NodeStateInternal, NodeStatus};

/// State while connected as a client to a host
pub(crate) struct ClientState;

impl ClientState {
    /// Process the Client state - handle commands while connected to a host
    ///
    /// Handles commands from the command channel while connected to a host.
    /// Monitors liveliness of the connected host and returns to SearchingHost if disconnected.
    /// Returns when either:
    /// - Host liveliness is lost (transitions back to SearchingHost)
    /// - The step timeout elapses
    /// - A Stop command is received (returns None)
    pub(crate) async fn run<E>(
        &self,
        state: &mut NodeStateInternal<E>,
        config: &NodeConfig,
        node_id: &NodeId,
        command_rx: &flume::Receiver<crate::node::NodeCommand<E::Action>>,
    ) -> Result<Option<NodeStatus<E::State>>>
    where
        E: crate::node::GameEngine,
    {
        // Extract the client state data temporarily to use the liveliness watch
        let (host_id, mut liveliness_watch, _liveliness_token) =
            match std::mem::take(state) {
                NodeStateInternal::Client {
                    host_id,
                    liveliness_watch,
                    liveliness_token,
                } => (host_id, liveliness_watch, liveliness_token),
                other_state => {
                    // Restore state if it wasn't Client
                    *state = other_state;
                    return Ok(Some(NodeStatus {
                        state: NodeState::from(&*state),
                        game_state: None,
                    }));
                }
            };

        let timeout = tokio::time::Duration::from_millis(config.step_timeout_ms);
        let sleep = tokio::time::sleep(timeout);
        tokio::pin!(sleep);

        // Process commands until timeout, shutdown, or host disconnection
        loop {
            tokio::select! {
                // Timeout elapsed
                () = &mut sleep => {
                    // No disconnection yet, restore state and return
                    *state = NodeStateInternal::Client {
                        host_id: host_id.clone(),
                        liveliness_watch,
                        liveliness_token: _liveliness_token,
                    };
                    return Ok(Some(NodeStatus {
                        state: NodeState::from(&*state),
                        game_state: None,
                    }));
                }
                // Host liveliness lost - disconnect and return to searching
                disconnect_result = liveliness_watch.disconnected() => {
                    match disconnect_result {
                        Ok(disconnected_id) => {
                            tracing::info!("Node '{}' detected host '{}' disconnection, returning to search", node_id, disconnected_id);
                            // Transition back to SearchingHost
                            *state = NodeStateInternal::SearchingHost;
                            return Ok(Some(NodeStatus {
                                state: NodeState::from(&*state),
                                game_state: None,
                            }));
                        }
                        Err(e) => {
                            tracing::warn!("Node '{}' liveliness error: {}", node_id, e);
                            // Treat error as disconnect
                            *state = NodeStateInternal::SearchingHost;
                            return Ok(Some(NodeStatus {
                                state: NodeState::from(&*state),
                                game_state: None,
                            }));
                        }
                    }
                }
                // Command received
                result = command_rx.recv_async() => match result {
                    Err(_) => {
                        tracing::info!("Node '{}' command channel closed", node_id);
                        return Ok(None);
                    }
                    Ok(crate::node::NodeCommand::Stop) => {
                        tracing::info!("Node '{}' received Stop command, exiting", node_id);
                        return Ok(None);
                    }
                    Ok(crate::node::NodeCommand::GameAction(_action)) => {
                        tracing::debug!(
                            "Node '{}' forwarding action to host '{}'",
                            node_id,
                            host_id
                        );
                        // TODO: Forward action to remote host via Zenoh pub/sub
                        // Placeholder for Phase 4 implementation
                        // Continue the loop
                        continue;
                    }
                }
            }
        }
    }
}
