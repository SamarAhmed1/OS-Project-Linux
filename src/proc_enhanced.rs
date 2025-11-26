use std::fs;
use std::io;
use libc;

#[derive(Debug, Clone, Default)]
pub struct EnhancedProcessMetrics {
    pub pid: u32,
    pub comm: String,
    pub user: String,
    pub uid: u32,
    pub cpu_time: f64,
    pub mem_usage: u64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    pub state: String,
    pub ppid: u32,
}

fn read_file(path: &str) -> io::Result<String> {
    fs::read_to_string(path)
}

fn parse_enhanced_stat(pid: u32) -> io::Result<(String, f64, String, u32)> {
    let stat_path = format!("/proc/{}/stat", pid);
    let stat_content = fs::read_to_string(&stat_path)?;
    let parts: Vec<&str> = stat_content.split_whitespace().collect();

    let comm = parts[1].trim_matches('(').trim_matches(')');
    let state = parts[2].to_string();
    let ppid = parts[3].parse::<u32>().unwrap_or(0);

    let utime = parts[13].parse::<u64>().unwrap_or(0);
    let stime = parts[14].parse::<u64>().unwrap_or(0);
    let starttime = parts[21].parse::<u64>().unwrap_or(0);

    let ticks_per_sec = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
    let uptime_content = fs::read_to_string("/proc/uptime")?;
    let uptime = uptime_content
        .split_whitespace()
        .next()
        .unwrap()
        .parse::<f64>()
        .unwrap();

    let elapsed_seconds = uptime - (starttime as f64 / ticks_per_sec);
    let total_cpu_time_seconds = (utime as f64 + stime as f64) / ticks_per_sec;
    let percent_cpu = if elapsed_seconds > 0.0 {
        ((total_cpu_time_seconds / elapsed_seconds) * 100.0).round()
    } else {
        0.0
    };

    Ok((comm.to_string(), percent_cpu, state, ppid))
}

fn parse_enhanced_status(pid: u32) -> io::Result<(u64, u32)> {
    let status_path = format!("/proc/{}/status", pid);
    let status = read_file(&status_path)?;
    let mut mem_usage = 0;
    let mut uid = 0;

    for line in status.lines() {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            mem_usage = parts[1].parse::<u64>().unwrap_or(0);
        } else if line.starts_with("Uid:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            uid = parts[1].parse::<u32>().unwrap_or(0);
        }
    }
    Ok((mem_usage, uid))
}

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

pub fn get_enhanced_process_metrics(pid: u32) -> io::Result<EnhancedProcessMetrics> {
    let (comm, cpu_time, state, ppid) = parse_enhanced_stat(pid)?;
    let (mem_usage, uid) = parse_enhanced_status(pid)?;
    let (io_read_bytes, io_write_bytes) = parse_io(pid)?;

    let user = if uid == 0 { 
        "root".to_string() 
    } else { 
        format!("user{}", uid) 
    };

    Ok(EnhancedProcessMetrics {
        pid,
        comm,
        user,
        uid,
        cpu_time,
        mem_usage,
        io_read_bytes,
        io_write_bytes,
        state,
        ppid,
    })
}

pub fn get_all_enhanced_processes() -> io::Result<Vec<EnhancedProcessMetrics>> {
    let mut processes = Vec::new();
    
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let filename = entry.file_name();
        
        if let Ok(pid) = filename.to_str().unwrap_or("").parse::<u32>() {
            if let Ok(metrics) = get_enhanced_process_metrics(pid) {
                processes.push(metrics);
            }
        }
    }
    
    Ok(processes)
}