# zenoh-arena Library Design Document

**Current Implementation Status**: Phase 1 Complete
- ✅ Core infrastructure and types
- ✅ Node state management (internal)
- ✅ Builder pattern API via SessionExt
- ✅ Command/step pattern for node control
- ✅ GameEngine trait (simplified)
- ✅ z_bonjour example working
- 🔄 Network layer (Phase 2-4) - in planning

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

Internal configuration structure (not exposed in public API):

- **node_name**: Optional custom name (auto-generated UUID-based if not provided)
- **zenoh_config**: Zenoh session configuration
- **discovery_timeout_ms**: Timeout for host discovery queries (default: 5000ms)
- **discovery_jitter**: Random jitter factor for timeouts (0.0-1.0, default: 0.3)
- **max_clients**: Maximum clients per host (None = unlimited, default: Some(4))
- **force_host**: Whether to force host mode permanently (default: false)
- **keyexpr_prefix**: Prefix for all arena key expressions (default: "zenoh/arena")
- **step_timeout_ms**: Timeout for step() method (default: 100ms)

### Node Identity

**NodeId**: Unique node identifier that must be a valid single-chunk Zenoh key expression

Requirements:
- Non-empty UTF-8 string
- Cannot contain: `/` (separator), `*` (wildcard), `$` (DSL), `?` `#` (reserved), `@` (verbatim)
- Must be a single chunk (no path separators)

Methods:
- `generate()` - Creates auto-generated ID using base58-encoded UUID
- `from_name(name)` - Creates ID from custom name (validates keyexpr compatibility)
- `as_str()` - Returns string representation

**NodeInfo**: Metadata about a node
- `id` - Node identifier
- `role` - Current role (Client or Host)
- `connected_since` - Timestamp of node creation or connection

**NodeRole**: Enumeration of node roles
- `Client` - Connected to a host
- `Host` - Running game engine and accepting clients

### Node State (Internal)

Internal state representation (not exposed in public API):

**NodeStateInternal<E>**: Enum with three variants
- `SearchingHost` - Looking for available hosts
- `Client { host_id }` - Connected to specified host
- `Host { is_accepting, connected_clients, engine }` - Hosting with game engine

Helper methods:
- `is_host()` - Check if currently hosting
- `is_client()` - Check if currently a client
- `is_accepting_clients()` - Check if host is accepting new clients
- `client_count()` - Get number of connected clients (None if not host)

**Note**: NodeStateInternal is not exposed in the public API. Applications interact via NodeState (public view without engine).

### Game Engine Integration

**Current Implementation** (Phase 1 - Simplified):

**GameEngine Trait**: Interface for game logic integration

Required associated types:
- `Action` - Type for player actions (must be Zenoh serializable)
- `State` - Type for game state (must be Zenoh serializable and cloneable)

Required method:
- `process_action(&mut self, action: Action, client_id: &NodeId) -> Result<State>` - Process action and return new state

**Future Extensions** (may be implemented in later phases):

Additional methods that could be added to GameEngine trait:
- `initialize()` - Initialize engine and return initial state
- `current_state()` - Get current state without processing action
- `tick(delta_ms)` - Time-based state updates for real-time games
- `client_connected(client_id)` - Notification when client joins
- `client_disconnected(client_id)` - Notification when client leaves
- `is_session_ended()` - Check if game session has completed

### Node API

**Current Implementation** (Phase 1):

**Node Structure**: Main interface for node management
- Generic over engine type `E` and factory function type `F`
- Contains: node ID, internal config, internal state, Zenoh session, engine factory, command channels
- Internal state stores the game engine (only accessible when in Host mode)

**Public Methods**:
- `step()` - Execute one step of state machine, returns `Option<NodeStatus<State>>`
  - Returns `Some(status)` with current state and optional game state
  - Returns `None` when Stop command received
- `id()` - Get node identifier
- `session()` - Get reference to Zenoh session
- `sender()` - Get command sender for sending actions/stop commands

**Builder Pattern** (via SessionExt trait):
- `declare_arena_node(get_engine)` - Start building a node with engine factory
- Builder methods:
  - `name(String)` - Set custom node name
  - `force_host(bool)` - Force node to always be a host
  - `step_timeout_ms(u64)` - Set timeout for step() method
- Await the builder to create the node

