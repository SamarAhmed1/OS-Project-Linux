mod parser;
mod proc_reader;

use proc_reader::get_process_metrics;
use parser::{Command, CommandParser};
use std::io::{self, Write};

fn main() {
    println!("Linux Process Manager - Rust Edition");
    println!("Type 'help' for available commands, 'exit' to quit\n");

    let parser = CommandParser::new();
    let mut input = String::new();

    loop {
        print!("lpm> ");
        io::stdout().flush().unwrap();
        
        input.clear();
        io::stdin().read_line(&mut input).unwrap();
        
        let result = parser.parse(&input);
        
        match result.command {
            Command::ListProcesses { all, user, sort_by } => {
                // Example: List all PIDs from /proc
                for entry in std::fs::read_dir("/proc").unwrap() {
                    let entry = entry.unwrap();
                    let filename = entry.file_name();
                    if let Ok(pid) = filename.to_str().unwrap_or("").parse::<u32>() {
                        if let Ok(metrics) = get_process_metrics(pid) {
                            println!("{:?}", metrics);
                        }
                    }
                }
            }
            Command::KillProcess { pid, signal } => {
                println!("Killing process {} with signal {:?}", pid, signal);
                // TODO: Implement actual process killing
            }
            Command::ProcessInfo { pid, detailed } => {
                match get_process_metrics(pid) {
                    Ok(metrics) => println!("{:?}", metrics),
                    Err(e) => println!("Error reading process metrics: {}", e),
                }
            }
            Command::SystemStats { refresh_interval } => {
                println!("Showing system stats (refresh: {:?})", refresh_interval);
                // TODO: Implement actual system stats
            }
            Command::SearchProcess { name, exact } => {
                println!("Searching for process '{}' (exact: {})", name, exact);
                // TODO: Implement actual process search
            }
            Command::Help => {
                show_help();
            }
            Command::Exit => {
                println!("Goodbye!");
                break;
            }
            Command::Unknown(cmd) => {
                println!("Unknown command: {}", cmd);
                show_help();
            }
        }
    }
}

fn show_help() {
    println!("\nAvailable commands:");
    println!("  ps, list           - List processes (flags: -a/--all, -u/--user USER, -s/--sort FIELD)");
    println!("  kill PID [SIGNAL]  - Kill process with optional signal");
    println!("  info, show PID     - Show process information (flags: -d/--detailed)");
    println!("  stats, status      - Show system statistics (flags: --refresh SECONDS)");
    println!("  search, find NAME  - Search for process by name (flags: -e/--exact)");
    println!("  help               - Show this help message");
    println!("  exit, quit         - Exit the program");
    println!();
}