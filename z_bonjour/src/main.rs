mod engine;

use engine::{BonjourAction, BonjourEngine};
use std::io::{self, Read};
use zenoh_arena::{NodeCommand, SessionExt};

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create zenoh session
    let session = zenoh::open(zenoh::Config::default())
        .await
        .map_err(|e| format!("Failed to open zenoh session: {}", e))?;

    // Declare node with force_host enabled for simplicity
    let mut node = session
        .declare_arena_node(BonjourEngine::new)
        .force_host(true)
        .name("bonjour_node".to_string())?
        .step_timeout_ms(1000)
        .await?;

    println!("=== z_bonjour - Zenoh Arena Demo ===");
    println!("Node ID: {}", node.id());
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
