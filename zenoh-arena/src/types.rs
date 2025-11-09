/// Core types for the zenoh-arena library
use std::sync::Arc;
use std::time::Instant;

use crate::error::{ArenaError, Result};
use crate::network::{HostQueryable, NodeLivelinessToken, NodeLivelinessWatch, Role};
use futures::future::select_all;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use zenoh::key_expr::KeyExpr;

/// Unique node identifier
///
/// NodeId must be a valid single-chunk keyexpr:
/// - Non-empty UTF-8 string
/// - Cannot contain: / * $ ? # @
/// - Must be a single chunk (no slashes)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(String);

impl NodeId {
    /// Generate a new unique node ID (guaranteed to be keyexpr-safe)
    /// Uses base58 encoding of UUID to avoid special characters
    pub fn generate() -> Self {
        let uuid = uuid::Uuid::new_v4();
        let encoded = bs58::encode(uuid.as_bytes()).into_string();
        // Take first 16 characters for reasonable length
        let shortened = encoded.chars().take(16).collect::<String>();
        NodeId(shortened)
    }

    /// Create from a specific name (must be unique and keyexpr-compatible)
    /// Returns error if name contains invalid characters
    pub fn from_name(name: String) -> Result<Self> {
        Self::validate(&name)?;
        Ok(NodeId(name))
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate that a string can be used as NodeId (single keyexpr chunk)
    fn validate(s: &str) -> Result<()> {
        if s.is_empty() {
            return Err(ArenaError::InvalidNodeName(
                "Node name cannot be empty".to_string(),
            ));
        }

        // Check for invalid characters: / * $ ? # @
        for ch in s.chars() {
            if matches!(ch, '/' | '*' | '$' | '?' | '#' | '@') {
                return Err(ArenaError::InvalidNodeName(format!(
                    "Node name '{}' contains invalid character '{}'",
                    s, ch
                )));
            }
        }

        Ok(())
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Node role in the arena
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    /// Client role - connects to hosts
    Client,
    /// Host role - accepts clients and runs game engine
    Host,
}

/// Node information
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// Node identifier
    pub id: NodeId,
    /// Node role
    pub role: NodeRole,
    /// Time when node was created or connected
    pub connected_since: Instant,
}

/// Future type used to monitor client liveliness disconnect events
type ClientDisconnectFuture = BoxFuture<'static, (NodeId, Result<()>)>;

/// Public node state returned by step() method
#[derive(Debug, Clone)]
pub enum NodeState {
    /// Searching for available hosts
    SearchingHost,
    /// Connected as client to a host
    Client {
        /// ID of the host we're connected to
        host_id: NodeId,
    },
    /// Acting as host
    Host {
        /// Whether accepting new clients (derived from queryable presence and capacity)
        is_accepting: bool,
        /// List of connected client IDs
        connected_clients: Vec<NodeId>,
    },
}

impl std::fmt::Display for NodeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeState::SearchingHost => write!(f, "Searching for host..."),
            NodeState::Client { host_id } => write!(f, "Connected as client to host: {}", host_id),
            NodeState::Host {
                is_accepting,
                connected_clients,
            } => {
                let accepting_str = if *is_accepting { "open" } else { "closed" };
                if connected_clients.is_empty() {
                    write!(f, "Host mode ({}, no clients)", accepting_str)
                } else {
                    write!(
                        f,
                        "Host mode ({}, {} client(s))",
                        accepting_str,
                        connected_clients.len()
                    )
                }
            }
        }
    }
}

/// Status returned by Node::step() method
#[derive(Debug, Clone)]
pub struct NodeStatus<S> {
    /// Current node state (Searching, Client, or Host)
    pub state: NodeState,
    /// Current game state (if available)
    pub game_state: Option<S>,
}

impl<S: std::fmt::Display> std::fmt::Display for NodeStatus<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[State] {}", self.state)?;
        if let Some(ref game_state) = self.game_state {
            write!(f, " | Game: {}", game_state)?;
        }
        Ok(())
    }
}