**NodeCommand Enum**: Commands that can be sent to node
- `GameAction(action)` - Send action to be processed by engine
- `Stop` - Stop the node's event loop

**NodeStatus Struct**: Returned by step() method
- `state: NodeState` - Current node state (Searching/Client/Host)
- `game_state: Option<State>` - Optional game state from engine

**NodeState Enum** (public view):
- `SearchingHost` - Looking for hosts
- `Client { host_id }` - Connected to specified host
- `Host { is_accepting, connected_clients }` - Hosting with client list

**Key API Design Decisions:**

1. **Builder Pattern**: Follows Zenoh-style conventions with `SessionExt` trait extension
   - Configuration via builder methods, not exposed struct
   
2. **Step-based Execution**: User controls event loop via `step()` calls
   - Returns status on each step
   - Integrates with user's async event handling
   
3. **Command Channel**: Decoupled command sending
   - Commands sent via sender, processed in step()
   - Thread-safe command submission
   
4. **State is Internal**: Engine stored in internal state only
   - Public API shows state without engine details
   - Engine lifecycle managed internally

5. **Engine Factory Pattern**: Function closure creates engine instances
   - Supports creating fresh engines on demand
   - Allows engine reuse or recreation

### State Update API (Future Phases)

**StateReceiver**: Type alias for game state update channel (using flume)

**StateUpdate**: Structure containing:
- `state` - The game state
- `source` - NodeId of state source
- `timestamp` - When state was generated

### Network Protocol & Data Transmission

The library uses Zenoh's pub/sub API for game data transmission:

#### Discovery & Connection (Query/Queryable)

- **Host**: Declares `Queryable` on `<prefix>/discovery` to respond to discovery queries
- **Client**: Uses `get()` to query for available hosts
- **Connection handshake**: Query/reply pattern for join request/accept/confirm

#### Game Data Flow (Pub/Sub)

**Actions (Client → Host)**:

- Client publishes actions to: `<prefix>/host/<host_id>/client/<client_id>/action`
- Host subscribes to all client actions with wildcard: `<prefix>/host/<host_id>/client/*/action`
- Host receives actions from all clients through single subscriber
- Actions are deserialized and forwarded to game engine with client ID

**States (Host → Clients)**:

- Host publishes state updates to: `<prefix>/host/<host_id>/state`
- Each client subscribes to: `<prefix>/host/<host_id>/state`
- Host broadcasts state updates to all connected clients
- Clients receive states and forward to application via channels

#### Key Expression Patterns

**KeyExpressions Structure**: Manages all key expression patterns

Key patterns used:
- `<prefix>/discovery` - Host discovery queries
- `<prefix>/host/<host_id>` - Host-specific base
- `<prefix>/host/<host_id>/join` - Host join requests
- `<prefix>/host/<host_id>/state` - Host state broadcasts
- `<prefix>/host/<host_id>/client/<client_id>/action` - Client action publishing
- `<prefix>/node/<node_id>` - Liveliness tokens

Note: NodeId is guaranteed to be a valid single-chunk keyexpr, so it can be safely used in keyexpr construction. Liveliness tokens use regular keyexprs which Zenoh automatically maps to the hermetic `@` namespace.

#### Connection Messages (Query/Reply)

**ConnectionMessage Enum**: Messages for discovery and connection handshake

Message types:
- `JoinRequest { client_id }` - Client requests to join host
- `JoinAccept { host_id, initial_state }` - Host accepts client with initial state
- `JoinReject { host_id, reason }` - Host rejects client with reason
- `JoinConfirm { client_id }` - Client confirms connection established
- `DiscoveryQuery` - Client queries for available hosts
- `DiscoveryResponse { host_id, is_accepting, current_clients, max_clients }` - Host responds with availability info

**Data Flow Summary**:

1. **Discovery**: Client uses Zenoh `get()` (query) to find hosts via `Queryable`
2. **Connection**: Request/Accept/Confirm handshake via query/reply
3. **Game Actions**: Clients publish actions, host subscribes with wildcard pattern
4. **Game States**: Host publishes states, all clients subscribe
5. **Liveliness**: Automatic tracking via Zenoh liveliness API

### Error Handling

**ArenaError Enum**: Error types using `thiserror`

