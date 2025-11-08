# zenoh-arena Library Design Document

## Overview

The `zenoh-arena` library is a peer-to-peer network framework for simple game applications built on top of the Zenoh network library. It provides automatic host/client role negotiation, connection management, and state synchronization for distributed game sessions.

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

## Core Concepts

### Application Roles

- **Client**: Searches for available hosts and connects to them
- **Host**: Accepts client connections and runs the game engine
- **Empty Host**: A host with no connected clients
- **Open Host**: A host accepting new client connections
- **Closed Host**: A host that has stopped accepting new clients

### State Transitions

```
Initial State (Client)
    |
    v
Searching for Hosts
    |
    ├─> Host Found ──> Connected Client
    |                       |
    |                       v
    |                  (On disconnect/loss)
    |                       |
    └─> No Hosts Found ─────┴──> Become Host
                                      |
                                      v
                                 Open/Closed Host
                                      |
                                      v
                            (On session end/empty/request)
                                      |
                                      v
                                Back to Searching
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
│   │   ├── types.rs            // Core types and traits
│   │   ├── arena.rs            // Main Arena coordinator
│   │   ├── node.rs             // Node identity and state
│   │   ├── host.rs             // Host role implementation
│   │   ├── client.rs           // Client role implementation
│   │   ├── network/
│   │   │   ├── mod.rs          // Network layer coordinator
│   │   │   ├── discovery.rs    // Host discovery using Queryable
│   │   │   ├── connection.rs   // Connection management
│   │   │   ├── liveliness.rs   // Liveliness token management
│   │   │   └── transport.rs    // Message serialization/transport
│   │   ├── engine/
│   │   │   ├── mod.rs          // Game engine integration
│   │   │   └── adapter.rs      // Engine adapter trait
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

### Core Configuration

```rust
/// Main configuration for the Arena
pub struct ArenaConfig {
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
    
    /// Whether to automatically become host if no hosts found
    pub auto_host: bool,
    
    /// Key expression prefix for arena communication
    pub keyexpr_prefix: String,
}

