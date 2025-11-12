use clap::Parser;
use console::Term;
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
        .step_timeout_break_ms(3000)
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

    // Setup Ctrl+C handler
    let ctrlc_sender = node_sender.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\n→ Ctrl+C received, stopping...");
        let _ = ctrlc_sender.send(NodeCommand::Stop);
    });

    // Spawn keyboard input task
    let keyboard_sender = node_sender.clone();
    let keyboard_task = tokio::task::spawn_blocking(move || {
        let term = Term::stdout();

        loop {
            match term.read_key() {
                Ok(console::Key::Char('b') | console::Key::Char('B')) => {
                    println!("→ Sending Bonjour action...");
                    if keyboard_sender
                        .send(NodeCommand::GameAction(BonjourAction::Bonjour))
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(console::Key::Char('s') | console::Key::Char('S')) => {
                    println!("→ Sending Bonsoir action...");
                    if keyboard_sender
                        .send(NodeCommand::GameAction(BonjourAction::Bonsoir))
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(console::Key::Char('q') | console::Key::Char('Q')) => {
                    println!("→ Quit requested");
                    let _ = keyboard_sender.send(NodeCommand::Stop);
                    break;
                }
                Ok(console::Key::Enter) => {
                    // Just ignore Enter key
                }
                Err(_) => {
                    // Error reading from terminal, exit
                    break;
                }
                _ => {}
            }
        }
    });

    // Main step loop - processes commands and prints state
    loop {
        let result = node.step().await?;
        
        // Print status line for each event
        match result {
            StepResult::Stop => {
                println!("[STOPPED] Node: {}", node.id());
                break;
            }
            StepResult::RoleChanged(role) => {
                println!("{}: role changed to {:?}", node.id(), role);
            }
            StepResult::GameState(state) => {
                println!("{}: new game state {}", node.id(), state);
            }
            StepResult::Timeout => {
                println!("{}: {} {}", node.id(), node.state(), node.game_state().unwrap_or_default());
            }
        }
    }

    // Wait for keyboard task to finish
    keyboard_task.abort();
    let _ = keyboard_task.await;

    println!("Goodbye!");
    Ok(())
}
