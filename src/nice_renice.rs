use libc::{self, c_int};
use std::process::{Child, Command as ProcessCommand};

/// Get the current nice value of a process (returns i32 in range [-20, 19])
pub fn get_nice(pid: u32) -> Result<i32, String> {
    // Clear errno before the call
    unsafe { *libc::__errno_location() = 0; }
    
    // On Linux, getpriority actually returns (20 - nice_value) as the priority
    // But we want the nice value, so we convert it
    let result = unsafe { libc::getpriority(libc::PRIO_PROCESS, pid as u32) };
    let err = unsafe { *libc::__errno_location() };

    // Check errno to detect errors (since -1 can be a valid return)
    if err != 0 {
        return Err(format!("getpriority failed with errno {}: {}", err,
            match err {
                1 => "Operation not permitted",
                3 => "No such process",
                22 => "Invalid argument",
                _ => "Unknown error"
            }
        ));
    }
    
    // getpriority returns (20 - nice), so nice = 20 - result
    Ok(20 - result)
}

/// Set absolute nice value (e.g. -5, 0, 10)
pub fn set_nice(pid: u32, nice: i32) -> Result<(), String> {
    // Clamp the nice value to valid range
    let nice = nice.clamp(-20, 19);
    
    let res = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as u32, nice as c_int) };
    if res == -1 {
        let err = unsafe { *libc::__errno_location() };
        return Err(format!("setpriority failed with errno {}: {}", err, 
            match err {
                1 => "Operation not permitted (need root/CAP_SYS_NICE)",
                3 => "No such process",
                22 => "Invalid argument",
                _ => "Unknown error"
            }
        ));
    }
    Ok(())
}

/// Renice relative to current value (e.g. +5, -3)
pub fn renice(pid: u32, delta: i32) -> Result<i32, String> {
    let current = get_nice(pid)?;
    let new_nice = (current + delta).clamp(-20, 19);
    set_nice(pid, new_nice)?;
    Ok(new_nice)
}

/// Spawn a new process with the given nice value
pub fn spawn_with_nice(nice: i32, cmd: &str, args: &[String]) -> Result<Child, String> {
    let child = ProcessCommand::new(cmd)  // Remove 'mut'
        .args(args)
        .spawn()
        .map_err(|e| format!("failed to start '{}': {}", cmd, e))?;
    // Set the child's nice value (absolute)
    if let Err(e) = set_nice(child.id(), nice) {
        eprintln!(
            "Warning: started process {} ('{}'), but failed to set nice: {}",
            child.id(),
            cmd,
            e
        );
    }

    Ok(child)
}