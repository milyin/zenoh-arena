//! # zenoh-arena
//!
//! A peer-to-peer network framework for simple game applications built on top of Zenoh.
//!
//! ## Overview
//!
//! The `zenoh-arena` library provides a Node-centric architecture where each node manages
//! its own role (host or client), handles discovery, connection management, and state
//! synchronization for distributed game sessions using Zenoh's pub/sub and query/queryable APIs.
//!
//! ## Key Features
//!
//! - Autonomous node behavior - no central coordinator
//! - Automatic host discovery and connection
//! - Host/client role management and transitions
//! - Game state synchronization via pub/sub
//! - Liveliness tracking for connection monitoring
//! - Support for custom game engines via trait
//!
//! ## Example
//!
//! ```rust,no_run
//! use zenoh_arena::{NodeConfig, NodeId};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create configuration
//!     let config = NodeConfig::default();
//!     
//!     // Generate a node ID
//!     let node_id = NodeId::generate();
//!     println!("Node ID: {}", node_id);
//!     
//!     Ok(())
//! }
//! ```

// Module declarations
pub mod config;
pub mod error;
pub mod node;
pub mod types;

// Re-exports for convenience
pub use config::NodeConfig;
pub use error::{ArenaError, Result};
pub use node::{GameEngine, Node, NodeCommand};
pub use types::{NodeId, NodeInfo, NodeRole, NodeStateInfo, NodeStatus};
