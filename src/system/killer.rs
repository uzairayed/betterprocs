use anyhow::Result;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

pub fn kill_process(pid: u32, force: bool) -> Result<()> {
    let sig = if force {
        Signal::SIGKILL
    } else {
        Signal::SIGTERM
    };
    signal::kill(Pid::from_raw(pid as i32), sig)?;
    Ok(())
}
