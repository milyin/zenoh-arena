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
//! struct MyEngine {
//!     input_tx: flume::Sender<(NodeId, String)>,
//!     input_rx: flume::Receiver<(NodeId, String)>,
//!     output_tx: flume::Sender<String>,
//!     output_rx: flume::Receiver<String>,
//! }
//! 
//! impl MyEngine {
//!     fn new() -> Self {
//!         let (input_tx, input_rx) = flume::unbounded();
//!         let (output_tx, output_rx) = flume::unbounded();
//!         
//!         // Spawn a task to process actions
//!         let input_rx_clone = input_rx.clone();
//!         let output_tx_clone = output_tx.clone();
//!         std::thread::spawn(move || {
//!             while let Ok((_node_id, action)) = input_rx_clone.recv() {
//!                 let state = format!("Processed: {}", action);
//!                 let _ = output_tx_clone.send(state);
//!             }
//!         });
//!         
//!         Self { input_tx, input_rx, output_tx, output_rx }
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
//!     
//!     fn input_sender(&self) -> flume::Sender<(NodeId, Self::Action)> {
//!         self.input_tx.clone()
//!     }
//!     
//!     fn output_receiver(&self) -> flume::Receiver<Self::State> {
//!         self.output_rx.clone()
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
//!         .declare_arena_node(|| MyEngine::new())
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