Error variants:
- `Zenoh(zenoh::Error)` - Zenoh library errors
- `NodeNameConflict(String)` - Node name already in use
- `InvalidNodeName(String)` - Invalid keyexpr in node name
- `InvalidStateTransition { from, to }` - Invalid state machine transition
- `HostNotFound` - No hosts available during discovery
- `ConnectionRejected(String)` - Host rejected join request
- `NotHost` - Operation requires host mode
- `NotClient` - Operation requires client mode
- `Serialization(String)` - Serialization/deserialization failure
- `Engine(Box<Error>)` - Game engine error
- `Timeout(String)` - Operation timed out
- `Internal(String)` - Internal consistency error

**Result Type**: `Result<T> = std::result::Result<T, ArenaError>`

## Implementation Phases

### Phase 1: Core Infrastructure ✅ COMPLETED

- [x] Basic types and configuration
- [x] Node identity and state management
- [x] Error handling
- [x] Zenoh session initialization
- [x] Builder pattern API via SessionExt trait
- [x] Step-based execution model
- [x] Command channel pattern
- [x] Simplified GameEngine trait

**Status**: Fully implemented. z_bonjour example demonstrates the API.

### Phase 2: Network Layer 🔄 IN PROGRESS

- [ ] Key expression management and construction
- [ ] Message serialization/deserialization (zenoh-ext)
- [ ] Publisher/Subscriber setup for actions and states
- [ ] Liveliness token management
- [ ] Query/Queryable for discovery

**Status**: Design documented, implementation pending.

### Phase 3: Node Discovery & Connection

- [ ] Host discovery using Queryable/get()
- [ ] Client join protocol
- [ ] Connection confirmation
- [ ] Host acceptance logic
- [ ] Liveliness monitoring

**Status**: Planned after Phase 2 completion.

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

**Status**: Depends on Phases 2 and 3. Currently only local host mode works.

### Phase 5: State Transitions

- [ ] Automatic role switching
- [ ] Empty host behavior
- [ ] Host closing logic
- [ ] Randomized timeouts

**Status**: Planned for future implementation.

### Phase 6: Game Engine Integration

- [x] Basic engine trait implementation (simplified)
- [ ] Extended engine trait with lifecycle methods
- [ ] Action processing pipeline (network)
- [ ] State synchronization (network)
- [ ] Session lifecycle management

**Status**: Basic trait complete. Network integration pending.

### Phase 7: API Verification - z_bonjour Example ✅ COMPLETED

- [x] Simple game engine for API verification
- [x] Action type: `BonjourAction` (single variant)
- [x] State type: `BonjourState` (counter)
- [x] Engine responds with incremented counter to each action
- [x] Terminal UI to send actions and display state
- [x] Verify local node management

**Status**: Working example in `z_bonjour/` directory. Network features pending.

### Phase 8: Testing & Full Examples

- [ ] Unit tests
- [ ] Integration tests (requires network layer)
- [ ] z_tetris full game application
- [ ] Documentation and examples

**Status**: Unit tests exist for basic types. Integration tests pending network layer.

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

The simplest possible application to verify the API:

**Purpose**: Verify core Arena functionality with minimal complexity

**Current Implementation**:

Types:
- `BonjourAction` - Simple action type (unit struct)
- `BonjourState` - Counter state containing `u64` value
- `BonjourEngine` - Engine that increments counter on each action

Implementation:
- Engine maintains internal counter state
- `process_action()` increments counter and returns new state
- Both action and state types implement Zenoh serialization traits

**Terminal UI**:

- Display current node state (Searching/Client/Host)
- Display node ID
- Press 'b' to send `BonjourAction`
- Display received `BonjourState` with counter value
- Press 'q' to quit

**Verification Goals** (Phase 1):

- [x] Builder pattern API works correctly
- [x] Node creation and initialization
- [x] Command channel accepts actions
- [x] step() method processes commands
- [x] Engine processes actions and returns state
- [x] NodeStatus displays correctly

**Future Verification Goals** (Phase 2+):

- [ ] Host discovery works correctly
- [ ] Client-host connection established
- [ ] Actions sent from client reach host engine
- [ ] State updates broadcast to all clients
- [ ] Multiple instances can find each other
- [ ] Liveliness detection and reconnection
- [ ] Host/client role transitions

#### z_tetris - Full Game Example

- z_tetris serves as both example and integration test
- Real-world usage patterns and edge cases
- Uses gametetris-rs library as game engine
- Demonstrates complex state synchronization