impl Default for ArenaConfig {
    fn default() -> Self {
        Self {
            node_name: None,
            zenoh_config: zenoh::Config::default(),
            discovery_timeout_ms: 5000,
            discovery_jitter: 0.3,
            max_clients: Some(4),
            auto_host: true,
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

### Application State

```rust
/// Current state of the Arena
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArenaState {
    /// Initializing
    Initializing,
    
    /// Searching for available hosts
    SearchingHost,
    
    /// Connected as client to a host
    ConnectedClient { host_id: NodeId },
    
    /// Acting as host
    Host {
        is_open: bool,
        connected_clients: Vec<NodeId>,
    },
    
    /// Transitioning between states
    Transitioning,
    
    /// Stopped/Closed
    Stopped,
}

impl ArenaState {
    pub fn is_host(&self) -> bool;
    pub fn is_client(&self) -> bool;
    pub fn is_empty_host(&self) -> bool;
    pub fn is_open_host(&self) -> bool;
}
```

### Game Engine Integration

```rust
/// Trait for game engine integration
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

### Arena API

```rust
/// Main Arena coordinator
pub struct Arena<E: GameEngine> {
    config: ArenaConfig,
    state: Arc<RwLock<ArenaState>>,
    node_id: NodeId,
    engine: Option<E>,
    // Internal fields...
}

impl<E: GameEngine> Arena<E> {
    /// Create a new Arena instance
    pub async fn new(config: ArenaConfig, engine: E) -> Result<Self, ArenaError>;
    
    /// Start the arena (begins host discovery)
    pub async fn start(&mut self) -> Result<(), ArenaError>;
    
    /// Stop the arena
    pub async fn stop(&mut self) -> Result<(), ArenaError>;
    
    /// Get current arena state
    pub fn state(&self) -> ArenaState;
    
    /// Get node ID
    pub fn node_id(&self) -> &NodeId;
    
    /// Send an action (as client or local processing)
    pub async fn send_action(&self, action: E::Action) -> Result<(), ArenaError>;
    
    /// Subscribe to state updates
    pub fn subscribe_state(&self) -> StateReceiver<E::State>;
    
    /// Subscribe to arena state changes
    pub fn subscribe_arena_state(&self) -> ArenaStateReceiver;
    
    /// Manually close host (if in host mode)
    pub async fn close_host(&mut self) -> Result<(), ArenaError>;
    
    /// Manually disconnect (if in client mode)
    pub async fn disconnect(&mut self) -> Result<(), ArenaError>;
    
    /// Set host open/closed status
    pub async fn set_host_open(&mut self, open: bool) -> Result<(), ArenaError>;
}

/// Receiver for game state updates
pub type StateReceiver<T> = tokio::sync::mpsc::Receiver<StateUpdate<T>>;

/// Receiver for arena state changes
pub type ArenaStateReceiver = tokio::sync::mpsc::Receiver<ArenaState>;

#[derive(Debug, Clone)]
pub struct StateUpdate<T> {
    pub state: T,
    pub source: NodeId,
    pub timestamp: std::time::SystemTime,
}
```

### Network Protocol

```rust
/// Internal network message types
/// Note: Implements zenoh_ext::Serialize and zenoh_ext::Deserialize
#[derive(Debug, Clone)]
enum NetworkMessage<Action, State> 
where
    Action: zenoh_ext::Serialize + zenoh_ext::Deserialize,
    State: zenoh_ext::Serialize + zenoh_ext::Deserialize,
{
    /// Client -> Host: Request to join
    JoinRequest {
        client_id: NodeId,
    },
    
    /// Host -> Client: Accept join request
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
    
    /// Client -> Host: Game action
    Action {
        client_id: NodeId,
        action: Action,
    },
    
    /// Host -> Clients: Game state update
    StateUpdate {
        state: State,
        timestamp: u64,
    },
    
    /// Either -> Either: Disconnect notification
    Disconnect {
        node_id: NodeId,
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

/// Key expression patterns
/// 
/// Note: NodeId is guaranteed to be a valid single-chunk keyexpr,
/// so it can be safely used in keyexpr construction via format/join
struct KeyExpressions {
    /// Discovery: <prefix>/discovery
    discovery: String,
    
    /// Host-specific: <prefix>/host/<host_id>
    host: String,
    
    /// Host join: <prefix>/host/<host_id>/join
    host_join: String,
    
    /// Host state: <prefix>/host/<host_id>/state
    host_state: String,
    
    /// Client-specific: <prefix>/host/<host_id>/client/<client_id>
    client: String,
    
    /// Liveliness: <prefix>/liveliness/<node_id>
    liveliness: String,
}

impl KeyExpressions {
    /// Create keyexpr patterns using zenoh::key_expr::KeyExpr::join
    /// Since NodeId is validated as single-chunk keyexpr, joining is safe
    fn new(prefix: &str, node_id: &NodeId) -> Result<Self, ArenaError>;
}
```

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
        from: ArenaState,
        to: ArenaState,
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
- [ ] Key expression management
- [ ] Message serialization/deserialization
- [ ] Liveliness token management
- [ ] Basic put/get operations

### Phase 3: Discovery & Connection
- [ ] Host discovery using Queryable
- [ ] Client join protocol
- [ ] Connection confirmation
- [ ] Host acceptance logic

### Phase 4: Role Implementation
- [ ] Client role implementation
  - Host searching
  - Connection management
  - Action forwarding
  - State reception
- [ ] Host role implementation
  - Queryable setup
  - Client management
  - Engine coordination
  - State broadcasting

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
- Use `tokio::sync::mpsc` channels for state updates

### 3. Serialization

- Use Zenoh's native serialization via `zenoh-ext`
- Action/State types must implement `zenoh_ext::Serialize` and `zenoh_ext::Deserialize`
- Use `zenoh_ext::z_serialize` and `zenoh_ext::z_deserialize` functions

### 4. Error Handling
- Use `thiserror` for error types
- Propagate errors rather than panicking
- Provide detailed error context

### 5. Thread Safety
- All types are `Send + Sync` where required
- Engine runs on host only, isolated from network layer

### 6. Discovery Protocol
- Use Zenoh Queryable for host discovery
- Randomized timeout prevents thundering herd
- Two-phase commit: request -> accept -> confirm

### 7. Liveliness
- Each node declares liveliness token
- Clients monitor host liveliness
- Automatic reconnection on host failure

## Usage Example (Conceptual)

```rust
use zenoh_arena::{Arena, ArenaConfig, GameEngine, NodeId};

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
    // Configure arena
    let config = ArenaConfig::default();
    
    // Create engine
    let engine = MyGameEngine::new();
    
    // Create arena
    let mut arena = Arena::new(config, engine).await?;
    
    // Subscribe to state updates
    let mut state_rx = arena.subscribe_state();
    let mut arena_state_rx = arena.subscribe_arena_state();
    
    // Start arena (automatic host discovery/role negotiation)
    arena.start().await?;
    
    // Main loop
    loop {
        tokio::select! {
            Some(state) = state_rx.recv() => {
                // Handle game state update
                println!("New state: {:?}", state);
            }
            Some(arena_state) = arena_state_rx.recv() => {
                // Handle arena state change
                println!("Arena state: {:?}", arena_state);
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

1. **Multi-host support**: Should clients be able to query multiple hosts and choose?
2. **Host migration**: Should state transfer to a new host when old host disconnects?
3. **Spectator mode**: Should there be a read-only observer role?
4. **Matchmaking**: Should there be a matchmaking service for pairing clients?
5. **Security**: Authentication and encryption considerations
6. **Metrics**: Built-in latency/performance monitoring
7. **Backpressure**: How to handle slow clients or network congestion
8. **Partial state updates**: Delta encoding for large states

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
- Display current arena state (Searching/Client/Host)
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
