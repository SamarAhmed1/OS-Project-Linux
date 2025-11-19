use std::fs;
use std::io::{self, BufRead};
use std::path::Path;

#[derive(Debug)]
pub struct ProcessMetrics {
    pub pid: u32,
    pub comm: String,
    pub user: String,
    pub cpu_time: u64,
    pub mem_usage: u64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
}

// Helper to read the entire contents of a file as String
fn read_file(path: &str) -> io::Result<String> {
    fs::read_to_string(path)
}

// Parse /proc/[pid]/stat for CPU and command
fn parse_stat(pid: u32) -> io::Result<(String, u64)> {
    let stat_path = format!("/proc/{}/stat", pid);
    let stat = read_file(&stat_path)?;
    let parts: Vec<&str> = stat.split_whitespace().collect();
    let comm = parts[1].trim_matches('(').trim_matches(')');
    // utime + stime (fields 13 and 14) -- example for total CPU time
    let utime = parts[13].parse::<u64>().unwrap_or(0);
    let stime = parts[14].parse::<u64>().unwrap_or(0);
    Ok((comm.to_string(), utime + stime))
}

// Parse /proc/[pid]/status for memory usage
fn parse_status(pid: u32) -> io::Result<u64> {
    let status_path = format!("/proc/{}/status", pid);
    let status = read_file(&status_path)?;
    for line in status.lines() {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            return Ok(parts[1].parse::<u64>().unwrap_or(0));
        }
    }
    Ok(0)
}

// Parse /proc/[pid]/io for I/O stats
fn parse_io(pid: u32) -> io::Result<(u64, u64)> {
    let io_path = format!("/proc/{}/io", pid);
    let io = read_file(&io_path)?;
    let mut read_bytes = 0;
    let mut write_bytes = 0;
    for line in io.lines() {
        if line.starts_with("read_bytes:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            read_bytes = parts[1].parse::<u64>().unwrap_or(0);
        } else if line.starts_with("write_bytes:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            write_bytes = parts[1].parse::<u64>().unwrap_or(0);
        }
    }
    Ok((read_bytes, write_bytes))
}

// Combine all metrics above
pub fn get_process_metrics(pid: u32) -> io::Result<ProcessMetrics> {
    let (comm, cpu_time) = parse_stat(pid)?;
    let mem_usage = parse_status(pid)?;
    let (io_read_bytes, io_write_bytes) = parse_io(pid)?;

    // For user name, simplified (real code: get UID from /proc/[pid]/status and map to user)
    let user = "user".to_string();

    Ok(ProcessMetrics {
        pid,
        comm,
        user,
        cpu_time,
        mem_usage,
        io_read_bytes,
        io_write_bytes,
    })
}
