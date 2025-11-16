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
//! use std::sync::{Arc, Mutex};
//! use std::pin::Pin;
//! use std::future::Future;
//!
//! // Define your game engine
//! struct MyEngine {
//!     node_id: Mutex<Option<NodeId>>,
//!     input_tx: flume::Sender<(NodeId, String)>,
//!     output_rx: flume::Receiver<String>,
//! }
//! 
//! impl MyEngine {
//!     fn new() -> Self {
//!         let (input_tx, input_rx) = flume::unbounded();
//!         let (output_tx, output_rx) = flume::unbounded();
//!         
//!         // Spawn a task to process actions
//!         std::thread::spawn(move || {
//!             while let Ok((_node_id, action)) = input_rx.recv() {
//!                 let state = format!("Processed: {}", action);
//!                 let _ = output_tx.send(state);
//!             }
//!         });
//!         
//!         Self {
//!             node_id: Mutex::new(None),
//!             input_tx,
//!             output_rx,
//!         }
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
//!     fn set_node_id(&self, node_id: NodeId) {
//!         *self.node_id.lock().unwrap() = Some(node_id);
//!     }
//!     
//!     fn run(&self, _initial_state: Option<String>) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
//!         Box::pin(async {})
//!     }
//!     
//!     fn stop(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
//!         Box::pin(async {})
//!     }
//!     
//!     fn action_sender(&self) -> &flume::Sender<(NodeId, String)> {
//!         &self.input_tx
//!     }
//!     
//!     fn state_receiver(&self) -> &flume::Receiver<String> {
//!         &self.output_rx
//!     }
//! }
//!
//! #[tokio::main(flavor = "multi_thread", worker_threads = 1)]
//! async fn main() {
//!     // Create a zenoh session
//!     let session = zenoh::open(zenoh::Config::default()).await.unwrap();
//!     
//!     // Create engine and wrap in Arc
//!     let engine = Arc::new(MyEngine::new());
//!     
//!     // Declare an arena node using the extension trait
//!     let node = session
//!         .declare_arena_node(engine)
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
pub use node::game_engine::GameEngine;
pub use node::arena_node::{Node, NodeCommand};
pub use node::session_ext::{NodeBuilder, SessionExt};
pub use node::stats::{NodeStats, StatsTracker};
pub use node::types::{NodeId, NodeInfo, NodeRole, NodeState, StepResult};