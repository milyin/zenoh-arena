use clap::Parser;
use console::{Key, Term};
use std::path::PathBuf;
use std::vec;
use z_tetris::engine::{TetrisAction, TetrisEngine};
use z_tetris::{Action, AnsiTermStyle, GameFieldPair, TermRender, TetrisPairState};
use zenoh::key_expr::KeyExpr;
use zenoh_arena::{NodeCommand, NodeState, SessionExt, StepResult};

/// z_tetris - Zenoh Arena Tetris Game
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Node name
    #[arg(short, long)]
    name: Option<String>,

    /// Key expression prefix
    #[arg(short, long)]
    prefix: Option<KeyExpr<'static>>,

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
        .declare_arena_node(TetrisEngine::new)
        .force_host(args.force_host)
        .step_timeout_break_ms(1000);

    // Apply name if provided
    if let Some(name) = args.name.clone() {
        node_builder = node_builder.name(name)?;
    }

    // Apply prefix if provided
    if let Some(prefix) = args.prefix.clone() {
        node_builder = node_builder.prefix(prefix);
    }

    let mut node = node_builder.await?;

    println!("=== z_tetris - Zenoh Arena Tetris ===");
    println!("Node ID: {}", node.id());
    println!("Force host: {}", args.force_host);
    if let Some(ref prefix) = args.prefix {
        println!("Prefix: {}", prefix);
    }
    println!();

    // Get command sender for the node
    let node_sender = node.sender();

    // Spawn keyboard input task with separate term
    let keyboard_sender = node_sender.clone();
    let _keyboard_task = tokio::task::spawn_blocking(move || {
        let input_term = Term::stdout();
        loop {
            if let Ok(key) = input_term.read_key() {
                let action = match key {
                    Key::ArrowLeft => Some(Action::MoveLeft),
                    Key::ArrowRight => Some(Action::MoveRight),
                    Key::ArrowDown => Some(Action::MoveDown),
                    Key::ArrowUp => Some(Action::RotateRight),
                    Key::Char('z') | Key::Char('Z') => Some(Action::RotateLeft),
                    Key::Char('x') | Key::Char('X') => Some(Action::RotateRight),
                    Key::Char(' ') => Some(Action::Drop),
                    Key::Char('q') | Key::Char('Q') => {
                        println!("→ Quit requested");
                        let _ = keyboard_sender.send(NodeCommand::Stop);
                        break;
                    }
                    _ => None,
                };

                if let Some(act) = action
                    && keyboard_sender
                        .send(NodeCommand::GameAction(TetrisAction { action: act }))
                        .is_err()
                {
                    break;
                }
            }
        }
    });

    // Create rendering terminal (separate from input)
    let render_term = Term::stdout();

    // Main step loop - processes commands and renders state
    loop {
        match node.step().await? {
            StepResult::Stop => {
                // Node stopped
                println!("Node stopped");
                break;
            }
            StepResult::GameState(mut game_state) => {
                // Game state changed - render it
                let state = node.node_state();

                // If we're a client, swap player and opponent views
                if matches!(state, NodeState::Client { .. }) {
                    game_state.swap();
                }

                let message = format_node_state_message(&state);
                render_game(&render_term, &game_state, message)?;

                // Check if player's game is over
                if game_state.player.game_over {
                    break;
                }
            }
            StepResult::RoleChanged(_) => {
                // Role changed - later show status, now unused
            }
            StepResult::Timeout => {
                // Timeout - no game state change, don't render
            }
        }
    }

    println!("Game Over!");

    // Force exit to avoid waiting on cleanup of blocked keyboard task
    std::process::exit(0);
}

fn format_node_state_message(state: &NodeState) -> Vec<String> {
    // Start with controls help
    let mut output = vec![
        "".to_string(),
        "← → ↓ - Move".to_string(),
        "↑ z/x - Rotate".to_string(),
        "Space - Drop".to_string(),
        "q - Quit".to_string(),
        "".to_string(),
    ];
    
    // Add node state info
    match state {
        NodeState::SearchingHost => {
            output.push("State: Searching Host".to_string());
        },
        NodeState::Client { host_id } => {
            output.push("State: Client".to_string());
            output.push(format!("Host ID: {}", host_id));
        },
        NodeState::Host { is_accepting, connected_clients } => {
            output.push("State: Host".to_string());
            output.push(format!("Accepting: {}", if *is_accepting { "Yes" } else { "No" }));
            output.push(format!("Clients: {}", connected_clients.len()));
            for client_id in connected_clients {
                output.push(format!("  - {}", client_id));
            }
        },
        NodeState::Stop => {
            output.push("State: Stopped".to_string());
        },
    }
    
    output
}

fn render_game(term: &Term, state: &TetrisPairState, message: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let field = GameFieldPair::new(state.clone(), message);
    let lines = field.render(&AnsiTermStyle);

    // Clear and render
    term.move_cursor_to(0, 0)?;
    for line in lines {
        term.write_line(&line)?;
    }
    term.flush()?;

    Ok(())
}
