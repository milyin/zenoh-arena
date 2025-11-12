# zenoh-arena

A peer-to-peer network framework for simple game applications built on the [Zenoh](https://docs.rs/zenoh/latest/zenoh/) network library.

**Current Status**: Functional - Core infrastructure and network layer implemented. Two example applications (z_bonjour and z_tetris) are working.

## Overview

`zenoh-arena` provides a Node-centric architecture where each application instance manages its own role (host or client), handles discovery, connection management, and state synchronization for distributed game sessions. There is no central coordinator—each node is autonomous and manages its local view of the network.

## Quick Start

### Requirements

- Rust 1.75+ (2021 edition)
- Zenoh 1.6.2
- Terminal with ANSI color support (for z_tetris)

### Running the Examples

```bash
# Clone the repository
git clone https://github.com/milyin/zenoh-arena
cd zenoh-arena

# Run the simple example (z_bonjour)
cargo run --package z_bonjour

# Or try the multiplayer Tetris game
# Terminal 1:
cargo run --package z_tetris

# Terminal 2:
cargo run --package z_tetris
```

## Key Features

- **Autonomous Nodes**: Each application instance has a unique name (auto-generated if not specified)
- **Dynamic Role Management**: Nodes can switch between host and client modes based on network conditions
- **Automatic Discovery**: Clients discover hosts using [Zenoh Queriers](https://docs.rs/zenoh/latest/zenoh/query/struct.Querier.html)
- **Liveliness Tracking**: Hosts declare [liveliness tokens](https://docs.rs/zenoh/latest/zenoh/liveliness/index.html) for connection monitoring
- **Game Engine Integration**: Clean separation between network layer and game logic

## Architecture

### Node States

Each node operates in one of three states:

1. **Searching for Host** - Looking for available hosts to connect to
2. **Client** - Connected to a host, sending actions and receiving state
3. **Host** - Running game engine, accepting clients, broadcasting state

### State Behaviors

#### Searching for Host State

When in this state, the node:

- Queries the network for available hosts using [Zenoh Queriers](https://docs.rs/zenoh/latest/zenoh/query/struct.Querier.html)
- Waits for host responses (with configurable timeout and randomized jitter)
- Evaluates available hosts based on acceptance status and capacity
- Transitions to **Client** state if a suitable host is found
- Transitions to **Host** state if no hosts are found (will wait or fail depending on configuration)

**Note**: This state is skipped entirely if `force_host` is enabled in configuration.

#### Client State

When in this state, the node:

- Maintains connection to a specific host
- Publishes actions to the host via dedicated keyexpr
- Subscribes to state updates from the host
- Monitors host liveliness using [liveliness tokens](https://docs.rs/zenoh/latest/zenoh/liveliness/index.html)
- Processes and displays state updates from the host
- Transitions to **Searching** state if host disconnects or connection is lost

**Note**: This state cannot be entered if `force_host` is enabled in configuration.

#### Host State

When in this state, the node:

- Runs the game engine instance
- Declares [Queryable](https://docs.rs/zenoh/latest/zenoh/query/struct.Queryable.html) for host discovery (when accepting clients)
- Accepts or rejects client join requests based on capacity
- Subscribes to actions from all connected clients via wildcard pattern
- Processes actions through the game engine
- Publishes state updates to all connected clients
- Declares liveliness token for connection monitoring
- Manages client lifecycle (connections/disconnections)
- Can be **Open** (accepting new clients) or **Closed** (not accepting new clients)
- Can be **Empty** (no connected clients) or have connected clients
- Normally transitions to **Searching** state when:
  - Game session ends
  - Host becomes empty and chooses to search for other hosts
  - User explicitly requests to stop hosting
- If `force_host` is enabled, remains in Host state permanently (cannot transition out)

### State Transition Rules

**Important**: A node can only be in one state at a time. State transitions follow these rules:

**Normal Mode** (when `force_host` is `false`):

- **Searching → Client**: When a host accepts the connection request
- **Searching → Host**: When no hosts found and node decides to become host
- **Client → Searching**: When host disconnects, connection fails, or user disconnects
- **Host → Searching**: When host stops (game ends, becomes empty, or user request)
- **Direct transitions between Client and Host are not allowed** - must go through Searching state

**Force Host Mode** (when `force_host` is `true`):

- Node starts directly in **Host** state
- **No transitions allowed** - node remains in Host state permanently
- Searching and Client states are blocked and cannot be entered

### Connection Flow

1. **Discovery** (Searching state): Node queries for available hosts
2. **Request** (Searching state): Node sends join request to selected host
3. **Response**: Host (in Host state) accepts or rejects based on availability
4. **Confirmation** (Client state): Client confirms connection with targeted query to host's keyexpr
5. **Connected** (Client state): Client begins sending actions and receiving state updates

### Failover Behavior

When a client detects that its host has disconnected (via liveliness monitoring):

1. Transitions to **Searching** state
2. Waits for a randomized timeout (prevents thundering herd)
3. Queries for available hosts
4. Becomes **Host** if no other hosts are found
5. Otherwise, connects to an available host and becomes **Client**

## API Layers

The framework provides three distinct API surfaces:

### 1. User Interface ↔ Framework

- User interface is agnostic to current node mode (host or client)
- Framework accepts commands via `NodeCommand::GameAction(ACTION)`
- Framework returns node status via `NodeStatus<STATE>` from `step()` method
- Types must support [Zenoh serialization](https://docs.rs/zenoh-ext/latest/zenoh_ext/)
- Actions are delivered to remote host (when in client mode) or processed locally (when in host mode)

### 2. Framework ↔ Game Engine

- When in host mode, framework manages game engine instance
- Forwards `ACTION`s to engine via `process_action()` method
- Receives `STATE` updates from engine as return value
- Engine only runs on host nodes

### 3. Framework ↔ Framework (Internal)

- Node-to-node communication using Zenoh network API
- Discovery, connection handshake, and data exchange
- Liveliness monitoring

## API Usage

### Using in Your Project

Add to your `Cargo.toml`:

```toml
[dependencies]
zenoh-arena = { path = "../zenoh-arena" }  # or use git dependency
zenoh = "1.6.2"
zenoh-ext = "1.0.3"
serde = { version = "1.0", features = ["derive"] }
```

### Creating a Node

Nodes are created using the builder pattern via the `SessionExt` trait:

1. Create a Zenoh session
2. Call `declare_arena_node()` on the session with an engine factory function
3. Configure the node using builder methods:
   - `name()` - Set custom node name (optional, auto-generated if not specified)
   - `force_host()` - Force the node to always be a host
   - `prefix()` - Set key expression prefix for namespacing
   - `step_timeout_break_ms()` - Set timeout for step() method
   - `search_timeout_ms()` - Set host search timeout
   - `search_jitter_ms()` - Set randomized delay for host search
4. Await the builder to create the node

**Example:**

```rust
use zenoh_arena::{SessionExt, GameEngine};

// Create zenoh session
let session = zenoh::open(zenoh::Config::default()).await?;

// Create node with custom settings
let mut node = session
    .declare_arena_node(MyEngine::new)
    .name("my_node".to_string())?
    .force_host(false)
    .step_timeout_break_ms(1000)
    .await?;
```

### Sending Commands

Commands are sent to the node via a command sender channel:

- `NodeCommand::GameAction(action)` - Send an action to be processed
- `NodeCommand::Stop` - Stop the node's event loop

**Example:**

```rust
let sender = node.sender();

// Send game action
sender.send(NodeCommand::GameAction(MyAction::Jump))?;

// Stop the node
sender.send(NodeCommand::Stop)?;
```

### Processing Node Events

The `step()` method processes pending commands and returns a `StepResult`:

- `StepResult::Stop` - Node has stopped
- `StepResult::GameState(state)` - Game state updated
- `StepResult::RoleChanged(role)` - Node role changed (e.g., client → host)
- `StepResult::Timeout` - No events within timeout period

Call `step()` in a loop to drive the node's event processing.

**Example:**

```rust
loop {
    match node.step().await? {
        StepResult::Stop => break,
        StepResult::GameState(state) => {
            // Handle new game state
            println!("State: {:?}", state);
        }
        StepResult::RoleChanged(role) => {
            println!("Role changed to: {:?}", role);
        }
        StepResult::Timeout => {
            // No events, continue
        }
    }
}
```

### Implementing a Game Engine

The `GameEngine` trait defines how your game logic integrates with the framework:

```rust
use zenoh_arena::{GameEngine, NodeId};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct MyAction {
    // Your action fields
}

#[derive(Serialize, Deserialize, Clone)]
struct MyState {
    // Your state fields
}

struct MyEngine {
    // Engine state
}

impl MyEngine {
    fn new(
        host_id: NodeId,
        input_rx: flume::Receiver<(NodeId, MyAction)>,
        output_tx: flume::Sender<MyState>,
        initial_state: Option<MyState>,
    ) -> Self {
        // Spawn task to process actions
        std::thread::spawn(move || {
            while let Ok((client_id, action)) = input_rx.recv() {
                // Process action and generate new state
                let new_state = /* ... */;
                let _ = output_tx.send(new_state);
            }
        });
        
        Self { /* ... */ }
    }
}

impl GameEngine for MyEngine {
    type Action = MyAction;
    type State = MyState;
    
    fn max_clients(&self) -> Option<usize> {
        Some(1)  // Limit to 1 client, or None for unlimited
    }
}
```

## Example Applications

### z_bonjour - Minimal Example

A minimal example demonstrating the core API:

- Simple counter engine that increments/decrements on actions
- Terminal interface with keyboard input
- Demonstrates node lifecycle and state management
- Runs in forced host mode (single-node operation)
- Located in `z_bonjour/` directory

**See [z_bonjour/README.md](z_bonjour/README.md) for detailed documentation.**

### z_tetris - Full Game Example

A fully functional multiplayer competitive Tetris game demonstrating all framework capabilities.

#### Features

- Two-player competitive Tetris gameplay
- Automatic host discovery and connection
- Terminal-based UI with dual-field display
- Player names displayed above each field
- Automatic role switching and game state management

#### How to Run

```bash
# Build the game
cargo build --package z_tetris

# Run first instance (becomes host)
cargo run --package z_tetris

# Run second instance in another terminal (connects as client)
cargo run --package z_tetris
```

#### Controls

- `←` `→` - Move piece left/right
- `↓` - Move piece down
- `↑` `z` `x` - Rotate piece (left/right)
- `Space` - Drop piece
- `q` - Quit

#### Behavior

**Startup:**

- Searches for existing hosts
- Becomes host if no hosts found
- Connects as client if host is available

**Host Mode:**

- Accepts one client connection
- Runs game engine for both players
- Broadcasts game state to client
- Processes actions from both players
- Displays opponent's field on right side

**Client Mode:**

- Connects to available host
- Sends player actions to host
- Receives and displays game state
- Views are swapped (player on left, opponent on right)

**Game Over:**

- Game ends when either player's field fills up
- Losing node exits automatically
- Winning node can continue playing or start new game

## Project Structure

This repository is a Cargo workspace containing:

- **`zenoh-arena/`** - Core library providing the framework
- **`z_bonjour/`** - Minimal example (counter application)
- **`z_tetris/`** - Full multiplayer Tetris game example

## Building

Build all packages:

```bash
cargo build
```

Build specific package:

```bash
cargo build --package zenoh-arena
cargo build --package z_bonjour
cargo build --package z_tetris
```

Build with optimizations:

```bash
cargo build --release
```

## Testing

Run tests for all packages:

```bash
cargo test
```

Run tests for specific package:

```bash
cargo test --package zenoh-arena
```

## Documentation

Generate and view documentation:

```bash
cargo doc --open
```
