use anyhow::Result;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

/// Send SIGTERM to the entire process group (not just the shell).
/// This is the core fix over mprocs — killpg hits the full process tree.
pub fn terminate_process_group(pid: u32) -> Result<()> {
    let pgid = Pid::from_raw(pid as i32);
    signal::killpg(pgid, Signal::SIGTERM)?;
    Ok(())
}

/// Force-kill the entire process group with SIGKILL.
pub fn force_kill_process_group(pid: u32) -> Result<()> {
    let pgid = Pid::from_raw(pid as i32);
    signal::killpg(pgid, Signal::SIGKILL)?;
    Ok(())
}

/// Check if a process is still alive.
pub fn is_process_alive(pid: u32) -> bool {
    // Sending signal 0 checks if process exists without actually sending a signal
    signal::kill(Pid::from_raw(pid as i32), None).is_ok()
}
