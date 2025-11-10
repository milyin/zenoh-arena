/// Client state implementation
use crate::config::NodeConfig;
use crate::error::Result;
use crate::network::{NodeLivelinessToken, NodeLivelinessWatch};
use crate::types::{NodeId, NodeState, NodeStateInternal, NodeStatus};

/// State while connected as a client to a host
pub(crate) struct ClientState {
    /// ID of the host we're connected to
    pub(crate) host_id: NodeId,
    /// Watches for host liveliness to detect disconnection
    pub(crate) liveliness_watch: NodeLivelinessWatch,
    /// Client's liveliness token (role: Client) for the host to track disconnection
    #[allow(dead_code)]
    pub(crate) liveliness_token: NodeLivelinessToken,
}

impl ClientState {
    /// Process the Client state - handle commands while connected to a host
    ///
    /// Consumes self and returns the status along with the next state.
    /// Handles commands from the command channel while connected to a host.
    /// Monitors liveliness of the connected host and returns to SearchingHost if disconnected.
    /// Returns when either:
    /// - Host liveliness is lost (transitions back to SearchingHost)
    /// - The step timeout elapses
    /// - A Stop command is received (returns None)
    pub(crate) async fn run<E>(
        mut self,
        config: &NodeConfig,
        node_id: &NodeId,
        command_rx: &flume::Receiver<crate::node::NodeCommand<E::Action>>,
    ) -> Result<Option<(NodeStatus<E::State>, NodeStateInternal<E>)>>
    where
        E: crate::node::GameEngine,
    {
        let timeout = tokio::time::Duration::from_millis(config.step_timeout_ms);
        let sleep = tokio::time::sleep(timeout);
        tokio::pin!(sleep);

        // Process commands until timeout, shutdown, or host disconnection
        loop {
            tokio::select! {
                // Timeout elapsed
                () = &mut sleep => {
                    return Ok(Some((
                        NodeStatus {
                            state: NodeState::Client {
                                host_id: self.host_id.clone(),
                            },
                            game_state: None,
                        },
                        NodeStateInternal::Client(self),
                    )));
                }
                // Host liveliness lost - disconnect and return to searching
                disconnect_result = self.liveliness_watch.disconnected() => {
                    match disconnect_result {
                        Ok(disconnected_id) => {
                            tracing::info!("Node '{}' detected host '{}' disconnection, returning to search", node_id, disconnected_id);
                            // Transition back to SearchingHost
                            return Ok(Some((
                                NodeStatus {
                                    state: NodeState::SearchingHost,
                                    game_state: None,
                                },
                                NodeStateInternal::SearchingHost,
                            )));
                        }
                        Err(e) => {
                            tracing::warn!("Node '{}' liveliness error: {}", node_id, e);
                            // Treat error as disconnect
                            return Ok(Some((
                                NodeStatus {
                                    state: NodeState::SearchingHost,
                                    game_state: None,
                                },
                                NodeStateInternal::SearchingHost,
                            )));
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
                            self.host_id
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
