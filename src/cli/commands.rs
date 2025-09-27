//! CLI command implementations
//!
//! This module contains the implementations for all CLI commands that map
//! to the Goldylox public API methods.

use std::path::PathBuf;

use crate::cli::{
    config::CliConfig,
    daemon_detection::DaemonDetection,
    errors::{CliError, CliResult},
    output::{
        format_batch_summary, format_boolean, format_hash, format_optional, format_stats,
        format_value, print_output,
    },
};

// We'll define the command enums here and re-export them to the binary

#[derive(Debug)]
pub enum Commands {
    Get {
        key: String,
    },
    Put {
        key: String,
        value: String,
    },
    Remove {
        key: String,
    },
    Clear,
    Contains {
        key: String,
    },
    Hash {
        key: String,
    },
    PutIfAbsent {
        key: String,
        value: String,
    },
    Replace {
        key: String,
        value: String,
    },
    CompareAndSwap {
        key: String,
        expected: String,
        new_value: String,
    },
    GetOrInsert {
        key: String,
        value: String,
    },
    BatchGet {
        keys: Vec<String>,
    },
    BatchPut {
        entries: Vec<String>,
    },
    BatchRemove {
        keys: Vec<String>,
    },
    Stats {
        detailed: bool,
    },
    CompressionStats,
    Strategy {
        action: StrategyAction,
    },
    Tasks {
        action: TaskAction,
    },
    Maintenance {
        action: MaintenanceAction,
    },
    System {
        action: SystemAction,
    },
    Config {
        action: ConfigAction,
    },
    Repl,
    Completions {
        shell: String,
    },
}

#[derive(Debug)]
pub enum StrategyAction {
    Metrics,
    Thresholds,
    Force { strategy: String },
}

#[derive(Debug)]
pub enum TaskAction {
    List,
    Cancel { task_id: u64 },
    Stats,
}

#[derive(Debug)]
pub enum MaintenanceAction {
    Breakdown,
    Config,
    Start,
}

#[derive(Debug)]
pub enum SystemAction {
    Start,
    Stop,
    Shutdown,
}

#[derive(Debug)]
pub enum ConfigAction {
    Show,
    Update { parameter: String, value: String },
    Generate { output: PathBuf },
}