/// Current state of a Node (internal)
#[derive(Debug, Default)]
pub(crate) enum NodeStateInternal<E>
where
    E: crate::node::GameEngine,
{
    /// Searching for available hosts
    #[default]
    SearchingHost,

    /// Connected as client to a host
    #[allow(dead_code)]
    Client {
        /// ID of the host we're connected to
        host_id: NodeId,
        /// Watches for host liveliness to detect disconnection
        liveliness_watch: NodeLivelinessWatch,
        /// Client's liveliness token (role: Client) for the host to track disconnection
        #[allow(dead_code)]
        liveliness_token: NodeLivelinessToken,
    },

    /// Acting as host
    Host {
        /// List of connected client IDs
        connected_clients: Vec<NodeId>,
        /// Game engine (only present in Host mode)
        #[allow(dead_code)]
        engine: E,
        /// Liveliness token for host discovery
        #[allow(dead_code)]
        liveliness_token: Option<NodeLivelinessToken>,
        /// Queryable for host discovery
        #[allow(dead_code)]
        queryable: Option<Arc<HostQueryable>>,
        /// Sender used to register new client disconnect monitors
        client_disconnect_sender: UnboundedSender<ClientDisconnectFuture>,
        /// Receiver yielding client disconnect events
        client_disconnect_events: UnboundedReceiver<(NodeId, Result<()>)>,
        /// Background task handling client disconnect monitoring
        #[allow(dead_code)]
        client_disconnect_task: JoinHandle<()>,
    },
}

impl<E> NodeStateInternal<E>
where
    E: crate::node::GameEngine,
{
    /// Check if currently in host mode
    #[allow(dead_code)]
    pub fn is_host(&self) -> bool {
        matches!(self, NodeStateInternal::Host { .. })
    }

    /// Check if currently in client mode
    #[allow(dead_code)]
    pub fn is_client(&self) -> bool {
        matches!(self, NodeStateInternal::Client { .. })
    }

    /// Check if host mode and accepting clients
    ///
    /// Host is accepting when it has a queryable and client count is below max_clients
    #[allow(dead_code)]
    pub fn is_accepting_clients(&self) -> bool {
        match self {
            NodeStateInternal::Host {
                queryable,
                connected_clients,
                engine,
                ..
            } => {
                // Only accepting if queryable is present (advertised)
                if queryable.is_none() {
                    return false;
                }

                // Check if we have capacity
                let max = engine.max_clients();
                let current = connected_clients.len();
                match max {
                    None => true, // Unlimited clients
                    Some(max_count) => current < max_count,
                }
            }
            _ => false,
        }
    }

    /// Get number of connected clients (None if not host)
    #[allow(dead_code)]
    pub fn client_count(&self) -> Option<usize> {
        match self {
            NodeStateInternal::Host {
                connected_clients, ..
            } => Some(connected_clients.len()),
            _ => None,
        }
    }

    /// Transition to SearchingHost state from any state
    ///
    /// Drops the current state (including engine and liveliness token if in Host mode)
    #[allow(dead_code)]
    pub fn searching(&mut self) {
        *self = NodeStateInternal::SearchingHost;
    }

    /// Transition from SearchingHost to Host state
    ///
    /// Creates liveliness token and queryable for host discovery
    pub async fn host(
        &mut self,
        engine: E,
        session: &zenoh::Session,
        prefix: impl Into<KeyExpr<'static>>,
        node_id: &NodeId,
    ) -> Result<()>
    where
        E: crate::node::GameEngine,
    {
        let prefix = prefix.into();

        // Create host liveliness token for discovery
        let token =
            NodeLivelinessToken::declare(session, prefix.clone(), Role::Host, node_id.clone())
                .await?;

        // Declare queryable for host discovery
        let queryable = HostQueryable::declare(session, prefix.clone(), node_id.clone()).await?;

        let (client_disconnect_sender, client_disconnect_events, client_disconnect_task) =
            start_client_disconnect_monitor();

        *self = NodeStateInternal::Host {
            connected_clients: Vec::new(),
            engine,
            liveliness_token: Some(token),
            queryable: Some(Arc::new(queryable)),
            client_disconnect_sender,
            client_disconnect_events,
            client_disconnect_task,
        };

        Ok(())
    }

    /// Transition from SearchingHost to Client state
    ///
    /// Subscribes to liveliness events for the host and declares a client liveliness token
    #[allow(dead_code)]
    pub async fn client(
        &mut self,
        session: &zenoh::Session,
        prefix: impl Into<KeyExpr<'static>>,
        host_id: NodeId,
        client_id: NodeId,
    ) -> Result<()> {
        use crate::network::NodeLivelinessWatch;

        let prefix = prefix.into();

        // Subscribe to liveliness events for the host
        let liveliness_watch =
            NodeLivelinessWatch::subscribe(session, prefix.clone(), Role::Host, host_id.clone())
                .await?;

        // Declare client liveliness token (role: Client) so host can track our presence
        let liveliness_token =
            NodeLivelinessToken::declare(session, prefix, Role::Client, client_id).await?;

        *self = NodeStateInternal::Client {
            host_id,
            liveliness_watch,
            liveliness_token,
        };

        Ok(())
    }
}

