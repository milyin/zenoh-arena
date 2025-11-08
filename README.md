# zenoh-arena

A peer-to-peer network framework for simple game applications built on the [Zenoh](https://docs.rs/zenoh/latest/zenoh/) network library.

**Current Status**: Early development - Phase 1 complete (core infrastructure and local node management). Network layer implementation in progress.

## Overview

`zenoh-arena` provides a Node-centric architecture where each application instance manages its own role (host or client), handles discovery, connection management, and state synchronization for distributed game sessions. There is no central coordinator—each node is autonomous and manages its local view of the network.

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

**Note**: Failover behavior will be implemented in Phase 5. Currently, only local node state management is implemented. Network layer features are planned for future phases.

## API Layers

The framework provides three distinct API surfaces:

### 1. User Interface ↔ Framework

- User interface is agnostic to current node mode (host or client)
- Framework accepts commands via `NodeCommand::GameAction(ACTION)`
- Framework returns node status via `NodeStatus<STATE>` from `step()` method
- Types must support [Zenoh serialization](https://docs.rs/zenoh-ext/latest/zenoh_ext/)
- Data is either delivered to remote host or processed locally (depending on implementation phase)

### 2. Framework ↔ Game Engine

- When in host mode, framework manages game engine instance
- Forwards `ACTION`s to engine via `process_action()` method
- Receives `STATE` updates from engine as return value
- Engine only runs on host nodes

### 3. Framework ↔ Framework (Internal - Planned)

- Node-to-node communication using Zenoh network API
- Discovery, connection handshake, and data exchange
- Liveliness monitoring
- **Status**: Planned for Phase 2-4 implementation

## API Usage

### Creating a Node

Nodes are created using the builder pattern via the `SessionExt` trait:

1. Create a Zenoh session
2. Call `declare_arena_node()` on the session with an engine factory function
3. Configure the node using builder methods:
   - `name()` - Set custom node name (optional, auto-generated if not specified)
   - `force_host()` - Force the node to always be a host
   - `step_timeout_ms()` - Set timeout for step() method
4. Await the builder to create the node

### Sending Commands

Commands are sent to the node via a command sender channel:

- `NodeCommand::GameAction(action)` - Send an action to be processed
- `NodeCommand::Stop` - Stop the node's event loop

### Processing Node Events

The `step()` method processes pending commands and returns:
- `Some(NodeStatus)` - Contains current node state and optional game state
- `None` - Node has stopped (Stop command received or channel closed)

Call `step()` in a loop to drive the node's event processing.

## Example Applications

### z_bonjour - Minimal Example

A minimal example demonstrating the core API:

- Simple counter engine that increments on each action
- Terminal interface to send actions
- Demonstrates node lifecycle and state management
- Located in `z_bonjour/` directory

### z_tetris - Full Game Example (Planned)

The `z_tetris` application will demonstrate full multiplayer functionality with a competitive Tetris game.

#### Features

- Uses [gametetris-rs](https://github.com/milyin/gametetris-rs) library as game engine
- Single terminal application
- Automatic host discovery and connection
- One-on-one competitive gameplay

#### Planned Behavior

**Startup:**

- Searches for existing instances
- Becomes host if no instances found

**Host Mode:**

- Accepts only one client
- Starts pair Tetris game when client connects
- Continues playing if client disconnects
- New clients connect to existing game state

**Client Mode:**

- Connects to available host
- If client loses, disconnects and returns to host-searching
- If client wins (host loses), attempts to find new host or becomes host with current game state

**Game Over:**

- Losing host closes itself
- Winning client searches for new opponent

**Note**: z_tetris implementation is planned for Phase 4+ (after network layer completion).