/// Execute a CLI command
pub async fn execute_command(command: Commands, config: CliConfig) -> CliResult<()> {
    // Load configuration
    let (cli_config, _cache_config) = config.load_and_merge()?;

    // Handle commands that don't need daemon connection
    match &command {
        Commands::Completions { .. } => {
            // Completions don't need daemon connection
        }
        _ => {
            // All other commands need daemon connection
        }
    }

    // Handle commands that don't need daemon connection first
    if let Commands::Completions { shell } = command {
        // Generate completions - simplified implementation
        println!("# Goldylox CLI completions for {}", shell);
        println!("# Add this to your shell's configuration file");
        println!();

        // Provide basic completion setup instructions based on shell
        match shell.to_lowercase().as_str() {
            "bash" => {
                println!("# Add this to ~/.bashrc:");
                println!("# eval \"$(lox completions bash)\"");
                println!();
                println!(
                    "complete -W 'get put remove clear contains hash put-if-absent replace compare-and-swap get-or-insert batch-get batch-put batch-remove stats compression-stats strategy tasks maintenance system config repl completions' lox"
                );
            }
            "zsh" => {
                println!("# Add this to ~/.zshrc:");
                println!("# eval \"$(lox completions zsh)\"");
                println!();
                println!("#compdef lox");
                println!("_lox() {{");
                println!("  local -a commands");
                println!("  commands=(");
                println!("    'get:Get a value from the cache'");
                println!("    'put:Store a key-value pair in the cache'");
                println!("    'remove:Remove a key from the cache'");
                println!("    'clear:Clear all entries from the cache'");
                println!("    'contains:Check if a key exists'");
                println!("    'hash:Get hash value for a key'");
                println!("    'stats:Get cache statistics'");
                println!("    'repl:Start interactive REPL mode'");
                println!("  )");
                println!("  _describe 'commands' commands");
                println!("}}");
                println!("compdef _lox lox");
            }
            "fish" => {
                println!("# Add this to ~/.config/fish/completions/lox.fish:");
                println!(
                    "complete -c lox -n '__fish_use_subcommand' -a get -d 'Get a value from the cache'"
                );
                println!(
                    "complete -c lox -n '__fish_use_subcommand' -a put -d 'Store a key-value pair in the cache'"
                );
                println!(
                    "complete -c lox -n '__fish_use_subcommand' -a remove -d 'Remove a key from the cache'"
                );
                println!(
                    "complete -c lox -n '__fish_use_subcommand' -a clear -d 'Clear all entries from the cache'"
                );
                println!(
                    "complete -c lox -n '__fish_use_subcommand' -a contains -d 'Check if a key exists'"
                );
                println!(
                    "complete -c lox -n '__fish_use_subcommand' -a hash -d 'Get hash value for a key'"
                );
                println!(
                    "complete -c lox -n '__fish_use_subcommand' -a stats -d 'Get cache statistics'"
                );
                println!(
                    "complete -c lox -n '__fish_use_subcommand' -a repl -d 'Start interactive REPL mode'"
                );
            }
            "powershell" | "pwsh" => {
                println!("# Add this to your PowerShell profile:");
                println!("Register-ArgumentCompleter -Native -CommandName lox -ScriptBlock {{{{");
                println!("    param($commandName, $wordToComplete, $cursorPosition)");
                println!(
                    "    $commands = @('get', 'put', 'remove', 'clear', 'contains', 'hash', 'stats', 'repl')"
                );
                println!(
                    "    $commands | Where-Object {{{{ $_ -like \"$wordToComplete*\" }}}} | ForEach-Object {{{{"
                );
                println!(
                    "        [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)"
                );
                println!("    }}}}");
                println!("}}}}");
            }
            "elvish" => {
                println!("# Add this to ~/.elvish/rc.elv:");
                println!("edit:completion:arg-completer[lox] = [@words]{{");
                println!("  commands = [get put remove clear contains hash stats repl]");
                println!("  if (== (count $words) 2) {{");
                println!("    put $@commands");
                println!("  }}");
                println!("}}");
            }
            _ => {
                return Err(CliError::ArgumentError(format!(
                    "Unsupported shell: {}. Supported: bash, zsh, fish, powershell, elvish",
                    shell
                )));
            }
        }

        if !cli_config.quiet {
            eprintln!("Shell completions generated for: {}", shell);
        }
        return Ok(());
    }

    // Set up daemon connection for all other commands
    let daemon_endpoint = cli_config
        .daemon_endpoint
        .unwrap_or_else(|| "http://127.0.0.1:51085".to_string());
    let daemon_timeout = cli_config.daemon_timeout_ms.unwrap_or(5000);
    let auto_start = cli_config.auto_start_daemon.unwrap_or(true);

    // Initialize daemon detection
    let daemon_detection = DaemonDetection::new(daemon_endpoint, daemon_timeout, auto_start);

    // Ensure daemon is running and get client
    let client = daemon_detection.ensure_daemon_running().await?;

    match command {
        Commands::Get { key } => {
            let result = client.get(&key).await?;
            let output = format_optional(&result, &cli_config.output_format);
            print_output(&output, &cli_config.output_format, cli_config.quiet);
        }

        Commands::Put { key, value } => {
            client.put(&key, &value).await?;
            if !cli_config.quiet {
                print_output("OK", &cli_config.output_format, false);
            }
        }

        Commands::Remove { key } => {
            let removed = client.remove(&key).await?;
            let output = format_boolean(removed, &cli_config.output_format);
            print_output(&output, &cli_config.output_format, cli_config.quiet);
        }

        Commands::Clear => {
            client.clear().await?;
            if !cli_config.quiet {
                print_output("Cache cleared", &cli_config.output_format, false);
            }
        }

        Commands::Contains { key } => {
            let contains = client.contains_key(&key).await?;
            let output = format_boolean(contains, &cli_config.output_format);
            print_output(&output, &cli_config.output_format, cli_config.quiet);
        }

        Commands::Hash { key } => {
            let hash = client.hash_key(&key).await?;
            let output = format_hash(hash, &cli_config.output_format);
            print_output(&output, &cli_config.output_format, cli_config.quiet);
        }

        Commands::PutIfAbsent { key, value } => {
            let previous = client.put_if_absent(&key, &value).await?;
            let output = format_optional(&previous, &cli_config.output_format);
            print_output(&output, &cli_config.output_format, cli_config.quiet);
        }

        Commands::Replace { key, value } => {
            let previous = client.replace(&key, &value).await?;
            let output = format_optional(&previous, &cli_config.output_format);
            print_output(&output, &cli_config.output_format, cli_config.quiet);
        }

        Commands::CompareAndSwap {
            key,
            expected,
            new_value,
        } => {
            let swapped = client.compare_and_swap(&key, &expected, &new_value).await?;
            let output = format_boolean(swapped, &cli_config.output_format);
            print_output(&output, &cli_config.output_format, cli_config.quiet);
        }

        Commands::GetOrInsert { key, value } => {
            let result = client.get_or_insert(&key, &value).await?;
            let output = format_value(&result, &cli_config.output_format);
            print_output(&output, &cli_config.output_format, cli_config.quiet);
        }

        Commands::BatchGet { keys } => {
            let summary = client.batch_get(keys).await?;
            let output = format_batch_summary(&summary, "get", &cli_config.output_format);
            print_output(&output, &cli_config.output_format, cli_config.quiet);
        }

        Commands::BatchPut { entries } => {
            let parsed_entries: Result<Vec<(String, String)>, CliError> = entries
                .into_iter()
                .map(|entry| {
                    let parts: Vec<&str> = entry.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        Ok((parts[0].to_string(), parts[1].to_string()))
                    } else {
                        Err(CliError::ArgumentError(format!(
                            "Invalid entry format: '{}'. Expected 'key=value'",
                            entry
                        )))
                    }
                })
                .collect();

            let entries = parsed_entries?;
            let summary = client.batch_put(entries).await?;
            let output = format_batch_summary(&summary, "put", &cli_config.output_format);
            print_output(&output, &cli_config.output_format, cli_config.quiet);
        }

        Commands::BatchRemove { keys } => {
            let summary = client.batch_remove(keys).await?;
            let output = format_batch_summary(&summary, "remove", &cli_config.output_format);
            print_output(&output, &cli_config.output_format, cli_config.quiet);
        }

        Commands::Stats { detailed } => {
            let stats_json = if detailed {
                client.detailed_analytics().await?
            } else {
                client.stats().await?
            };
            let output = format_stats(&stats_json, &cli_config.output_format);
            print_output(&output, &cli_config.output_format, cli_config.quiet);
        }

        Commands::CompressionStats => {
            let compression_stats = client.get_compression_stats().await?;
            print_output(
                &compression_stats,
                &cli_config.output_format,
                cli_config.quiet,
            );
        }

        Commands::Strategy { action } => match action {
            StrategyAction::Metrics => {
                let metrics = client.get_strategy_metrics().await?;
                print_output(&metrics, &cli_config.output_format, cli_config.quiet);
            }
            StrategyAction::Thresholds => {
                let thresholds = client.get_strategy_thresholds().await?;
                print_output(&thresholds, &cli_config.output_format, cli_config.quiet);
            }
            StrategyAction::Force { strategy } => {
                client.force_cache_strategy(&strategy).await?;
                if !cli_config.quiet {
                    print_output(
                        &format!("Strategy forced to: {}", strategy),
                        &cli_config.output_format,
                        false,
                    );
                }
            }
        },

        Commands::Tasks { action } => match action {
            TaskAction::List => {
                let active_tasks = client.get_active_tasks().await?;
                print_output(&active_tasks, &cli_config.output_format, cli_config.quiet);
            }
            TaskAction::Cancel { task_id } => {
                let cancelled = client.cancel_task(task_id).await?;
                let message = if cancelled {
                    format!("Task {} cancelled successfully", task_id)
                } else {
                    format!("Task {} was not found or already completed", task_id)
                };
                print_output(&message, &cli_config.output_format, cli_config.quiet);
            }
            TaskAction::Stats => {
                let task_stats = client.get_task_coordinator_stats().await?;
                print_output(&task_stats, &cli_config.output_format, cli_config.quiet);
            }
        },

        Commands::Maintenance { action } => match action {
            MaintenanceAction::Breakdown => {
                let breakdown = client.get_maintenance_breakdown().await?;
                print_output(&breakdown, &cli_config.output_format, cli_config.quiet);
            }
            MaintenanceAction::Config => {
                let maintenance_config = client.get_maintenance_config_info().await?;
                print_output(
                    &maintenance_config,
                    &cli_config.output_format,
                    cli_config.quiet,
                );
            }
            MaintenanceAction::Start => {
                client.start_background_processor().await?;
                if !cli_config.quiet {
                    print_output(
                        "Background maintenance processor started",
                        &cli_config.output_format,
                        false,
                    );
                }
            }
        },

        Commands::System { action } => match action {
            SystemAction::Start => {
                client.start_system().await?;
                if !cli_config.quiet {
                    print_output("Cache system started", &cli_config.output_format, false);
                }
            }
            SystemAction::Stop => {
                client.stop_policy_engine().await?;
                if !cli_config.quiet {
                    print_output("Policy engine stopped", &cli_config.output_format, false);
                }
            }
            SystemAction::Shutdown => {
                client.shutdown_gracefully().await?;
                if !cli_config.quiet {
                    print_output(
                        "Cache system shutdown gracefully",
                        &cli_config.output_format,
                        false,
                    );
                }
            }
        },

        Commands::Config { action } => match action {
            ConfigAction::Show => {
                let config = client.get_config().await?;
                print_output(&config, &cli_config.output_format, cli_config.quiet);
            }
            ConfigAction::Update { parameter, value } => {
                client.update_config(&parameter, &value).await?;
                if !cli_config.quiet {
                    print_output(
                        &format!("Configuration updated: {} = {}", parameter, value),
                        &cli_config.output_format,
                        false,
                    );
                }
            }
            ConfigAction::Generate { output } => {
                let default_config = client.get_default_config().await?;
                std::fs::write(&output, default_config).map_err(|e| {
                    CliError::SystemError(format!("Failed to write config file: {}", e))
                })?;
                if !cli_config.quiet {
                    print_output(
                        &format!("Default configuration written to: {}", output.display()),
                        &cli_config.output_format,
                        false,
                    );
                }
            }
        },

        Commands::Repl => {
            // REPL mode is handled in the main binary, this should not be reached
            return Err(CliError::SystemError(
                "REPL mode should be handled before command execution".to_string(),
            ));
        }

        Commands::Completions { shell } => {
            // Generate completions - simplified implementation
            println!("# Goldylox CLI completions for {}", shell);
            println!("# Add this to your shell's configuration file");
            println!();

            // Provide basic completion setup instructions based on shell
            match shell.to_lowercase().as_str() {
                "bash" => {
                    println!("# Add this to ~/.bashrc:");
                    println!("# eval \"$(lox completions bash)\"");
                    println!();
                    println!(
                        "complete -W 'get put remove clear contains hash put-if-absent replace compare-and-swap get-or-insert batch-get batch-put batch-remove stats compression-stats strategy tasks maintenance system config repl completions' lox"
                    );
                }
                "zsh" => {
                    println!("# Add this to ~/.zshrc:");
                    println!("# eval \"$(lox completions zsh)\"");
                    println!();
                    println!("#compdef lox");
                    println!("_lox() {{");
                    println!("  local -a commands");
                    println!("  commands=(");
                    println!("    'get:Get a value from the cache'");
                    println!("    'put:Store a key-value pair in the cache'");
                    println!("    'remove:Remove a key from the cache'");
                    println!("    'clear:Clear all entries from the cache'");
                    println!("    'contains:Check if a key exists'");
                    println!("    'hash:Get hash value for a key'");
                    println!("    'stats:Get cache statistics'");
                    println!("    'repl:Start interactive REPL mode'");
                    println!("  )");
                    println!("  _describe 'commands' commands");
                    println!("}}");
                    println!("compdef _lox lox");
                }
                "fish" => {
                    println!("# Add this to ~/.config/fish/completions/lox.fish:");
                    println!(
                        "complete -c lox -n '__fish_use_subcommand' -a get -d 'Get a value from the cache'"
                    );
                    println!(
                        "complete -c lox -n '__fish_use_subcommand' -a put -d 'Store a key-value pair in the cache'"
                    );
                    println!(
                        "complete -c lox -n '__fish_use_subcommand' -a remove -d 'Remove a key from the cache'"
                    );
                    println!(
                        "complete -c lox -n '__fish_use_subcommand' -a clear -d 'Clear all entries from the cache'"
                    );
                    println!(
                        "complete -c lox -n '__fish_use_subcommand' -a contains -d 'Check if a key exists'"
                    );
                    println!(
                        "complete -c lox -n '__fish_use_subcommand' -a hash -d 'Get hash value for a key'"
                    );
                    println!(
                        "complete -c lox -n '__fish_use_subcommand' -a stats -d 'Get cache statistics'"
                    );
                    println!(
                        "complete -c lox -n '__fish_use_subcommand' -a repl -d 'Start interactive REPL mode'"
                    );
                }
                "powershell" | "pwsh" => {
                    println!("# Add this to your PowerShell profile:");
                    println!(
                        "Register-ArgumentCompleter -Native -CommandName lox -ScriptBlock {{{{"
                    );
                    println!("    param($commandName, $wordToComplete, $cursorPosition)");
                    println!(
                        "    $commands = @('get', 'put', 'remove', 'clear', 'contains', 'hash', 'stats', 'repl')"
                    );
                    println!(
                        "    $commands | Where-Object {{{{ $_ -like \"$wordToComplete*\" }}}} | ForEach-Object {{{{"
                    );
                    println!(
                        "        [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)"
                    );
                    println!("    }}}}");
                    println!("}}}}");
                }
                "elvish" => {
                    println!("# Add this to ~/.elvish/rc.elv:");
                    println!("edit:completion:arg-completer[lox] = [@words]{{");
                    println!("  commands = [get put remove clear contains hash stats repl]");
                    println!("  if (== (count $words) 2) {{");
                    println!("    put $@commands");
                    println!("  }}");
                    println!("}}");
                }
                _ => {
                    return Err(CliError::ArgumentError(format!(
                        "Unsupported shell: {}. Supported: bash, zsh, fish, powershell, elvish",
                        shell
                    )));
                }
            }

            if !cli_config.quiet {
                eprintln!("Shell completions generated for: {}", shell);
            }
        }
    }

    Ok(())
}
