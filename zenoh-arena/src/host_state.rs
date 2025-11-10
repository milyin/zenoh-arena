use crate::NodeCommand;
/// Host state implementation
use crate::config::NodeConfig;
use crate::error::Result;
use crate::network::host_queryable::HostRequest;
use crate::types::{NodeId, NodeStateInternal};
use std::sync::Arc;

/// State while acting as a host
pub(crate) struct HostState<E>
where
    E: crate::node::GameEngine,
{
    /// List of connected client IDs
    pub(crate) connected_clients: Vec<NodeId>,
    /// Game engine (only present in Host mode)
    pub(crate) engine: E,
    /// Liveliness token for host discovery
    pub(crate) liveliness_token: Option<crate::network::NodeLivelinessToken>,
    /// Queryable for host discovery
    pub(crate) queryable: Option<Arc<crate::network::HostQueryable>>,
    /// Multinode liveliness watch to detect any client disconnect
    pub(crate) client_liveliness_watch: crate::network::NodeLivelinessWatch,
    /// Current game state from the engine
    pub(crate) game_state: Option<E::State>,
}

impl<E> HostState<E>
where
    E: crate::node::GameEngine,
{
    /// Process the Host state - handle client connections and game actions
    ///
    /// Consumes self and returns the next state (Host or Stop if stopped).
    /// Handles commands from the command channel and processes game actions through the engine.
    /// Also monitors client liveliness to detect disconnections.
    /// Returns when either:
    /// - A new game state is produced by the engine
    /// - The step timeout elapses
    /// - A Stop command is received (returns Stop)
    /// - A client disconnects (handled and continues loop)
    pub(crate) async fn step(
        mut self,
        config: &NodeConfig,
        node_id: &NodeId,
        session: &zenoh::Session,
        command_rx: &flume::Receiver<crate::node::NodeCommand<E::Action>>,
    ) -> Result<NodeStateInternal<E>>
    {
        let timeout = tokio::time::Duration::from_millis(config.step_timeout_ms);
        let sleep = tokio::time::sleep(timeout);
        tokio::pin!(sleep);

        // Process commands until timeout or new state
        while tokio::select! {
            // Timeout elapsed
            () = &mut sleep => {
                false
            }
            // Query received from a client (connection request)
            request_result = async {
                let queryable = self.queryable.clone().expect("queryable available");
                queryable.expect_connection().await
            }, if self.queryable.is_some() => {
                if let Ok(request) = request_result {
                    Self::handle_connection_request(&mut self, config, node_id, session, request).await?;
                }
                true
            }
            // Client disconnect detected via liveliness watch
            disconnect_result = self.client_liveliness_watch.disconnected() => {
                if self.client_liveliness_watch.has_subscribers() {
                    if let Ok(disconnected_id) = disconnect_result {
                        Self::handle_client_disconnect(&mut self, config, node_id, session, disconnected_id).await?;
                    }
                }
                true
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
                        "Node '{}' processing action in host mode",
                        node_id
                    );
                    // Process action directly in the engine
                    let new_game_state = self.engine.process_action(action, node_id)?;
                    self.game_state = Some(new_game_state);
                    false
                }
            }
        } {}

        Ok(NodeStateInternal::Host(self))
    }

    /// Handle a connection request from a client
    ///
    /// Checks if the current client count is below the maximum.
    /// Accepts the connection if capacity is available, otherwise rejects it.
    async fn handle_connection_request(
        host_state: &mut Self,
        config: &NodeConfig,
        node_id: &NodeId,
        session: &zenoh::Session,
        request: HostRequest,
    ) -> Result<()> {
        let current_count = host_state.connected_clients.len();
        let max_allowed = host_state.engine.max_clients();

        let should_accept = max_allowed.map(|max| current_count < max).unwrap_or(true); // Accept if no limit

        if should_accept {
            match request.accept().await {
                Ok(client_id) => {
                    tracing::info!(
                        "Node '{}' accepted connection from client '{}' ({}/{})",
                        node_id,
                        client_id,
                        host_state.connected_clients.len() + 1,
                        max_allowed
                            .map(|m| m.to_string())
                            .unwrap_or_else(|| "unlimited".to_string())
                    );
                    // Track accepted client
                    host_state.connected_clients.push(client_id.clone());

                    // Subscribe to liveliness events for the client so we can detect disconnects
                    match host_state
                        .client_liveliness_watch
                        .subscribe(
                            session,
                            config.keyexpr_prefix.clone(),
                            crate::network::Role::Client,
                            &client_id,
                        )
                        .await
                    {
                        Ok(()) => {
                            tracing::debug!(
                                "Node '{}' subscribed to liveliness for client '{}'",
                                node_id,
                                client_id
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Node '{}' failed to subscribe to liveliness for client '{}': {}",
                                node_id,
                                client_id,
                                e
                            );
                        }
                    }

                    // Update queryable if we've reached capacity
                    let new_count = host_state.connected_clients.len();
                    let has_capacity = match max_allowed {
                        None => true, // Unlimited clients
                        Some(max_count) => new_count < max_count,
                    };

                    if !has_capacity && host_state.queryable.is_some() {
                        host_state.queryable = None;
                        tracing::debug!("Host '{}' capacity reached (dropped queryable)", node_id);
                    }
                }
                Err(e) => {
                    tracing::warn!("Node '{}' failed to accept connection: {:?}", node_id, e);
                }
            }
        } else {
            tracing::info!(
                "Node '{}' rejected connection from client '{}' (limit reached: {}/{})",
                node_id,
                request.client_id().as_str(),
                current_count,
                max_allowed.unwrap_or(0)
            );
            if let Err(e) = request.reject("Maximum number of clients reached").await {
                tracing::warn!("Node '{}' failed to reject connection: {:?}", node_id, e);
            }
        }
        Ok(())
    }

    /// Handle a client disconnect
    async fn handle_client_disconnect(
        host_state: &mut Self,
        config: &NodeConfig,
        node_id: &NodeId,
        session: &zenoh::Session,
        disconnected_id: NodeId,
    ) -> Result<()> {
        tracing::info!(
            "Node '{}' detected client '{}' disconnect",
            node_id,
            disconnected_id
        );

        let removed = if let Some(pos) = host_state.connected_clients.iter().position(|id| id == &disconnected_id) {
            host_state.connected_clients.remove(pos);
            true
        } else {
            false
        };

        if !removed {
            tracing::debug!(
                "Node '{}' received disconnect for unknown client '{}'",
                node_id,
                disconnected_id
            );
        }

        let has_capacity = match host_state.engine.max_clients() {
            None => true,
            Some(max) => host_state.connected_clients.len() < max,
        };

        if has_capacity && host_state.queryable.is_none() {
            let new_queryable = crate::network::HostQueryable::declare(
                session,
                config.keyexpr_prefix.clone(),
                node_id.clone(),
            )
            .await?;
            host_state.queryable = Some(Arc::new(new_queryable));
            tracing::debug!("Host '{}' resumed accepting clients", node_id);
        }

        Ok(())
    }
}
