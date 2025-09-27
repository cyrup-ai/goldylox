//! Interactive REPL for Goldylox CLI
//!
//! This module provides an interactive Read-Eval-Print Loop (REPL) for
//! exploring and using the Goldylox cache system interactively.

use std::io::{self, Write};

use crate::cli::{
    commands::{Commands, execute_command},
    config::CliConfig,
    errors::{CliError, CliResult},
};

/// Start the interactive REPL
pub async fn start_repl(config: CliConfig) -> CliResult<()> {
    println!("🚀 Goldylox Interactive REPL");
    println!("Type 'help' for available commands, 'exit' to quit.");
    println!();

    // Create a mutable config for internal use
    let mut repl_config = config.clone();
    repl_config.quiet = false; // Always show output in REPL mode

    loop {
        print!("lox> ");
        io::stdout().flush().map_err(CliError::IoError)?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(CliError::IoError)?;

        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        // Handle REPL-specific commands first
        match input {
            "exit" | "quit" | "q" => {
                println!("Goodbye!");
                break;
            }
            "help" | "h" => {
                print_help();
                continue;
            }
            "clear" => {
                print!("\x1B[2J\x1B[1;1H"); // Clear screen
                continue;
            }
            _ => {}
        }

        // Parse and execute cache commands
        match parse_repl_command(input) {
            Ok(command) => {
                match execute_command(command, repl_config.clone()).await {
                    Ok(()) => {} // Success - output already handled by execute_command
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Parse error: {}", e);
                println!("Type 'help' to see available commands.");
            }
        }
    }

    Ok(())
}

/// Parse a REPL command line into a Commands enum
fn parse_repl_command(input: &str) -> CliResult<Commands> {
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.is_empty() {
        return Err(CliError::ArgumentError("Empty command".to_string()));
    }

    let command = parts[0].to_lowercase();

    match command.as_str() {
        "get" => {
            if parts.len() != 2 {
                return Err(CliError::ArgumentError("Usage: get <key>".to_string()));
            }
            Ok(Commands::Get {
                key: parts[1].to_string(),
            })
        }
        "put" => {
            if parts.len() < 3 {
                return Err(CliError::ArgumentError(
                    "Usage: put <key> <value>".to_string(),
                ));
            }
            let key = parts[1].to_string();
            let value = parts[2..].join(" ");
            Ok(Commands::Put { key, value })
        }
        "remove" | "rm" => {
            if parts.len() != 2 {
                return Err(CliError::ArgumentError("Usage: remove <key>".to_string()));
            }
            Ok(Commands::Remove {
                key: parts[1].to_string(),
            })
        }
        "clear-cache" => Ok(Commands::Clear),
        "contains" => {
            if parts.len() != 2 {
                return Err(CliError::ArgumentError("Usage: contains <key>".to_string()));
            }
            Ok(Commands::Contains {
                key: parts[1].to_string(),
            })
        }
        "hash" => {
            if parts.len() != 2 {
                return Err(CliError::ArgumentError("Usage: hash <key>".to_string()));
            }
            Ok(Commands::Hash {
                key: parts[1].to_string(),
            })
        }
        "stats" => {
            let detailed = parts.len() > 1 && (parts[1] == "--detailed" || parts[1] == "-d");
            Ok(Commands::Stats { detailed })
        }
        "compression-stats" => Ok(Commands::CompressionStats),
        "put-if-absent" => {
            if parts.len() < 3 {
                return Err(CliError::ArgumentError(
                    "Usage: put-if-absent <key> <value>".to_string(),
                ));
            }
            let key = parts[1].to_string();
            let value = parts[2..].join(" ");
            Ok(Commands::PutIfAbsent { key, value })
        }
        "replace" => {
            if parts.len() < 3 {
                return Err(CliError::ArgumentError(
                    "Usage: replace <key> <value>".to_string(),
                ));
            }
            let key = parts[1].to_string();
            let value = parts[2..].join(" ");
            Ok(Commands::Replace { key, value })
        }
        "compare-and-swap" | "cas" => {
            if parts.len() < 4 {
                return Err(CliError::ArgumentError(
                    "Usage: compare-and-swap <key> <expected> <new_value>".to_string(),
                ));
            }
            let key = parts[1].to_string();
            let expected = parts[2].to_string();
            let new_value = parts[3..].join(" ");
            Ok(Commands::CompareAndSwap {
                key,
                expected,
                new_value,
            })
        }
        "get-or-insert" => {
            if parts.len() < 3 {
                return Err(CliError::ArgumentError(
                    "Usage: get-or-insert <key> <value>".to_string(),
                ));
            }
            let key = parts[1].to_string();
            let value = parts[2..].join(" ");
            Ok(Commands::GetOrInsert { key, value })
        }
        _ => Err(CliError::ArgumentError(format!(
            "Unknown command: {}. Type 'help' for available commands.",
            command
        ))),
    }
}

/// Print help information
fn print_help() {
    println!("Available cache commands:");
    println!("  get <key>                    Get a value by key");
    println!("  put <key> <value>            Store a key-value pair");
    println!("  remove <key>                 Remove a key");
    println!("  clear-cache                  Clear all cache entries");
    println!("  contains <key>               Check if key exists");
    println!("  hash <key>                   Get hash value for key");
    println!("  put-if-absent <key> <value>  Store only if key doesn't exist");
    println!("  replace <key> <value>        Replace existing value");
    println!("  compare-and-swap <key> <expected> <new>  Atomic compare and swap");
    println!("  get-or-insert <key> <value>  Get or insert if absent");
    println!();
    println!("Statistics and management:");
    println!("  stats [--detailed]           Show cache statistics");
    println!("  compression-stats            Show compression statistics");
    println!();
    println!("REPL commands:");
    println!("  help, h                      Show this help");
    println!("  clear                        Clear screen");
    println!("  exit, quit, q                Exit REPL");
    println!();
}
