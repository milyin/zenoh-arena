mod engine;

use clap::Parser;
use engine::{BonjourAction, BonjourEngine};
use std::io::{self, Read};
use std::path::PathBuf;
use zenoh_arena::{NodeCommand, SessionExt};

/// z_bonjour - Zenoh Arena Demo
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Node name
    #[arg(short, long, default_value = "bonjour_node")]
    name: String,

    /// Key expression prefix
    #[arg(short, long)]
    prefix: Option<String>,

    /// Force host mode
    #[arg(short, long, default_value = "true")]
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
        .force_host(args.force_host)
        .name(args.name.clone())?
        .step_timeout_ms(1000);

    // Apply prefix if provided
    let prefix_str = args.prefix.clone();
    if let Some(prefix) = args.prefix {
        let prefix_keyexpr: zenoh::key_expr::KeyExpr<'static> = prefix.try_into()
            .map_err(|e| format!("Invalid prefix key expression: {}", e))?;
        node_builder = node_builder.prefix(prefix_keyexpr);
    }

    let mut node = node_builder.await?;

    println!("=== z_bonjour - Zenoh Arena Demo ===");
    println!("Node name: {}", args.name);
    println!("Node ID: {}", node.id());
    println!("Force host: {}", args.force_host);
    if let Some(prefix) = prefix_str {
        println!("Prefix: {}", prefix);
    }
    println!("Commands:");
    println!("  b - Send Bonjour action");
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
                        .send(NodeCommand::GameAction(BonjourAction))
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
            Some(status) => {
                println!("{}", status);
            }
            None => {
                // Node stopped
                println!("Node stopped");
                break;
            }
        }
    }

    // Wait for keyboard task to finish
    keyboard_task.abort();
    let _ = keyboard_task.await;

    println!("Goodbye!");
    Ok(())
}
