/// Host state implementation
use crate::config::NodeConfig;
use crate::error::Result;
use crate::network::host_queryable::HostRequest;
use crate::types::{NodeId, NodeState, NodeStateInternal, NodeStatus};
use std::sync::Arc;

/// State while acting as a host
pub(crate) struct HostState;

impl HostState {
    /// Process the Host state - handle client connections and game actions
    ///
    /// Handles commands from the command channel and processes game actions through the engine.
    /// Also monitors client liveliness to detect disconnections.
    /// Returns when either:
    /// - A new game state is produced by the engine
    /// - The step timeout elapses
    /// - A Stop command is received (returns None)
    /// - A client disconnects (handled and continues loop)
    pub(crate) async fn run<E>(
        &self,
        state: &mut NodeStateInternal<E>,
        config: &NodeConfig,
        node_id: &NodeId,
        session: &zenoh::Session,
        command_rx: &flume::Receiver<crate::node::NodeCommand<E::Action>>,
    ) -> Result<Option<NodeStatus<E::State>>>
    where
        E: crate::node::GameEngine,
    {
        let timeout = tokio::time::Duration::from_millis(config.step_timeout_ms);
        let sleep = tokio::time::sleep(timeout);
        tokio::pin!(sleep);

        // Process commands until timeout or new state
        loop {
            // Snapshot queryable and whether we have clients to monitor
            let (queryable_arc, has_clients) = match &state {
                NodeStateInternal::Host {
                    queryable,
                    client_liveliness_watch,
                    ..
                } => (queryable.clone(), client_liveliness_watch.has_subscribers()),
                _ => {
                    return Ok(Some(NodeStatus {
                        state: NodeState::from(&*state),
                        game_state: None,
                    }));
                }
            };

            tokio::select! {
                // Timeout elapsed
                () = &mut sleep => {
                    return Ok(Some(NodeStatus {
                        state: NodeState::from(&*state),
                        game_state: None,
                    }));
                }
                // Query received from a client (connection request)
                request_result = async {
                    let queryable = queryable_arc.clone().expect("queryable available");
                    queryable.expect_connection().await
                }, if queryable_arc.is_some() => {
                    if let Ok(request) = request_result {
                        Self::handle_connection_request(state, config, node_id, session, request).await?;
                    }
                }
                // Client disconnect detected via liveliness watch
                disconnect_result = async {
                    if let NodeStateInternal::Host {
                        client_liveliness_watch,
                        ..
                    } = state
                    {
                        client_liveliness_watch.disconnected().await
                    } else {
                        futures::future::pending().await
                    }
                }, if has_clients => {
                    if let Ok(disconnected_id) = disconnect_result {
                        Self::handle_client_disconnect(state, config, node_id, session, disconnected_id).await?;
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
                    Ok(crate::node::NodeCommand::GameAction(action)) => {
                        if let NodeStateInternal::Host { engine, .. } = state {
                            tracing::debug!(
                                "Node '{}' processing action in host mode",
                                node_id
                            );
                            // Process action directly in the engine and get new state
                            let new_game_state = engine.process_action(action, node_id)?;
                            // Build the node state info using From trait
                            return Ok(Some(NodeStatus {
                                state: NodeState::from(&*state),
                                game_state: Some(new_game_state),
                            }));
                        }
                    }
                }
            }
        }
    }

    /// Handle a connection request from a client
    ///
    /// Checks if the node is in host mode and if the current client count is below the maximum.
    /// Accepts the connection if capacity is available, otherwise rejects it.
    async fn handle_connection_request<E>(
        state: &mut NodeStateInternal<E>,
        config: &NodeConfig,
        node_id: &NodeId,
        session: &zenoh::Session,
        request: HostRequest,
    ) -> Result<()>
    where
        E: crate::node::GameEngine,
    {
        let NodeStateInternal::Host {
            engine,
            connected_clients,
            queryable,
            client_liveliness_watch,
            ..
        } = state
        else {
            tracing::warn!(
                "Node '{}' received connection request but not in host mode",
                node_id
            );
            return Ok(());
        };

        let current_count = connected_clients.len();
        let max_allowed = engine.max_clients();

        let should_accept = max_allowed.map(|max| current_count < max).unwrap_or(true); // Accept if no limit

        if should_accept {
            match request.accept().await {
                Ok(client_id) => {
                    tracing::info!(
                        "Node '{}' accepted connection from client '{}' ({}/{})",
                        node_id,
                        client_id,
                        connected_clients.len() + 1,
                        max_allowed
                            .map(|m| m.to_string())
                            .unwrap_or_else(|| "unlimited".to_string())
                    );
                    // Track accepted client
                    connected_clients.push(client_id.clone());

                    // Subscribe to liveliness events for the client so we can detect disconnects
                    match client_liveliness_watch
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
                    let new_count = connected_clients.len();
                    let has_capacity = match max_allowed {
                        None => true, // Unlimited clients
                        Some(max_count) => new_count < max_count,
                    };

                    if !has_capacity && queryable.is_some() {
                        *queryable = None;
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
    async fn handle_client_disconnect<E>(
        state: &mut NodeStateInternal<E>,
        config: &NodeConfig,
        node_id: &NodeId,
        session: &zenoh::Session,
        disconnected_id: NodeId,
    ) -> Result<()>
    where
        E: crate::node::GameEngine,
    {
        let NodeStateInternal::Host {
            connected_clients,
            queryable,
            engine,
            ..
        } = state
        else {
            return Ok(());
        };

        tracing::info!(
            "Node '{}' detected client '{}' disconnect",
            node_id,
            disconnected_id
        );

        let removed = if let Some(pos) = connected_clients.iter().position(|id| id == &disconnected_id) {
            connected_clients.remove(pos);
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

        let has_capacity = match engine.max_clients() {
            None => true,
            Some(max) => connected_clients.len() < max,
        };

        if has_capacity && queryable.is_none() {
            let new_queryable = crate::network::HostQueryable::declare(
                session,
                config.keyexpr_prefix.clone(),
                node_id.clone(),
            )
            .await?;
            *queryable = Some(Arc::new(new_queryable));
            tracing::debug!("Host '{}' resumed accepting clients", node_id);
        }

        Ok(())
    }
}
