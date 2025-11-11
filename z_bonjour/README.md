# z_bonjour

A minimal example application demonstrating the zenoh-arena API.

## Overview

z_bonjour is a simple counter application that verifies the core functionality of the zenoh-arena framework:

- **Engine**: Maintains a signed counter that can be incremented or decremented
- **Actions**: 
  - `Bonjour` - increments the counter
  - `Bonsoir` - decrements the counter
- **State**: Current counter value (signed integer)
- **Node**: Runs in host mode (force_host enabled)

## Building

```bash
cargo build --package z_bonjour
```

## Running

```bash
cargo run --package z_bonjour
```

The application will start in host mode and display:
- Current node state (Host mode)
- Counter value (when it changes)
- Available commands

## Usage

Once running, you can interact with the application:

- Press `b` to send a Bonjour action (increments the counter)
- Press `s` to send a Bonsoir action (decrements the counter)
- Press `q` to quit

### Example Session

```text
=== z_bonjour - Zenoh Arena Demo ===
Node ID: bonjour_node
Commands:
  b - Send Bonjour action (increment counter)
  s - Send Bonsoir action (decrement counter)
  q - Quit

[State] Host mode (open, no clients)
→ Sending Bonjour action...
← Game State: Counter = 1
→ Sending Bonjour action...
← Game State: Counter = 2
→ Sending Bonsoir action...
← Game State: Counter = 1
→ Sending Bonsoir action...
← Game State: Counter = 0
→ Sending Bonsoir action...
← Game State: Counter = -1
→ Quit requested
Node stopped
Goodbye!
```

## Architecture

### BonjourEngine

The game engine implements the `GameEngine` trait:

```rust
pub struct BonjourEngine {
    state: BonjourState,
}
```

- **Action Type**: `BonjourAction` (enum with `Bonjour` and `Bonsoir` variants)
- **State Type**: `BonjourState { counter: i64 }`
- **Behavior**: Increments counter on `Bonjour`, decrements on `Bonsoir`

### Main Application

The application runs two concurrent tasks:

1. **Step Loop**: Continuously calls `node.step()` to process commands and print state
2. **Keyboard Input**: Reads keyboard input in blocking mode and sends commands to the node

This architecture demonstrates:

- Non-blocking node operation using `step()`
- Command-based communication with the node
- State updates from the game engine
- Clean separation between UI and game logic

## Testing

Run the unit tests:

```bash
cargo test --package z_bonjour
```

Tests verify:

- Counter increments and decrements correctly
- Mixed action sequences work properly
- Serialization/deserialization of actions and states
- Engine state management
- Support for negative counter values