impl<E> From<&NodeStateInternal<E>> for NodeState
where
    E: crate::node::GameEngine,
{
    fn from(internal: &NodeStateInternal<E>) -> Self {
        match internal {
            NodeStateInternal::SearchingHost => NodeState::SearchingHost,
            NodeStateInternal::Client { host_id, .. } => NodeState::Client {
                host_id: host_id.clone(),
            },
            NodeStateInternal::Host {
                connected_clients,
                engine,
                queryable,
                ..
            } => {
                // Host is accepting if it has a queryable and has capacity
                let is_accepting = queryable.is_some() && {
                    let max = engine.max_clients();
                    let current = connected_clients.len();
                    match max {
                        None => true, // Unlimited clients
                        Some(max_count) => current < max_count,
                    }
                };

                NodeState::Host {
                    is_accepting,
                    connected_clients: connected_clients.clone(),
                }
            }
        }
    }
}

fn start_client_disconnect_monitor() -> (
    UnboundedSender<ClientDisconnectFuture>,
    UnboundedReceiver<(NodeId, Result<()>)>,
    JoinHandle<()>,
) {
    let (watch_tx, mut watch_rx) = mpsc::unbounded_channel::<ClientDisconnectFuture>();
    let (event_tx, event_rx) = mpsc::unbounded_channel::<(NodeId, Result<()>)>();

    let task = tokio::spawn(async move {
        let mut pending: Vec<ClientDisconnectFuture> = Vec::new();

        loop {
            // Drain all new futures from the channel
            while let Ok(fut) = watch_rx.try_recv() {
                pending.push(fut);
            }

            if pending.is_empty() {
                // Wait for at least one future
                if let Some(fut) = watch_rx.recv().await {
                    pending.push(fut);
                } else {
                    break;
                }
            }

            // Wait for any pending future or a new one from the channel
            let select_all_fut = select_all(std::mem::take(&mut pending)).fuse();
            let watch_recv = Box::pin(watch_rx.recv()).fuse();
            
            futures::pin_mut!(select_all_fut, watch_recv);
            
            futures::select! {
                (result, _idx, remaining) = select_all_fut => {
                    pending = remaining;
                    if event_tx.send(result).is_err() {
                        break;
                    }
                }
                opt_fut = watch_recv => {
                    if let Some(fut) = opt_fut {
                        pending.push(fut);
                    } else {
                        break;
                    }
                }
            }
        }
    });

    (watch_tx, event_rx, task)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_state_display_searching() {
        let state = NodeState::SearchingHost;
        assert_eq!(format!("{}", state), "Searching for host...");
    }

    #[test]
    fn test_node_state_display_client() {
        let host_id = NodeId::from_name("test_host".to_string()).unwrap();
        let state = NodeState::Client { host_id };
        assert_eq!(
            format!("{}", state),
            "Connected as client to host: test_host"
        );
    }

    #[test]
    fn test_node_state_display_host_empty() {
        let state = NodeState::Host {
            is_accepting: true,
            connected_clients: vec![],
        };
        assert_eq!(format!("{}", state), "Host mode (open, no clients)");
    }

    #[test]
    fn test_node_state_display_host_with_clients() {
        let client1 = NodeId::from_name("client1".to_string()).unwrap();
        let client2 = NodeId::from_name("client2".to_string()).unwrap();
        let state = NodeState::Host {
            is_accepting: true,
            connected_clients: vec![client1, client2],
        };
        assert_eq!(format!("{}", state), "Host mode (open, 2 client(s))");
    }

    #[test]
    fn test_node_status_display_no_game_state() {
        let status: NodeStatus<String> = NodeStatus {
            state: NodeState::SearchingHost,
            game_state: None,
        };
        assert_eq!(format!("{}", status), "[State] Searching for host...");
    }

    #[test]
    fn test_node_status_display_with_game_state() {
        let status: NodeStatus<String> = NodeStatus {
            state: NodeState::SearchingHost,
            game_state: Some("Level 5".to_string()),
        };
        assert_eq!(
            format!("{}", status),
            "[State] Searching for host... | Game: Level 5"
        );
    }
}
