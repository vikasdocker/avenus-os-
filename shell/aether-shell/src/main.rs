// Aether Shell - Main entry point
use anyhow::Result;
use std::io::{self, BufRead};
use tracing::{error, info};

use aether_shell::command::CommandRegistry;
use aether_shell::history::ShellHistory;
use aether_shell::output::OutputFormatter;
use aether_shell::session::ShellSession;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let session = ShellSession::new();
    let registry = CommandRegistry::new();
    let history = ShellHistory::new();
    let mut formatter = OutputFormatter::new();

    println!("Aether Shell v{}", env!("CARGO_PKG_VERSION"));
    println!("Type 'help' for command list\n");

    let stdin = io::stdin();
    let reader = stdin.lock();
    let mut lines = reader.lines();

    loop {
        print!("aether> ");
        io::Write::flush(&mut io::stdout())?;

        match lines.next() {
            Some(Ok(line)) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                match handle_command(line, &session, &registry, &mut formatter, &history).await {
                    Ok(should_exit) => {
                        if should_exit {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error: {}", e);
                        eprintln!("Error: {}", e);
                    }
                }
            }
            Some(Err(e)) => {
                error!("Input error: {}", e);
                break;
            }
            None => break,
        }
    }

    println!("Goodbye.");
    info!("Shell session ended");
    Ok(())
}

async fn handle_command(
    input: &str,
    session: &ShellSession,
    registry: &CommandRegistry,
    formatter: &mut OutputFormatter,
    history: &ShellHistory,
) -> Result<bool> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(false);
    }

    let command_name = parts[0];
    let args = &parts[1..];

    // Check for exit command
    if command_name == "exit" || command_name == "quit" {
        return Ok(true);
    }

    // Route to registry
    match registry.execute(command_name, args, session, formatter, history).await {
        Ok(_) => Ok(false),
        Err(e) => Err(e),
    }
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("aether_shell=info".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();
}
