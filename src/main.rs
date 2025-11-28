mod parser;
mod proc_reader;
mod nice_renice;
use nice_renice::{get_nice, set_nice, renice, spawn_with_nice};
mod filter;
mod proc_enhanced;
mod search_service;
mod history;
use filter::{ProcessFilter, FilterCondition};
use proc_reader::get_process_metrics;
use crate::proc_enhanced::EnhancedProcessMetrics;
use search_service::SearchService;
use proc_reader::{enumerate_processes, parse_process};
use parser::{Command, CommandParser};
use history::start_history_monitoring;
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
            "{:<8} {:<15} {:<10} {:<6} {:<10} {:<15} {:<15}",
            "PID", "Process", "User", "Nice", "%CPU", "Memory(KB)", "Read/Write (bytes)"
        );

        // Use enumerate_processes() here for unified collection
        let process_list = enumerate_processes();
        for process in process_list {
            // Assuming cpu_time is %CPU here as per ProcessInfo structure
            println!(
                "{:<8} {:<15} {:<10} {:<6} {:<10.2} {:<15} {:<7}/{}",
                process.pid,
                process.name,
                process.username,
                process.nice,
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
                let process_list = enumerate_processes();
                
                println!(
                    "{:<8} {:<20} {:<12} {:<6} {:<10} {:<12}",
                    "PID", "Name", "User", "Nice", "Memory(KB)", "CPU(ms)"
                );
                println!("{}", "-".repeat(80));

                for entry in std::fs::read_dir("/proc").unwrap() {
                    let entry = entry.unwrap();
                    let filename = entry.file_name();
                    if let Ok(pid) = filename.to_str().unwrap_or("").parse::<u32>() {
                        if let Ok(metrics) = get_process_metrics(pid) {
                            println!("{:?}", metrics);
                        }
                    }
                }

                for process in process_list {
                    println!(
                        "{:<8} {:<20} {:<12} {:<6} {:<10} {:<12}",
                        process.pid,
                        if process.name.len() > 20 {
                            format!("{}...", &process.name[..17])
                        } else {
                            process.name.clone()
                        },
                        if process.username.len() > 12 {
                            format!("{}...", &process.username[..9])
                        } else {
                            process.username.clone()
                        },
                        process.nice,
                        process.memory_kb,
                        process.cpu_time_ms
                    );
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
                    Some(process) => {
                        println!("\n=== Process Information ===");
                        println!("PID:          {}", process.pid);
                        println!("PPID:         {}", process.ppid);
                        println!("Name:         {}", process.name);
                        println!("User:         {} (UID: {})", process.username, process.uid);
                        println!("Nice:         {}", process.nice);
                        println!("State:        {}", process.state);
                        println!("Memory:       {} KB", process.memory_kb);
                        println!("CPU Time:     {} ms", process.cpu_time_ms);
                        println!("I/O Read:     {} bytes", process.io_read_bytes);
                        println!("I/O Write:    {} bytes", process.io_write_bytes);
                        if detailed {
                            println!("Command:      {}", process.command);
                        }
                        println!();
                    }
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
            Command::Renice { pid, nice } => {
                match nice {
                    None => {
                        // Get nice value
                        match get_nice(pid) {
                            Ok(nice_val) => println!("Process {} nice value: {}", pid, nice_val),
                            Err(e) => println!("Failed to get nice value for {}: {}", pid, e),
                        }
                    }
                    Some(nice_val) => {
                        // Set nice value
                        match set_nice(pid, nice_val) {
                            Ok(()) => println!("Set nice value for process {} to {}", pid, nice_val),
                            Err(e) => println!("Failed to set nice value for {}: {}", pid, e),
                        }
                    }
                }
            }
            Command::NiceStart { nice, cmd, args } => {
                match spawn_with_nice(nice, &cmd, &args) {
                    Ok(child) => {
                        println!(
                            "Started '{}' (PID {}) with nice {}",
                            cmd,
                            child.id(),
                            nice
                        );
                    }
                    Err(e) => {
                        eprintln!("Failed to start process with nice: {}", e);
                    }
                }
            }
            Command::SearchProcess { name, exact } => {
                println!("Searching for process '{}' (exact: {})", name, exact);
                let process_list = enumerate_processes();
                let mut found = false;
                
                println!(
                    "{:<8} {:<20} {:<12} {:<6} {:<10}",
                    "PID", "Name", "User", "Nice", "Memory(KB)"
                );
                println!("{}", "-".repeat(70));
                
                for process in process_list {
                    let matches = if exact {
                        process.name == name
                    } else {
                        process.name.to_lowercase().contains(&name.to_lowercase())
                    };
                    
                    if matches {
                        found = true;
                        println!(
                            "{:<8} {:<20} {:<12} {:<6} {:<10}",
                            process.pid,
                            process.name,
                            process.username,
                            process.nice,
                            process.memory_kb
                        );
                    }
                }
                
                if !found {
                    println!("No processes found matching '{}'", name);
                }
            }
            Command::FilterProcess { 
                name, 
                user, 
                min_cpu, 
                max_cpu, 
                min_mem, 
                max_mem, 
                state, 
                exact 
            } => {
                // Get all processes
                let all_processes = enumerate_processes();
                
                // Build filter
                let mut filter = ProcessFilter::new();
                
                if let Some(name_val) = name {
                    if exact {
                        filter.add_condition(FilterCondition::NameExact(name_val));
                    } else {
                        filter.add_condition(FilterCondition::NameContains(name_val));
                    }
                }
                
                if let Some(user_val) = user {
                    filter.add_condition(FilterCondition::User(user_val));
                }
                
                if let Some(min) = min_cpu {
                    filter.add_condition(FilterCondition::MinCpu(min));
                }
                
                if let Some(max) = max_cpu {
                    filter.add_condition(FilterCondition::MaxCpu(max));
                }
                
                if let Some(min) = min_mem {
                    filter.add_condition(FilterCondition::MinMemory(min));
                }
                
                if let Some(max) = max_mem {
                    filter.add_condition(FilterCondition::MaxMemory(max));
                }
                
                if let Some(state_val) = state {
                    filter.add_condition(FilterCondition::State(state_val));
                }
                
                // Convert ProcessInfo to EnhancedProcessMetrics
                let enhanced: Vec<EnhancedProcessMetrics> = all_processes
                    .into_iter()
                    .map(|p| EnhancedProcessMetrics {
                        pid: p.pid,
                        ppid: p.ppid,
                        uid: p.uid,
                        comm: p.name.clone(),
                        user: p.username.clone(),
                        state: p.state.clone(),
                        cpu_time: p.cpu_time_ms as f64 / 1000.0,
                        mem_usage: p.memory_kb,
                        io_read_bytes: p.io_read_bytes,
                        io_write_bytes: p.io_write_bytes,
                    })
                    .collect();
                
                // Apply filter
                let filtered = filter.filter_processes(enhanced);
                
                // Display results
                if filtered.is_empty() {
                    println!("No processes found matching the criteria");
                } else {
                    println!("\nFound {} matching processes:", filtered.len());
                    println!("{:<8} {:<20} {:<12} {:<8} {:<12} {:<10}", 
                            "PID", "Name", "User", "State", "CPU(s)", "Mem(KB)");
                    println!("{}", "-".repeat(80));
                    
                    for proc in filtered {
                        println!("{:<8} {:<20} {:<12} {:<8} {:<12.2} {:<10}",
                                proc.pid,
                                &proc.comm[..proc.comm.len().min(20)],
                                &proc.user[..proc.user.len().min(12)],
                                proc.state,
                                proc.cpu_time,
                                proc.mem_usage);
                    }
                }
            }
            Command::Monitor { interval } => {
                let interval = interval.clone();
                std::thread::spawn(move || {
                    monitor_processes(interval);
                });
                println!("Monitoring started in background.");
            }
            Command::Help => {
                show_help();
            }
            Command::Exit => {
                println!("Goodbye!");
                break;
            }
            Command::History { duration } => {
                if let Err(e) = start_history_monitoring(duration.unwrap_or(0)) {
                    println!("Error monitoring history: {}", e);
                }
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
    println!("  ps, list           - List processes");
    println!("  kill PID [SIGNAL]  - Kill process");
    println!("  info PID           - Show process information");
    println!("  stats              - Show system statistics");
    println!("  search, filter [OPTIONS] - Search/filter processes");
    println!("    -n, --name NAME    - Filter by name");
    println!("    -u, --user USER    - Filter by user");
    println!("    --min-cpu N        - Minimum CPU time (seconds)");
    println!("    --max-cpu N        - Maximum CPU time");
    println!("    --min-mem N        - Minimum memory (KB)");
    println!("    --max-mem N        - Maximum memory (KB)");
    println!("    -s, --state STATE  - Filter by state (R,S,D,Z,T)");
    println!("    -e, --exact        - Exact name match");
    println!("  nice N CMD [ARGS]  - Start CMD with nice value N");
    println!("  renice PID DELTA   - Adjust nice of PID by DELTA");;
    println!("  monitor [SEC]      - Live process monitor");
    println!("  history [SEC]      - Monitor finished processes (optional duration)");
    println!("  help               - Show this help");
    println!("  exit, quit         - Exit");
    println!();
}