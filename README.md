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

Each node operates in one of two modes:

- **Client Mode**: Searches for available hosts, connects, sends actions, receives game state
- **Host Mode**: Runs the game engine, accepts clients, processes actions, broadcasts state

### Node Behavior

**As Client:**

- Discovers available hosts via Zenoh query
- Connects to first responsive host
- Publishes actions to host
- Subscribes to state updates from host
- Monitors host liveliness
- Reconnects or becomes host if current host disconnects

**As Host:**

- Declares [Queryable](https://docs.rs/zenoh/latest/zenoh/query/struct.Queryable.html) for discovery
- Accepts or rejects client join requests
- Subscribes to actions from all clients
- Processes actions through game engine
- Publishes state updates to all clients
- Manages client lifecycle
- Declares liveliness token

### Host States

A host can be:

- **Open**: Accepts new client connections via Queryable API
- **Closed**: Stops accepting new clients (closes Queryable)
- **Empty**: No clients currently connected

### State Transitions

A host may close and switch back to client mode when:

- Game session ends
- Host becomes empty and wants to search for other hosts
- User explicitly requests shutdown

**Important**: A node cannot be both host and client simultaneously. It must first close the host (stop accepting clients) before searching for other hosts.

### Connection Flow

1. **Discovery**: Client queries for available hosts
2. **Request**: Client sends join request to selected host
3. **Response**: Host accepts or rejects based on availability
4. **Confirmation**: Client confirms connection with second targeted query to host's keyexpr
5. **Connected**: Client begins sending actions and receiving state updates

### Failover Behavior

When a client detects that its host has disconnected (via liveliness monitoring):

1. Switches to host-searching mode
2. Waits for a randomized timeout (prevents thundering herd)
3. Becomes host if no other hosts are found

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
