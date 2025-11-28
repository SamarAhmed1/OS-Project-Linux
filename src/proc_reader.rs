use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

use users::get_user_by_uid;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub uid: u32,
    pub username: String,
    pub name: String,
    pub command: String,
    pub state: String,
    pub memory_kb: u64,
    pub cpu_time_ms: u64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    pub nice: i32,  // Added nice value
    pub timestamp: SystemTime,
}

// Helper to read the entire contents of a file as String
fn read_file(path: &str) -> io::Result<String> {
    fs::read_to_string(path)
}

/// Get the nice value from /proc/[pid]/stat
fn get_nice_from_stat(stat_parts: &[&str]) -> i32 {
    // Nice value is at index 18 in /proc/[pid]/stat
    // Format: ... priority nice ...
    stat_parts.get(18)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0)
}

pub fn parse_process(pid: u32) -> Option<ProcessInfo> {
    let stat_path = format!("/proc/{}/stat", pid);
    let status_path = format!("/proc/{}/status", pid);
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    let io_path = format!("/proc/{}/io", pid);

    // Read /proc/[pid]/stat
    let stat_content = read_file(&stat_path).ok()?;
    let stat_parts: Vec<&str> = stat_content.split_whitespace().collect();

    let name = stat_parts.get(1)?.trim_matches('(').trim_matches(')').to_string();
    let state = stat_parts.get(2)?.to_string();
    let ppid = stat_parts.get(3)?.parse().ok()?;
    let utime = stat_parts.get(13)?.parse::<u64>().ok()?;
    let stime = stat_parts.get(14)?.parse::<u64>().ok()?;
    let cpu_time_ms = (utime + stime) * 10; // Approximate; adjust if needed
    
    // Get nice value from stat
    let nice = get_nice_from_stat(&stat_parts);

    // Read /proc/[pid]/status for UID and memory
    let status_content = read_file(&status_path).ok()?;
    let mut uid: u32 = 0;
    let mut mem_kb: u64 = 0;

    for line in status_content.lines() {
        if line.starts_with("Uid:") {
            uid = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        } else if line.starts_with("VmRSS:") {
            mem_kb = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        }
    }

    // Map uid to username
    let username = if let Some(user) = get_user_by_uid(uid) {
        user.name().to_string_lossy().into_owned()
    } else {
        "unknown".to_string()
    };

    // Read /proc/[pid]/cmdline and clean it up
    let command = read_file(&cmdline_path).unwrap_or_default().replace('\0', " ");

    // Read /proc/[pid]/io for I/O bytes
    let io_content = read_file(&io_path).unwrap_or_default();
    let mut io_read = 0;
    let mut io_write = 0;
    for line in io_content.lines() {
        if line.starts_with("read_bytes:") {
            io_read = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        } else if line.starts_with("write_bytes:") {
            io_write = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        }
    }

    Some(ProcessInfo {
        pid,
        ppid,
        uid,
        username,
        name,
        command,
        state,
        memory_kb: mem_kb,
        cpu_time_ms,
        io_read_bytes: io_read,
        io_write_bytes: io_write,
        nice,
        timestamp: SystemTime::now(),
    })
}

pub fn enumerate_processes() -> Vec<ProcessInfo> {
    let mut results = Vec::new();
    let proc = Path::new("/proc");

    let entries = match fs::read_dir(proc) {
        Ok(e) => e,
        Err(_) => return results,
    };

    for entry in entries {
        if let Ok(entry) = entry {
            let file_name = entry.file_name();
            let pid_str = file_name.to_string_lossy();

            if !pid_str.chars().all(|c| c.is_numeric()) {
                continue;
            }

            if let Ok(pid) = pid_str.parse::<u32>() {
                if let Some(info) = parse_process(pid) {
                    results.push(info);
                }
            }
        }
    }
    results
}