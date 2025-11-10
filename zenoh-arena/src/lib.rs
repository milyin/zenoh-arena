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
//! - Zenoh session extension trait for easy node creation
//!
//! ## Example
//!
//! ```rust,no_run
//! use zenoh_arena::{SessionExt, GameEngine, NodeId, Result};
//!
//! // Define your game engine
//! struct MyEngine;
//!
//! impl GameEngine for MyEngine {
//!     type Action = String;
//!     type State = String;
//!     
//!     fn process_action(&mut self, action: Self::Action, _client_id: &NodeId) -> Result<Self::State> {
//!         Ok(format!("Processed: {}", action))
//!     }
//! 
//!     fn max_clients(&self) -> Option<usize> {
//!         None // Unlimited clients
//!     }
//! }
//!
//! #[tokio::main(flavor = "multi_thread", worker_threads = 1)]
//! async fn main() {
//!     // Create a zenoh session
//!     let session = zenoh::open(zenoh::Config::default()).await.unwrap();
//!     
//!     // Declare an arena node using the extension trait
//!     let node = session
//!         .declare_arena_node(|| MyEngine)
//!         .name("my_node".to_string())
//!         .unwrap()
//!         .force_host(true)
//!         .await
//!         .unwrap();
//!     
//!     println!("Node ID: {}", node.id());
//! }
//! ```
pub(crate) mod network;
pub(crate) mod node;
pub(crate) mod error;

// Re-exports external API
pub use error::{ArenaError, Result};
pub use node::node::{GameEngine, Node, NodeCommand};
pub use node::session_ext::{NodeBuilder, SessionExt};
pub use node::types::{NodeId, NodeInfo, NodeRole, NodeState};