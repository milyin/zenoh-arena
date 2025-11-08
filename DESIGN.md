# zenoh-arena Library Design Document

## Overview

The `zenoh-arena` library is a peer-to-peer network framework for simple game applications built on top of the Zenoh network library. It provides a `Node`-centric architecture where each node manages its own role (host or client), handles discovery, connection management, and state synchronization for distributed game sessions. There is no central "Arena" coordinator - each node is autonomous and manages its local view of the network.

## Important Design Constraints

### Key Expression Compatibility

All node identifiers must be valid **single-chunk** Zenoh key expressions:
- Non-empty UTF-8 strings
- Cannot contain: `/` (separator), `*` (wildcard), `$` (DSL), `?` `#` (reserved), `@` (verbatim)
- Must represent a single chunk (no path separators)
- Auto-generated IDs use base58-encoded UUIDs to guarantee validity
- Custom node names are validated at construction time

See [Zenoh Key Expressions RFC](https://github.com/eclipse-zenoh/roadmap/blob/main/rfcs/ALL/Key%20Expressions.md) for details.

### Serialization

The library uses Zenoh's native serialization format (`zenoh-ext`) rather than serde:
- Game types must implement `zenoh_ext::Serialize` and `zenoh_ext::Deserialize`
- Uses `zenoh_ext::z_serialize` and `zenoh_ext::z_deserialize` functions
- Ensures interoperability between Zenoh-based applications
- See [Zenoh Serialization Format](https://github.com/eclipse-zenoh/roadmap/blob/main/rfcs/ALL/Serialization.md)

### Liveliness Namespace Separation

Zenoh's liveliness tokens are stored in a **hermetic `@` namespace**, completely separate from regular pub/sub data:
- Liveliness tokens cannot interfere with or match regular keyexprs
- The `@` namespace is used by Zenoh for control/admin data
- The liveliness API abstracts this - you use normal keyexprs, Zenoh handles the mapping
- Example: `session.liveliness().declare_token("my/app/node1")` is internally mapped to `@<internal>/my/app/node1`
- No special prefixes or namespace handling needed in application code

This ensures complete isolation between application data and liveliness tracking.

## Core Concepts

### Node States

Each `Node` operates in one of three states:

- **SearchingHost**: Node is looking for available hosts to connect to
- **Client**: Node is connected to a host, sending actions and receiving game state
- **Host**: Node runs the game engine, accepts client connections, processes actions, broadcasts state

**Important**: NodeState is an internal implementation detail and not exposed in the public API.

### Node Behavior

**As SearchingHost:**

- Queries the network for available hosts via Zenoh query
- Evaluates available hosts based on acceptance status and capacity
- Waits for responses with configurable timeout and randomized jitter
- Transitions to Client state when a host accepts connection
- Transitions to Host state when no hosts found (depending on configuration)
- Note: This state is skipped entirely if `force_host` is enabled

**As Client:**

- Maintains connection to a specific host
- Publishes actions to host via dedicated keyexpr
- Subscribes to state updates from host
- Monitors host liveliness via liveliness tokens
- Transitions to SearchingHost if host disconnects or connection is lost
- Note: This state cannot be entered if `force_host` is enabled

**As Host:**

- Runs the game engine instance
- Declares queryable for discovery (when accepting clients)
- Accepts/rejects client join requests based on capacity
- Subscribes to actions from all clients (wildcard pattern)
- Processes actions through game engine
- Publishes state updates to all clients
- Manages client lifecycle (connections/disconnections)
- Declares liveliness token
- Can be Open (accepting clients) or Closed (not accepting)
- Can be Empty (no clients) or have connected clients
- Normally transitions to SearchingHost when session ends or by user request
- If `force_host` is enabled, remains in Host state permanently

**State Transitions:**

```text
Normal Mode (force_host = false):

Node Start
    |
    v
SearchingHost
    |
    ├─> Host Found ──> Client ──> (Monitor host liveness)
    |                      |
    |                      v
    |            (Host disconnects)
    |                      |
    └─> No Hosts ─────────┴────> Host
                                   |
                                   v
                         (Session ends/user stop)
                                   |
                                   v
                            SearchingHost


Force Host Mode (force_host = true):

Node Start ──> Host (permanent, no transitions)
```


## Architecture

### Module Organization

```
zenoh-arena/             (workspace root)
├── Cargo.toml          (workspace manifest)
├── zenoh-arena/        (library crate)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs              // Public API and re-exports
│   │   ├── config.rs           // Configuration types  
│   │   ├── types.rs            // Core types (NodeId, NodeInfo, NodeRole, StateUpdate)
│   │   ├── node.rs             // Node - main interface for host/client behavior
│   │   ├── engine.rs           // GameEngine trait
│   │   ├── network/
│   │   │   ├── mod.rs          // Network layer coordinator
│   │   │   ├── keyexpr.rs      // Key expression builder
│   │   │   ├── discovery.rs    // Host discovery using Queryable/get()
│   │   │   ├── connection.rs   // Connection handshake (Query/Reply)
│   │   │   ├── liveliness.rs   // Liveliness token management
│   │   │   └── pubsub.rs       // Publisher/Subscriber setup
│   │   └── error.rs            // Error types
├── z_bonjour/          (minimal example for API verification)
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
└── z_tetris/           (full game example)
    ├── Cargo.toml
    └── src/
        └── main.rs
```

## Basic Types

### Node Configuration

```rust
/// Configuration for a Node
pub struct NodeConfig {
    /// Optional node name (auto-generated if None)
    pub node_name: Option<String>,
    
    /// Zenoh configuration
    pub zenoh_config: zenoh::Config,
    
    /// Host discovery timeout (in milliseconds)
    pub discovery_timeout_ms: u64,
    
    /// Random jitter range for discovery timeout (0.0 - 1.0)
    pub discovery_jitter: f64,
    
    /// Maximum number of clients per host (None = unlimited)
    pub max_clients: Option<usize>,
    
    /// Whether to force host mode (blocks Searching and Client states)
    pub force_host: bool,
    
    /// Key expression prefix for arena communication
    pub keyexpr_prefix: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_name: None,
            zenoh_config: zenoh::Config::default(),
            discovery_timeout_ms: 5000,
            discovery_jitter: 0.3,
            max_clients: Some(4),
            force_host: false,
            keyexpr_prefix: "zenoh/arena".to_string(),
        }
    }
}
```

### Node Identity

```rust
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
    pub fn generate() -> Self;
    
    /// Create from a specific name (must be unique and keyexpr-compatible)
    /// Returns error if name contains invalid characters
    pub fn from_name(name: String) -> Result<Self, ArenaError>;
    
    /// Get the string representation
    pub fn as_str(&self) -> &str;
    
    /// Validate that a string can be used as NodeId (single keyexpr chunk)
    fn validate(s: &str) -> Result<(), ArenaError>;
}

/// Node state information
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub id: NodeId,
    pub role: NodeRole,
    pub connected_since: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    Client,
    Host,
}
```

### Node State (Internal)

```rust
/// Current state of a Node (internal implementation detail)
#[derive(Debug)]
pub(crate) enum NodeState<E> {
    /// Searching for available hosts
    SearchingHost,
    
    /// Connected as client to a host
    Client { 
        host_id: NodeId,
    },
    
    /// Acting as host
    Host {
        is_accepting: bool,
        connected_clients: Vec<NodeId>,
        engine: E,  // Game engine stored in Host state
    },
}

impl<E> NodeState<E> {
    pub fn is_host(&self) -> bool;
    pub fn is_client(&self) -> bool;
    pub fn is_accepting_clients(&self) -> bool;
    pub fn client_count(&self) -> Option<usize>;
}
```

**Note**: NodeState is not exposed in the public API. It's an internal implementation detail of the Node.

### Game Engine Integration

```rust
/// Trait for game engine integration
/// 
/// The engine runs only on the host node and processes actions from clients
pub trait GameEngine: Send + Sync {
    /// Action type from user/client
    type Action: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send;
    
    /// State type sent to clients
    type State: zenoh_ext::Serialize + zenoh_ext::Deserialize + Send + Clone;
    
    /// Initialize the game engine
    fn initialize(&mut self) -> Result<Self::State, Box<dyn std::error::Error>>;
    
    /// Process an action and return new state
    fn process_action(
        &mut self,
        action: Self::Action,
        client_id: &NodeId,
    ) -> Result<Self::State, Box<dyn std::error::Error>>;
    
    /// Get current state
    fn current_state(&self) -> Self::State;
    
    /// Tick/update game state (for time-based games)
    fn tick(&mut self, delta_ms: u64) -> Option<Self::State>;
    
    /// Client connected notification
    fn client_connected(&mut self, client_id: &NodeId);
    
    /// Client disconnected notification
    fn client_disconnected(&mut self, client_id: &NodeId);
    
    /// Check if game session has ended
    fn is_session_ended(&self) -> bool;
}
```

### Node API

```rust
/// Main Node interface - manages host/client behavior and game sessions
/// 
/// A Node is autonomous and manages its own role, connections, and game state.
/// There is no central "Arena" - each node has its local view of the network.
pub struct Node<E: GameEngine, F: Fn() -> E> {
    id: NodeId,
    config: NodeConfig,
    state: NodeState<E>,  // Internal state, not exposed in public API
    session: Arc<zenoh::Session>,
    get_engine: F,  // Engine factory - called when transitioning to host mode
}

impl<E: GameEngine, F: Fn() -> E> Node<E, F> {
    /// Create a new Node instance
    /// 
    /// `get_engine` is a factory function that creates an engine when needed.
    /// If force_host is enabled, the engine is created immediately and node starts in Host state.
    /// Otherwise, node starts in SearchingHost state.
    pub async fn new(config: NodeConfig, get_engine: F) -> Result<Self, ArenaError>;
    
    /// Run the node state machine
    /// 
    /// This is the main event loop that manages state transitions.
    /// Returns error if force_host is enabled but node is not in Host state.
    pub async fn run(&mut self) -> Result<(), ArenaError>;
    
    /// Get node ID
    pub fn id(&self) -> &NodeId;
    
    /// Get reference to Zenoh session
    pub fn session(&self) -> &Arc<zenoh::Session>;
    
    // Future API methods (to be implemented in later phases):
    // - send_action()
    // - subscribe_state()
    // - become_host()
    // - disconnect()
    // - set_accepting_clients()
    // - kick_client()
}
```

**Key API Design Changes from Original Design:**

1. **Engine Factory Pattern**: Instead of `Option<E>`, uses `F: Fn() -> E` closure
   - Allows creating new engine instances on demand
   - Supports both reusing existing engines and creating new ones
   
2. **State is Internal**: NodeState is not exposed in public API
   - Users interact through `run()` method and future action/state subscription APIs
   
3. **Simplified Startup**: Just `new()` and `run()`
   - No separate `start()` / `stop()` methods in Phase 1
   - Node begins in appropriate state based on `force_host` configuration
   
4. **Force Host Mode**: Blocks non-host states at configuration time
   - More predictable than runtime `auto_host` decision
   - Enforced by `run()` method returning error if state is invalid

### State Update API (Future Phases)

```rust
/// Receiver for game state updates (using flume, same as zenoh)
pub type StateReceiver<T> = flume::Receiver<StateUpdate<T>>;

#[derive(Debug, Clone)]
pub struct StateUpdate<T> {
    pub state: T,
    pub source: NodeId,
    pub timestamp: std::time::SystemTime,
}
```

### Network Protocol & Data Transmission

The library uses Zenoh's pub/sub API for game data transmission:

#### Discovery & Connection (Query/Queryable)
- **Host**: Declares `Queryable` on `<prefix>/discovery` to respond to discovery queries
- **Client**: Uses `get()` to query for available hosts
- **Connection handshake**: Query/reply pattern for join request/accept/confirm

#### Game Data Flow (Pub/Sub)

**Actions (Client → Host)**:
```rust
// Client side: Publisher for actions
// Publishes to: <prefix>/host/<host_id>/client/<client_id>/action
let action_publisher = session
    .declare_publisher(format!("{}/host/{}/client/{}/action", prefix, host_id, client_id))
    .await?;

// Host side: Subscriber for all client actions
// Subscribes to: <prefix>/host/<host_id>/client/*/action
let action_subscriber = session
    .declare_subscriber(format!("{}/host/{}/client/*/action", prefix, host_id))
    .await?;

// Host receives actions from all clients
while let Ok(sample) = action_subscriber.recv_async().await {
    let action: Action = zenoh_ext::z_deserialize(sample.payload())?;
    let client_id = extract_client_id_from_keyexpr(sample.key_expr());
    engine.process_action(action, &client_id)?;
}
```

**States (Host → Clients)**:
```rust
// Host side: Publisher for state updates
// Publishes to: <prefix>/host/<host_id>/state
let state_publisher = session
    .declare_publisher(format!("{}/host/{}/state", prefix, host_id))
    .await?;

// Broadcast state to all clients
let state = engine.current_state();
let payload = zenoh_ext::z_serialize(&state)?;
state_publisher.put(payload).await?;

// Client side: Subscriber for state updates
// Subscribes to: <prefix>/host/<host_id>/state
let state_subscriber = session
    .declare_subscriber(format!("{}/host/{}/state", prefix, host_id))
    .await?;

// Client receives state updates from host
while let Ok(sample) = state_subscriber.recv_async().await {
    let state: State = zenoh_ext::z_deserialize(sample.payload())?;
    // Forward to application via flume channel
    state_channel.send(state)?;
}
```

#### Key Expression Patterns

```rust
/// Key expression patterns
/// 
/// Note: NodeId is guaranteed to be a valid single-chunk keyexpr,
/// so it can be safely used in keyexpr construction via format/join
struct KeyExpressions {
    /// Discovery: <prefix>/discovery
    discovery: String,
    
    /// Host-specific: <prefix>/host/<host_id>
    host: String,
    
    /// Host join query: <prefix>/host/<host_id>/join
    host_join: String,
    
    /// Host state pub/sub: <prefix>/host/<host_id>/state
    host_state: String,
    
    /// Client action pub/sub: <prefix>/host/<host_id>/client/<client_id>/action
    client_action: String,
    
    /// Liveliness token keyexpr: <prefix>/node/<node_id>
    /// Note: Liveliness tokens are stored in Zenoh's hermetic @ namespace
    /// automatically by the liveliness API, separate from regular pub/sub data.
    /// We use a regular keyexpr which Zenoh internally maps to @<keyexpr>
    liveliness: String,
}

impl KeyExpressions {
    /// Create keyexpr patterns using zenoh::key_expr::KeyExpr::join
    /// Since NodeId is validated as single-chunk keyexpr, joining is safe
    fn new(prefix: &str, node_id: &NodeId) -> Result<Self, ArenaError>;
}
```

#### Connection Messages (Query/Reply)

```rust
/// Messages used during discovery and connection (Query/Reply pattern)
#[derive(Debug, Clone)]
enum ConnectionMessage<State> 
where
    State: zenoh_ext::Serialize + zenoh_ext::Deserialize,
{
    /// Client -> Host: Request to join
    JoinRequest {
        client_id: NodeId,
    },
    
    /// Host -> Client: Accept join request with initial state
    JoinAccept {
        host_id: NodeId,
        initial_state: State,
    },
    
    /// Host -> Client: Reject join request
    JoinReject {
        host_id: NodeId,
        reason: String,
    },
    
    /// Client -> Host: Confirm connection
    JoinConfirm {
        client_id: NodeId,
    },
    
    /// Client -> Hosts: Discovery query
    DiscoveryQuery,
    
    /// Host -> Client: Discovery response
    DiscoveryResponse {
        host_id: NodeId,
        is_accepting: bool,
        current_clients: usize,
        max_clients: Option<usize>,
    },
}
```

**Data Flow Summary**:
1. **Discovery**: Client uses Zenoh `get()` (query) to find hosts via `Queryable`
2. **Connection**: Request/Accept/Confirm handshake via query/reply
3. **Game Actions**: Clients publish actions, host subscribes with wildcard pattern
4. **Game States**: Host publishes states, all clients subscribe
5. **Liveliness**: Automatic tracking via Zenoh liveliness API

### Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum ArenaError {
    #[error("Zenoh error: {0}")]
    Zenoh(#[from] zenoh::Error),
    
    #[error("Node name conflict: {0}")]
    NodeNameConflict(String),
    
    #[error("Invalid node name: {0}. Must be a valid single-chunk keyexpr (no /, *, $, ?, #, @)")]
    InvalidNodeName(String),
    
    #[error("Invalid state transition: from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: NodeState,
        to: NodeState,
    },
    
    #[error("Host not found")]
    HostNotFound,
    
    #[error("Connection rejected: {0}")]
    ConnectionRejected(String),
    
    #[error("Not in host mode")]
    NotHost,
    
    #[error("Not in client mode")]
    NotClient,
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Engine error: {0}")]
    Engine(Box<dyn std::error::Error + Send + Sync>),
    
    #[error("Timeout: {0}")]
    Timeout(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, ArenaError>;
```

## Implementation Phases

### Phase 1: Core Infrastructure
- [ ] Basic types and configuration
- [ ] Node identity and state management
- [ ] Error handling
- [ ] Zenoh session initialization

### Phase 2: Network Layer

- [ ] Key expression management and construction
- [ ] Message serialization/deserialization (zenoh-ext)
- [ ] Publisher/Subscriber setup for actions and states
- [ ] Liveliness token management
- [ ] Query/Queryable for discovery

### Phase 3: Node Discovery & Connection

- [ ] Host discovery using Queryable/get()
- [ ] Client join protocol
- [ ] Connection confirmation
- [ ] Host acceptance logic
- [ ] Liveliness monitoring

### Phase 4: Node Role Management

- [ ] Client mode implementation
  - Host searching
  - Connection management
  - Action publishing
  - State subscription
  - Host liveliness monitoring
- [ ] Host mode implementation
  - Discovery queryable setup
  - Client management
  - Engine coordination
  - State broadcasting
  - Action subscription (wildcard pattern)

### Phase 5: State Transitions
- [ ] Automatic role switching
- [ ] Empty host behavior
- [ ] Host closing logic
- [ ] Randomized timeouts

### Phase 6: Game Engine Integration
- [ ] Engine trait implementation helpers
- [ ] Action processing pipeline
- [ ] State synchronization
- [ ] Session lifecycle management

### Phase 7: API Verification - z_bonjour Example
- [ ] Simple game engine for API verification
- [ ] Action type: `Bonjour` (single variant enum)
- [ ] State type: `Bonsoir` (single variant enum)
- [ ] Engine responds with `Bonsoir` to every `Bonjour` action
- [ ] Minimal terminal UI to send actions and display state
- [ ] Verify host discovery, connection, and state synchronization

### Phase 8: Testing & Full Examples
- [ ] Unit tests
- [ ] Integration tests
- [ ] z_tetris full game application
- [ ] Documentation and examples

## Key Design Decisions

### 1. Async Runtime
- Use `tokio` as the async runtime (aligns with Zenoh)
- All public APIs are async

### 2. State Management

- Use `Arc<RwLock<T>>` for shared state
- Use `flume` channels for state updates (same as Zenoh uses internally)
- `flume` provides better performance and simpler API than `tokio::sync::mpsc`

### 3. Serialization

- Use Zenoh's native serialization via `zenoh-ext`
- Action/State types must implement `zenoh_ext::Serialize` and `zenoh_ext::Deserialize`
- Use `zenoh_ext::z_serialize` and `zenoh_ext::z_deserialize` functions
```

### 4. Error Handling
- Use `thiserror` for error types
- Propagate errors rather than panicking
- Provide detailed error context

### 5. Thread Safety
- All types are `Send + Sync` where required
- Engine runs on host only, isolated from network layer

### 6. Discovery Protocol

- Use Zenoh Queryable for host discovery
- Clients use `get()` to query available hosts
- Randomized timeout prevents thundering herd
- Connection handshake: Query/reply pattern for join request/accept/confirm

### 7. Game Data Transmission

**Actions (Client → Host)**:
- Each client declares a Publisher on `<prefix>/host/<host_id>/client/<client_id>/action`
- Host declares a Subscriber with wildcard `<prefix>/host/<host_id>/client/*/action`
- Host receives all client actions through single subscriber
- Actions are deserialized and forwarded to game engine

**States (Host → Clients)**:
- Host declares a Publisher on `<prefix>/host/<host_id>/state`
- Each client declares a Subscriber on `<prefix>/host/<host_id>/state`
- Host broadcasts state updates to all connected clients
- Clients receive states and forward to application via flume channels

**Benefits**:
- Efficient multicast distribution of state updates
- Scalable action collection from multiple clients
- Low-latency pub/sub pattern
- Automatic Zenoh routing optimization

### 8. Liveliness

- Each node declares a liveliness token using `session.liveliness().declare_token(keyexpr)`
- Liveliness tokens are automatically stored in Zenoh's hermetic `@` namespace
- This namespace is separate from regular pub/sub data - prevents interference
- Clients monitor host liveliness using `session.liveliness().declare_subscriber(keyexpr)`
- Clients subscribe to liveliness changes to detect host disconnection
- Automatic reconnection on host failure

**Important**: The liveliness API abstracts the `@` namespace - you use regular keyexprs, and Zenoh handles the namespace mapping internally. No special prefixes needed in application code.

## Usage Example (Conceptual)

```rust
use zenoh_arena::{Node, NodeConfig, GameEngine, NodeId};

// Define your game engine
struct MyGameEngine {
    // game state
}

impl GameEngine for MyGameEngine {
    type Action = MyAction;
    type State = MyState;
    
    // Implement trait methods...
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure node
    let config = NodeConfig::default();
    
    // Create engine (None if this node should only be client)
    let engine = Some(MyGameEngine::new());
    
    // Create node
    let mut node = Node::new(config, engine).await?;
    
    // Subscribe to state updates
    let mut state_rx = node.subscribe_state();
    let mut node_state_rx = node.subscribe_node_state();
    
    // Start node (automatic host discovery/role negotiation)
    node.start().await?;
    
    // Main loop
    loop {
        tokio::select! {
            Ok(state_update) = state_rx.recv_async() => {
                // Handle game state update
                println!("New state from {}: {:?}", state_update.source, state_update.state);
            }
            Ok(node_state) = node_state_rx.recv_async() => {
                // Handle node state change
                println!("Node state: {:?}", node_state);
            }
            // Handle user input, send actions, etc.
        }
    }
}
```

## Dependencies

**Workspace Cargo.toml**:
```toml
[workspace]
members = ["zenoh-arena", "z_bonjour", "z_tetris"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
zenoh = "1.6"
zenoh-ext = "1.6"
tokio = { version = "1", features = ["full"] }
flume = "0.11"
thiserror = "1"
uuid = { version = "1", features = ["v4"] }
bs58 = "0.5"
rand = "0.8"
tracing = "0.1"
```

**Library crate dependencies** (`zenoh-arena/Cargo.toml`):
- `zenoh` - Core networking and key expressions
- `zenoh-ext` - Serialization (Serialize/Deserialize traits, z_serialize/z_deserialize)
- `tokio` - Async runtime
- `flume` - Channels for state updates (same as Zenoh uses internally)
- `thiserror` - Error handling
- `uuid` - Node ID generation (for base58 encoding)
- `bs58` - Base58 encoding for keyexpr-safe node IDs
- `rand` - Randomized timeouts
- `tracing` - Logging/diagnostics

**Example crates** (`z_bonjour/Cargo.toml`, `z_tetris/Cargo.toml`):
- `zenoh-arena` (from workspace)
- `tokio` - For async main
- Additional UI/game dependencies as needed

## Open Questions & Future Enhancements

1. **Multi-host support**: Should clients be able to query multiple hosts and choose based on criteria (latency, player count)?
2. **Host migration**: Should a client automatically become host and transfer state when the current host disconnects?
3. **Spectator mode**: Should there be a read-only observer role that doesn't participate in the game?
4. **Matchmaking**: Should there be optional matchmaking to help clients find suitable hosts?
5. **Security**: Authentication and encryption considerations for private games
6. **Metrics**: Built-in latency/performance monitoring for debugging
7. **Backpressure**: How to handle slow clients or network congestion
8. **Partial state updates**: Delta encoding for large states to reduce bandwidth

## Architecture Summary

**Key Differences from Traditional Client-Server:**

- **No Central Arena**: Each `Node` is autonomous and manages its own view of the network
- **P2P Discovery**: Hosts advertise themselves via Zenoh queryables, clients discover via queries
- **Role Flexibility**: Any node can be a host or client, roles can change dynamically
- **Local State Management**: Each node maintains its local view of connected nodes
- **Engine on Host**: Game logic runs only on the host node, clients are thin
- **Pub/Sub Data Flow**: Efficient multicast state distribution, wildcard action collection

This design embraces Zenoh's P2P nature - there's no centralized "arena server". Each node is a peer that can discover others, negotiate roles, and participate in game sessions.

## Testing Strategy

### Unit Tests
- Configuration parsing and validation
- State transition logic
- Message serialization/deserialization
- Key expression generation

### Integration Tests
- Host discovery with multiple instances
- Client-host connection lifecycle
- State synchronization
- Liveliness detection and failover
- Empty host behavior

### Example Application

#### z_bonjour - API Verification Example

The simplest possible game to verify the API:

**Purpose**: Verify core Arena functionality with minimal complexity

**Game Types**:
```rust
#[derive(Debug, Clone)]
enum BonjourAction {
    Bonjour,
}

#[derive(Debug, Clone)]
enum BonsoirState {
    Bonsoir,
}

struct BonjourEngine;

impl GameEngine for BonjourEngine {
    type Action = BonjourAction;
    type State = BonsoirState;
    
    fn initialize(&mut self) -> Result<Self::State, Box<dyn std::error::Error>> {
        Ok(BonsoirState::Bonsoir)
    }
    
    fn process_action(
        &mut self,
        action: Self::Action,
        _client_id: &NodeId,
    ) -> Result<Self::State, Box<dyn std::error::Error>> {
        match action {
            BonjourAction::Bonjour => Ok(BonsoirState::Bonsoir),
        }
    }
    
    fn current_state(&self) -> Self::State {
        BonsoirState::Bonsoir
    }
    
    // ... other trait methods with minimal implementations
}

// Implement zenoh-ext serialization for simple types
impl zenoh_ext::Serialize for BonjourAction {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), std::io::Error> {
        // Minimal serialization - just write a tag byte
        zenoh_ext::z_serialize(&0u8, writer)
    }
}

impl zenoh_ext::Deserialize for BonjourAction {
    fn deserialize<R: std::io::Read>(reader: R) -> Result<Self, std::io::Error> {
        let _: u8 = zenoh_ext::z_deserialize(reader)?;
        Ok(BonjourAction::Bonjour)
    }
}

// Similar implementations for BonsoirState
```

**Terminal UI**:

- Display current node state (Searching/Client/Host)
- Display connected clients (if host)
- Press any key to send `Bonjour` action
- Display received `Bonsoir` state
- Press 'q' to quit

**Verification Goals**:

- Host discovery works correctly
- Client-host connection established
- Actions sent from client reach host engine
- State updates broadcast to all clients
- Multiple instances can find each other
- Liveliness detection and reconnection
- Host/client role transitions

#### z_tetris - Full Game Example

- z_tetris serves as both example and integration test
- Real-world usage patterns and edge cases
- Uses gametetris-rs library as game engine
- Demonstrates complex state synchronization
