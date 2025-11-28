use std::fs;
use std::io;
use std::collections::HashMap;
use std::time::{SystemTime, Duration, Instant};
use std::thread;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub command: String,
    pub start_time: SystemTime,
    pub start_instant: Instant,
    pub cpu_time_sec: f64,
}

#[derive(Debug, Clone)]
pub struct FinishedProcess {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub command: String,
    pub start_time: SystemTime,
    pub end_time: SystemTime,
    pub duration: Duration,
    pub cpu_time_sec: f64,
}

pub struct ProcessHistoryTracker {
    active_processes: HashMap<u32, ProcessInfo>,
    finished_processes: Vec<FinishedProcess>,
    boot_time: u64,
    clock_ticks: u64,
}

impl ProcessHistoryTracker {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            active_processes: HashMap::new(),
            finished_processes: Vec::new(),
            boot_time: Self::get_boot_time()?,
            clock_ticks: Self::get_clock_ticks(),
        })
    }

    /// Start monitoring - takes initial snapshot
    pub fn start_monitoring(&mut self) -> io::Result<()> {
        self.active_processes = self.scan_processes()?;
        println!("Started monitoring {} processes", self.active_processes.len());
        Ok(())
    }

    /// Update - check for finished processes
    pub fn update(&mut self) -> io::Result<()> {
        let current_processes = self.scan_processes()?;
        let now = SystemTime::now();
        let now_instant = Instant::now();

        // Find processes that are no longer running
        for (pid, old_info) in &self.active_processes {
            if !current_processes.contains_key(pid) {
                // Process has finished
                let duration = now_instant.duration_since(old_info.start_instant);
                
                self.finished_processes.push(FinishedProcess {
                    pid: *pid,
                    name: old_info.name.clone(),
                    user: old_info.user.clone(),
                    command: old_info.command.clone(),
                    start_time: old_info.start_time,
                    end_time: now,
                    duration,
                    cpu_time_sec: old_info.cpu_time_sec,
                });
            }
        }

        // Update active processes
        self.active_processes = current_processes;
        Ok(())
    }

    /// Scan all current processes
    fn scan_processes(&self) -> io::Result<HashMap<u32, ProcessInfo>> {
        let mut processes = HashMap::new();
        let now = SystemTime::now();
        let now_instant = Instant::now();

        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            let filename = entry.file_name();
            let name = filename.to_string_lossy();

            if let Ok(pid) = name.parse::<u32>() {
                if let Ok(info) = self.read_process_info(pid, now, now_instant) {
                    processes.insert(pid, info);
                }
            }
        }

        Ok(processes)
    }

    /// Read process information
    fn read_process_info(&self, pid: u32, now: SystemTime, now_instant: Instant) -> io::Result<ProcessInfo> {
        let stat_path = format!("/proc/{}/stat", pid);
        let cmdline_path = format!("/proc/{}/cmdline", pid);
        let status_path = format!("/proc/{}/status", pid);

        // Read stat file
        let stat_content = fs::read_to_string(&stat_path)?;
        let (name, utime, stime, _starttime) = Self::parse_stat(&stat_content)?;

        // Read command line
        let command = fs::read_to_string(&cmdline_path)
            .unwrap_or_default()
            .replace('\0', " ")
            .trim()
            .to_string();

        // Get username
        let user = Self::get_process_user(&status_path).unwrap_or_else(|_| "unknown".to_string());

        // Calculate CPU time in seconds
        let cpu_time_sec = (utime + stime) as f64 / self.clock_ticks as f64;

        Ok(ProcessInfo {
            pid,
            name: name.clone(),
            user,
            command: if command.is_empty() { name } else { command },
            start_time: now,
            start_instant: now_instant,
            cpu_time_sec,
        })
    }

    /// Parse /proc/[pid]/stat
    fn parse_stat(content: &str) -> io::Result<(String, u64, u64, u64)> {
        let start = content.find('(').ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid stat format")
        })?;
        let end = content.rfind(')').ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid stat format")
        })?;

        let name = content[start + 1..end].to_string();
        let after_name = &content[end + 2..];
        let parts: Vec<&str> = after_name.split_whitespace().collect();

        if parts.len() < 20 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Not enough fields"));
        }

        let utime: u64 = parts[11].parse().unwrap_or(0);
        let stime: u64 = parts[12].parse().unwrap_or(0);
        let starttime: u64 = parts[19].parse().unwrap_or(0);

        Ok((name, utime, stime, starttime))
    }

    /// Get process owner
    fn get_process_user(status_path: &str) -> io::Result<String> {
        let content = fs::read_to_string(status_path)?;
        
        for line in content.lines() {
            if line.starts_with("Uid:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let uid: u32 = parts[1].parse().unwrap_or(0);
                    return Ok(Self::get_username_from_uid(uid));
                }
            }
        }
        Ok("unknown".to_string())
    }

    /// Convert UID to username
    fn get_username_from_uid(uid: u32) -> String {
        if let Ok(content) = fs::read_to_string("/etc/passwd") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    if let Ok(file_uid) = parts[2].parse::<u32>() {
                        if file_uid == uid {
                            return parts[0].to_string();
                        }
                    }
                }
            }
        }
        uid.to_string()
    }

    /// Get system boot time
    fn get_boot_time() -> io::Result<u64> {
        let content = fs::read_to_string("/proc/stat")?;
        for line in content.lines() {
            if line.starts_with("btime") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    return parts[1].parse().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "Invalid btime")
                    });
                }
            }
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "btime not found"))
    }

    /// Get clock ticks per second
    fn get_clock_ticks() -> u64 {
        unsafe { libc::sysconf(libc::_SC_CLK_TCK) as u64 }
    }

    /// Display finished processes history
    pub fn show_history(&self) {
        println!("\n{:-<130}", "");
        println!("{:<8} {:<15} {:<20} {:<20} {:<12} {:<12} {:<40}",
            "PID", "User", "Start Time", "End Time", "Duration", "CPU Time", "Command");
        println!("{:-<130}", "");

        if self.finished_processes.is_empty() {
            println!("No processes have finished yet. Keep the monitor running...");
        } else {
            for proc in &self.finished_processes {
                let start_fmt = format_time(proc.start_time);
                let end_fmt = format_time(proc.end_time);
                let duration_fmt = format!("{:.2}s", proc.duration.as_secs_f64());
                let cpu_fmt = format!("{:.2}s", proc.cpu_time_sec);
                let cmd = truncate(&proc.command, 40);

                println!("{:<8} {:<15} {:<20} {:<20} {:<12} {:<12} {:<40}",
                    proc.pid,
                    truncate(&proc.user, 15),
                    start_fmt,
                    end_fmt,
                    duration_fmt,
                    cpu_fmt,
                    cmd
                );
            }
        }

        println!("{:-<130}", "");
        println!("Total finished processes: {}\n", self.finished_processes.len());
    }

    pub fn get_finished_count(&self) -> usize {
        self.finished_processes.len()
    }
}

