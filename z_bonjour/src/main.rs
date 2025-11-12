use clap::Parser;
use std::io::{self, Read};
use std::path::PathBuf;
use z_bonjour::engine::{BonjourAction, BonjourEngine};
use zenoh::key_expr::KeyExpr;
use zenoh_arena::{NodeCommand, SessionExt, StepResult};

/// z_bonjour - Zenoh Arena Demo
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Node name
    #[arg(short, long)]
    name: Option<String>,

    /// Key expression prefix
    #[arg(short, long, default_value = "zenoh/bonjour")]
    prefix: KeyExpr<'static>,

    /// Force host mode
    #[arg(short, long)]
    force_host: bool,

    /// Path to Zenoh config file
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create zenoh config
    let zenoh_config = if let Some(config_path) = args.config {
        zenoh::Config::from_file(config_path)
            .map_err(|e| format!("Failed to load config file: {}", e))?
    } else {
        zenoh::Config::default()
    };

    // Create zenoh session
    let session = zenoh::open(zenoh_config)
        .await
        .map_err(|e| format!("Failed to open zenoh session: {}", e))?;

    // Declare node with configured parameters
    let mut node_builder = session
        .declare_arena_node(BonjourEngine::new)
        .force_host(args.force_host);

    // Apply name if provided
    if let Some(name) = args.name.clone() {
        node_builder = node_builder.name(name)?;
    }

    // Apply prefix
    node_builder = node_builder.prefix(args.prefix.clone());

    let mut node = node_builder.await?;

    println!("=== z_bonjour - Zenoh Arena Demo ===");
    println!("Node ID: {}", node.id());
    println!("Force host: {}", args.force_host);
    println!("Prefix: {}", args.prefix);
    println!("Commands:");
    println!("  b - Send Bonjour action (increment counter)");
    println!("  s - Send Bonsoir action (decrement counter)");
    println!("  q - Quit");
    println!();

    // Get command sender for the node
    let node_sender = node.sender();

    // Spawn keyboard input task
    let keyboard_sender = node_sender.clone();
    let keyboard_task = tokio::task::spawn_blocking(move || {
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let mut buf = [0u8; 1];

        loop {
            if reader.read_exact(&mut buf).is_err() {
                break;
            }

            match buf[0] {
                b'b' | b'B' => {
                    println!("→ Sending Bonjour action...");
                    if keyboard_sender
                        .send(NodeCommand::GameAction(BonjourAction::Bonjour))
                        .is_err()
                    {
                        break;
                    }
                }
                b's' | b'S' => {
                    println!("→ Sending Bonsoir action...");
                    if keyboard_sender
                        .send(NodeCommand::GameAction(BonjourAction::Bonsoir))
                        .is_err()
                    {
                        break;
                    }
                }
                b'q' | b'Q' => {
                    println!("→ Quit requested");
                    let _ = keyboard_sender.send(NodeCommand::Stop);
                    break;
                }
                _ => {}
            }
        }
    });

    // Main step loop - processes commands and prints state
    loop {
        match node.step().await? {
            StepResult::Stop => {
                // Node stopped
                println!("Node stopped");
                break;
            }
            StepResult::RoleChanged(role) => {
                println!("→ Node role changed to: {:?}", role);
            }
            StepResult::GameState(state) => {
                println!("Game state: {}", state);
            }
            StepResult::Timeout => {
                println!("Timeout passed without state update");
            }
        }
        println!("Node state: {}", node.state());
    }

    // Wait for keyboard task to finish
    keyboard_task.abort();
    let _ = keyboard_task.await;

    println!("Goodbye!");
    Ok(())
}
