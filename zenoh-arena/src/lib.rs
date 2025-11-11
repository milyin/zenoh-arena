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
//! impl MyEngine {
//!     fn new(
//!         input_rx: flume::Receiver<(NodeId, String)>,
//!         output_tx: flume::Sender<String>,
//!         _initial_state: Option<String>,
//!     ) -> Self {
//!         // Spawn a task to process actions
//!         std::thread::spawn(move || {
//!             while let Ok((_node_id, action)) = input_rx.recv() {
//!                 let state = format!("Processed: {}", action);
//!                 let _ = output_tx.send(state);
//!             }
//!         });
//!         
//!         Self
//!     }
//! }
//!
//! impl GameEngine for MyEngine {
//!     type Action = String;
//!     type State = String;
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
//!         .declare_arena_node(MyEngine::new)
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
pub use node::game_engine::{EngineFactory, GameEngine};
pub use node::node::{Node, NodeCommand};
pub use node::session_ext::{NodeBuilder, SessionExt};
pub use node::types::{NodeId, NodeInfo, NodeRole, NodeState};