fn format_time(time: SystemTime) -> String {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => {
            let secs = duration.as_secs();
            let hours = (secs / 3600) % 24;
            let minutes = (secs / 60) % 60;
            let seconds = secs % 60;
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        }
        Err(_) => "Unknown".to_string()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}

pub fn start_history_monitoring(duration_secs: u64) -> io::Result<()> {


    println!("Linux Finished Process History Tracker");
    println!("=======================================\n");

    if !std::path::Path::new("/proc").exists() {
        eprintln!("Error: /proc not found. This tool only works on Linux.");
        return Err(io::Error::new(io::ErrorKind::NotFound, "Not running on Linux"));
    }

    let mut tracker = ProcessHistoryTracker::new()?;
    tracker.start_monitoring()?;

   

    
    println!("Run some commands in another terminal to see them appear here when they finish.\n");

    let mut counter = 0;
    let max_iterations = duration_secs * 2;
       while counter < max_iterations {
        tracker.update()?;
        counter += 1;

        // Show finished count every 5 seconds
        if counter % 10 == 0 {
            let count = tracker.get_finished_count();
            print!("\r{} finished processes detected...", count);
            std::io::Write::flush(&mut std::io::stdout()).ok();
        }

        // Show history every 10 seconds
        if counter % 20 == 0 && tracker.get_finished_count() > 0 {
            println!("\n");
            tracker.show_history();
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    // Show final history before exiting
    println!("\nFinal History:");
    tracker.show_history();

    Ok(())
}