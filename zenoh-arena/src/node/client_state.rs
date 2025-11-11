/// Client state implementation
use crate::node::config::NodeConfig;
use crate::error::Result;
use crate::network::{NodeLivelinessToken, NodeLivelinessWatch, NodePublisher};
use crate::node::node::{GameEngine, NodeCommand};
use crate::node::types::{NodeId, NodeStateInternal};

/// State while connected as a client to a host
pub(crate) struct ClientState<Action> {
    /// ID of the host we're connected to
    pub(crate) host_id: NodeId,
    /// Watches for host liveliness to detect disconnection
    pub(crate) liveliness_watch: NodeLivelinessWatch,
    /// Client's liveliness token (type: Client) for the host to track disconnection
    #[allow(dead_code)]
    pub(crate) liveliness_token: NodeLivelinessToken,
    /// Publisher for sending actions to the host
    pub(crate) action_publisher: NodePublisher<Action>,
}

impl<Action> ClientState<Action>
where
    Action: zenoh_ext::Serialize,
{
    /// Process the Client state - handle commands while connected to a host
    ///
    /// Consumes self and returns the next state.
    /// Handles commands from the command channel while connected to a host.
    /// Monitors liveliness of the connected host and returns to SearchingHost if disconnected.
    /// Returns when either:
    /// - Host liveliness is lost (transitions back to SearchingHost)
    /// - The step timeout elapses
    /// - A Stop command is received (returns Stop)
    pub(crate) async fn step<E>(
        mut self,
        config: &NodeConfig,
        node_id: &NodeId,
        command_rx: &flume::Receiver<NodeCommand<E::Action>>,
    ) -> Result<NodeStateInternal<E>>
    where
        E: GameEngine<Action = Action>,
    {
        let timeout = tokio::time::Duration::from_millis(config.step_timeout_ms);
        let sleep = tokio::time::sleep(timeout);
        tokio::pin!(sleep);

        // Process commands until timeout, shutdown, or host disconnection
        loop {
            tokio::select! {
                // Timeout elapsed
                () = &mut sleep => {
                    return Ok(NodeStateInternal::Client(self));
                }
                // Host liveliness lost - disconnect and return to searching
                disconnect_result = self.liveliness_watch.disconnected() => {
                    match disconnect_result {
                        Ok(disconnected_id) => {
                            tracing::info!("Node '{}' detected host '{}' disconnection, returning to search", node_id, disconnected_id);
                            // Transition back to SearchingHost
                            return Ok(NodeStateInternal::SearchingHost);
                        }
                        Err(e) => {
                            tracing::warn!("Node '{}' liveliness error: {}", node_id, e);
                            // Treat error as disconnect
                            return Ok(NodeStateInternal::SearchingHost);
                        }
                    }
                }
                // Command received
                result = command_rx.recv_async() => match result {
                    Err(_) => {
                        tracing::info!("Node '{}' command channel closed", node_id);
                        return Ok(NodeStateInternal::Stop);
                    }
                    Ok(NodeCommand::Stop) => {
                        tracing::info!("Node '{}' received Stop command, exiting", node_id);
                        return Ok(NodeStateInternal::Stop);
                    }
                    Ok(NodeCommand::GameAction(action)) => {
                        tracing::debug!(
                            "Node '{}' forwarding action to host '{}'",
                            node_id,
                            self.host_id
                        );
                        // Send action to remote host via Zenoh pub/sub
                        if let Err(e) = self.action_publisher.put(&action).await {
                            tracing::error!(
                                "Node '{}' failed to send action to host '{}': {}",
                                node_id,
                                self.host_id,
                                e
                            );
                        }
                        // Continue the loop
                        continue;
                    }
                }
            }
        }
    }
}
