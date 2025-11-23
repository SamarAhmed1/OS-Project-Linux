mod parser;
mod proc_reader;

use proc_reader::{enumerate_processes, parse_process};
use parser::{Command, CommandParser};

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

use sysinfo::CpuExt;
use sysinfo::{System, SystemExt};
use std::{thread, time};

use std::io::{self, Write};
use std::fs;

fn get_memory_stats() -> (u64, u64) {
    let meminfo = fs::read_to_string("/proc/meminfo").unwrap();
    let mut total = 0;
    let mut free = 0;
    let mut buffers = 0;
    let mut cached = 0;

    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            total = line.split_whitespace().nth(1).unwrap().parse::<u64>().unwrap();
        } else if line.starts_with("MemFree:") {
            free = line.split_whitespace().nth(1).unwrap().parse::<u64>().unwrap();
        } else if line.starts_with("Buffers:") {
            buffers = line.split_whitespace().nth(1).unwrap().parse::<u64>().unwrap();
        } else if line.starts_with("Cached:") {
            cached = line.split_whitespace().nth(1).unwrap().parse::<u64>().unwrap();
        }
    }
    let used = total - free - buffers - cached;
    // Values are in KB. Divide by 1024 for MB.
    (total / 1024, used / 1024)
}

fn monitor_processes(interval: u64) {
    loop {
        // Clear screen (optional for nice display)
        print!("\x1B[2J\x1B[H");

        println!(
            "{:<8} {:<15} {:<10} {:<10} {:<15} {:<15}",
            "PID", "Process", "User", "%CPU", "Memory(KB)", "Read/Write (bytes)"
        );

        // Use enumerate_processes() here for unified collection
        let process_list = enumerate_processes();
        for process in process_list {
            // Assuming cpu_time is %CPU here as per ProcessInfo structure
            println!(
                "{:<8} {:<15} {:<10} {:<10.2} {:<15} {:<7}/{}",
                process.pid,
                process.name,
                process.username,
                process.cpu_time_ms as f64 / 1000.0, // assuming cpu_time_ms stored in ms
                process.memory_kb,
                process.io_read_bytes,
                process.io_write_bytes
            );
        }

        thread::sleep(time::Duration::from_secs(interval));
    }
}

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
                // Use enumerate_processes() here too
                let process_list = enumerate_processes();
                // TODO: implement filtering by user and sorting later
                for process in process_list {
                    println!("{:?}", process);
                }
            }
            Command::KillProcess { pid, signal } => {
                let sig = match signal.as_deref() {
                    Some("SIGTERM") => Signal::SIGTERM,
                    Some("SIGKILL") => Signal::SIGKILL,
                    _ => Signal::SIGTERM,
                };
                match signal::kill(Pid::from_raw(pid as i32), sig) {
                    Ok(_) => println!("Successfully killed process {}", pid),
                    Err(e) => println!("Failed to kill process {}: {}", pid, e),
                }
            }
            Command::ProcessInfo { pid, detailed } => {
                match proc_reader::parse_process(pid) {
                    Some(metrics) => println!("{:?}", metrics),
                    None => println!("Error reading process metrics for pid {}", pid),
                }
            }
            Command::SystemStats { refresh_interval } => {
                let interval = refresh_interval.unwrap_or(0);
                if interval > 0 {
                    let mut sys = System::new_all();
                    loop {
                        sys.refresh_all();
                        let (total_mb, used_mb) = get_memory_stats();
                        print!("\x1B[2J\x1B[H"); // Clear screen
                        println!("Total memory: {} MB", total_mb);
                        println!("Used memory: {} MB", used_mb);
                        println!("CPU usage: {:.2}%", sys.global_cpu_info().cpu_usage());
                        std::io::stdout().flush().unwrap();
                        std::thread::sleep(std::time::Duration::from_secs(interval));
                    }
                } else {
                    let mut sys = System::new_all();
                    sys.refresh_all();
                    let (total_mb, used_mb) = get_memory_stats();
                    println!("Total memory: {} MB", total_mb);
                    println!("Used memory: {} MB", used_mb);
                    println!("CPU usage: {:.2}%", sys.global_cpu_info().cpu_usage());
                }
            }
            Command::SearchProcess { name, exact } => {
                println!("Searching for process '{}' (exact: {})", name, exact);
                // TODO: Implement actual process search
            }
            Command::Monitor { interval } => {
                monitor_processes(interval);
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
    println!("  Monitor (Seconds)  - Live process monitor (refresh every N seconds)");
    println!("  help               - Show this help message");
    println!("  exit, quit         - Exit the program");
    println!();
}
