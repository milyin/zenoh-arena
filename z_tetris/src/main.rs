use clap::Parser;
use console::{Key, Term};
use std::path::PathBuf;
use std::time::Duration;
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
    println!("Controls:");
    println!("  ← → - Move left/right");
    println!("  ↓ - Move down");
    println!("  z/x - Rotate left/right");
    println!("  Space - Drop");
    println!("  q - Quit");
    println!();

    // Get command sender for the node
    let node_sender = node.sender();

    // Spawn keyboard input task with separate term
    let keyboard_sender = node_sender.clone();
    let keyboard_task = tokio::task::spawn_blocking(move || {
        let input_term = Term::stdout();
        loop {
            if let Ok(key) = input_term.read_key() {
                let action = match key {
                    Key::ArrowLeft => Some(Action::MoveLeft),
                    Key::ArrowRight => Some(Action::MoveRight),
                    Key::ArrowDown => Some(Action::MoveDown),
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
    let mut last_render = std::time::Instant::now();
    let render_interval = Duration::from_millis(50); // 20 FPS

    // Main step loop - processes commands and renders state
    loop {
        match node.step().await? {
            StepResult::Stop => {
                // Node stopped
                println!("Node stopped");
                break;
            }
            StepResult::GameState(_) | StepResult::Timeout | StepResult::RoleChanged(_) => {
                // Get the current state
                let state = node.node_state();
                let game_state = node.game_state();
                
                match state {
                    NodeState::Host { .. } | NodeState::Client { .. } => {
                        if let Some(game_state) = game_state {
                            // Render the game state
                            if last_render.elapsed() >= render_interval {
                                render_game(&render_term, &game_state)?;
                                last_render = std::time::Instant::now();
                            }
                        }
                    }
                    NodeState::SearchingHost => {
                        // Clear rendering terminal while searching
                        render_term.clear_screen()?;
                        render_term.move_cursor_to(0, 0)?;
                        render_term.write_line("Searching for host...")?;
                        render_term.flush()?;
                    }
                    _ => {
                        // No game state to render yet or in another state
                    }
                }
            }
        }
    }

    // Wait for keyboard task to finish
    keyboard_task.abort();
    let _ = keyboard_task.await;

    println!("Game Over!");
    Ok(())
}

fn render_game(term: &Term, state: &TetrisPairState) -> Result<(), Box<dyn std::error::Error>> {
    let field = GameFieldPair::new(
        state.clone(),
        vec!["PLAYER".to_string()],
        vec!["OPPONENT".to_string()],
    );
    let lines = field.render(&AnsiTermStyle);

    // Clear and render
    term.move_cursor_to(0, 0)?;
    for line in lines {
        term.write_line(&line)?;
    }
    term.flush()?;

    Ok(())
}
