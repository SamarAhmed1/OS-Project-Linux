use libc::{self, c_int};

/// Get the current nice value of a process (returns i32 in range [-20, 19])
pub fn get_nice(pid: u32) -> Result<i32, String> {
    errno::set_errno(errno::Errno(0));
    let prio = unsafe { libc::getpriority(libc::PRIO_PROCESS, pid) };
    let err = errno::errno().0;

    if err != 0 {
        return Err(format!("getpriority failed with errno {}", err));
    }
    Ok(prio)
}

/// Set absolute nice value (e.g. -5, 0, 10)
pub fn set_nice(pid: u32, nice: i32) -> Result<(), String> {
    let res = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid, nice as c_int) };
    if res == -1 {
        let err = errno::errno().0;
        return Err(format!("setpriority failed with errno {}", err));
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
