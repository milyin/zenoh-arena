# zenoh-arena

A peer-to-peer network framework for simple game applications built on the [Zenoh](https://docs.rs/zenoh/latest/zenoh/) network library.

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
- Transitions to **Host** state if no hosts are found and `auto_host` is enabled

#### Client State

When in this state, the node:

- Maintains connection to a specific host
- Publishes actions to the host via dedicated keyexpr
- Subscribes to state updates from the host
- Monitors host liveliness using [liveliness tokens](https://docs.rs/zenoh/latest/zenoh/liveliness/index.html)
- Processes and displays state updates from the host
- Transitions to **Searching** state if host disconnects or connection is lost

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
- Transitions to **Searching** state when:
  - Game session ends
  - Host becomes empty and chooses to search for other hosts
  - User explicitly requests to stop hosting

### State Transition Rules

**Important**: A node can only be in one state at a time. State transitions follow these rules:

- **Searching → Client**: When a host accepts the connection request
- **Searching → Host**: When no hosts found and node becomes host (if `auto_host` enabled)
- **Client → Searching**: When host disconnects, connection fails, or user disconnects
- **Host → Searching**: When host stops (game ends, becomes empty, or user request)
- **Direct transitions between Client and Host are not allowed** - must go through Searching state

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
4. Becomes **Host** if no other hosts are found (if `auto_host` enabled)
5. Otherwise, connects to an available host and becomes **Client**

## API Layers

The framework provides three distinct API surfaces:

### 1. User Interface ↔ Framework

- User interface is agnostic to current node mode (host or client)
- Framework accepts data of type `ACTION`
- Framework emits data of type `STATE`
- Types must support [Zenoh serialization](https://docs.rs/zenoh-ext/latest/zenoh_ext/)
- Data is either delivered to remote host or processed locally

### 2. Framework ↔ Game Engine

- When in host mode, framework instantiates game engine
- Forwards `ACTION`s from clients to engine
- Receives `STATE` updates from engine
- Distributes states to connected clients

### 3. Framework ↔ Framework (Internal)

- Node-to-node communication using Zenoh network API
- Discovery, connection handshake, and data exchange
- Liveliness monitoring

## Example: z_tetris Application

The `z_tetris` application demonstrates `zenoh-arena` usage with a multiplayer Tetris game.

### Features

- Uses [gametetris-rs](https://github.com/milyin/gametetris-rs) library as game engine
- Single terminal application
- Automatic host discovery and connection
- One-on-one competitive gameplay

### Behavior

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
