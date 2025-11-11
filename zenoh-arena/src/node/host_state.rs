/// Host state implementation
use std::sync::Arc;

use crate::StepResult;
use crate::error::Result;
use crate::network::keyexpr::NodeType;
use crate::{
    network::{host_queryable::HostRequest, NodePublisher, NodeSubscriber},
    node::{
        config::NodeConfig,
        game_engine::GameEngine,
        arena_node::NodeCommand,
        types::{NodeId, NodeStateInternal},
    },
};

/// State while acting as a host
pub(crate) struct HostState<E>
where
    E: GameEngine,
{
    /// List of connected client IDs
    pub(crate) connected_clients: Vec<NodeId>,
    /// Game engine (only present in Host mode)
    pub(crate) engine: E,
    /// Input channel sender (for HostState to send actions to engine)
    pub(crate) input_tx: flume::Sender<(NodeId, E::Action)>,
    /// Output channel receiver (for HostState to receive states from engine)
    pub(crate) output_rx: flume::Receiver<E::State>,
    /// Liveliness token for host discovery
    pub(crate) _liveliness_token: Option<crate::network::NodeLivelinessToken>,
    /// Queryable for host discovery
    pub(crate) queryable: Option<Arc<crate::network::HostQueryable>>,
    /// Liveliness watch to detect any client disconnect
    pub(crate) client_liveliness_watch: crate::network::NodeLivelinessWatch,
    /// Subscriber to receive actions from clients
    pub(crate) action_subscriber: NodeSubscriber<E::Action>,
    /// Publisher to send game state to all clients
    pub(crate) state_publisher: NodePublisher<E::State>,
}

impl<E> HostState<E>
where
    E: GameEngine,
{
    /// Check if host has capacity for more clients
    ///
    /// Returns true if the current client count is below the maximum allowed.
    /// Returns true if there's no maximum (unlimited clients).
    pub(crate) fn has_capacity(&self) -> bool {
        match self.engine.max_clients() {
            None => true, // Unlimited clients
            Some(max_count) => self.connected_clients.len() < max_count,
        }
    }

    /// Check if host is accepting new clients
    ///
    /// Host is accepting when it has a queryable (is advertised) and has capacity for more clients.
    /// Returns false if queryable is not present or if client count is at or above max_clients.
    pub(crate) fn is_accepting_clients(&self) -> bool {
        // Only accepting if queryable is present (advertised)
        if self.queryable.is_none() {
            return false;
        }
        // Check if we have capacity
        self.has_capacity()
    }

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
        command_rx: &flume::Receiver<NodeCommand<E::Action>>,
    ) -> Result<(NodeStateInternal<E>, StepResult<E::State>)> {
        let timeout = tokio::time::Duration::from_millis(config.step_timeout_break_ms);
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
            // Action received from a client
            action_result = self.action_subscriber.recv() => {
                match action_result {
                    Ok((sender_id, action)) => {
                        tracing::debug!(
                            "Node '{}' received action from client '{}'",
                            node_id,
                            sender_id
                        );
                        // Send action to the engine via input channel
                        if let Err(e) = self.input_tx.send((sender_id, action)) {
                            tracing::error!(
                                "Node '{}' failed to send action to engine: {}",
                                node_id,
                                e
                            );
                        }

                        true
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Node '{}' failed to receive action: {}",
                            node_id,
                            e
                        );
                        true
                    }
                }
            }
            // State received from engine
            state_result = self.output_rx.recv_async() => {
                match state_result {
                    Ok(new_game_state) => {
                        // Publish game state to all clients
                        if let Err(e) = self.state_publisher.put(&new_game_state).await {
                            tracing::error!(
                                "Node '{}' failed to publish game state: {}",
                                node_id,
                                e
                            );
                        }

                        // Return immediately with the new game state
                        return Ok((
                            NodeStateInternal::Host(self),
                            StepResult::GameState(new_game_state),
                        ));
                    }
                    Err(e) => {
                        tracing::error!(
                            "Node '{}' engine output channel closed: {}",
                            node_id,
                            e
                        );
                        true
                    }
                }
            }
            // Command received
            result = command_rx.recv_async() => match result {
                Err(_) => {
                    tracing::info!("Node '{}' command channel closed", node_id);
                    return Ok((
                        NodeStateInternal::Stop,
                        StepResult::Stop,
                    ));
                }
                Ok(NodeCommand::Stop) => {
                    tracing::info!("Node '{}' received Stop command, exiting", node_id);
                    return Ok((
                        NodeStateInternal::Stop,
                        StepResult::Stop,
                    ));
                }
                Ok(NodeCommand::GameAction(action)) => {
                    tracing::debug!(
                        "Node '{}' processing action in host mode",
                        node_id
                    );
                    // Send action to the engine via input channel
                    if let Err(e) = self.input_tx.send((node_id.clone(), action)) {
                        tracing::error!(
                            "Node '{}' failed to send action to engine: {}",
                            node_id,
                            e
                        );
                    }

                    true
                }
            }
        } {}

        // Timeout occurred without receiving new game state
        Ok((
            NodeStateInternal::Host(self),
            StepResult::Timeout,
        ))
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
        let should_accept = host_state.has_capacity();

        if should_accept {
            match request.accept().await {
                Ok(client_id) => {
                    let max_clients = host_state.engine.max_clients();
                    tracing::info!(
                        "Node '{}' accepted connection from client '{}' ({}/{})",
                        node_id,
                        client_id,
                        host_state.connected_clients.len() + 1,
                        max_clients
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
                            NodeType::Client,
                            Some(client_id.clone()),
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
                    if !host_state.has_capacity() && host_state.queryable.is_some() {
                        host_state.queryable = None;
                        tracing::debug!("Host '{}' capacity reached (dropped queryable)", node_id);
                    }
                }
                Err(e) => {
                    tracing::warn!("Node '{}' failed to accept connection: {:?}", node_id, e);
                }
            }
        } else {
            let current_count = host_state.connected_clients.len();
            let max_clients = host_state.engine.max_clients();
            tracing::info!(
                "Node '{}' rejected connection from client '{}' (limit reached: {}/{})",
                node_id,
                request.client_id().as_str(),
                current_count,
                max_clients.unwrap_or(0)
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

        let removed = if let Some(pos) = host_state
            .connected_clients
            .iter()
            .position(|id| id == &disconnected_id)
        {
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

        // Resume accepting clients if we now have capacity and queryable was dropped
        if host_state.has_capacity() && host_state.queryable.is_none() {
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
