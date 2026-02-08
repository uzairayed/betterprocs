use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::sync::mpsc;

use super::signal;
use super::types::{ProcessConfig, ProcessStatus};
use crate::terminal::screen::TerminalScreen;

pub struct ProcessHandle {
    pub config: ProcessConfig,
    pub status: ProcessStatus,
    pub screen: TerminalScreen,
    child: Option<Box<dyn portable_pty::Child + Send>>,
    master_pty: Option<Box<dyn portable_pty::MasterPty + Send>>,
    output_rx: Option<std::sync::mpsc::Receiver<Vec<u8>>>,
    reader_thread: Option<std::thread::JoinHandle<()>>,
}

impl ProcessHandle {
    pub fn new(config: ProcessConfig, rows: u16, cols: u16) -> Self {
        let autostart = config.autostart;
        let mut handle = Self {
            config,
            status: ProcessStatus::NotStarted,
            screen: TerminalScreen::new(rows, cols, 10_000),
            child: None,
            master_pty: None,
            output_rx: None,
            reader_thread: None,
        };

        if autostart {
            let _ = handle.spawn();
        }

        handle
    }

    pub fn spawn(&mut self) -> Result<()> {
        if self.status.is_running() {
            self.stop(true)?;
        }

        let pty_system = native_pty_system();
        let pty_pair = pty_system
            .openpty(PtySize {
                rows: self.screen.rows(),
                cols: self.screen.cols(),
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to open PTY")?;

        let mut cmd = if let Some(ref args) = self.config.cmd {
            let mut builder = CommandBuilder::new(&args[0]);
            for arg in &args[1..] {
                builder.arg(arg);
            }
            builder
        } else {
            let mut builder = CommandBuilder::new("sh");
            builder.args(["-c", &self.config.command]);
            builder
        };

        if let Some(ref cwd) = self.config.cwd {
            cmd.cwd(cwd);
        }

        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        cmd.env("TERM", "xterm-256color");

        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn process")?;

        let pid = child.process_id().unwrap_or(0);

        let reader = pty_pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;

        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        let reader_thread = std::thread::spawn(move || {
            read_pty_output(reader, tx);
        });

        self.child = Some(child);
        self.master_pty = Some(pty_pair.master);
        self.output_rx = Some(rx);
        self.reader_thread = Some(reader_thread);
        self.status = ProcessStatus::Running { pid };
        self.screen.scroll_to_bottom();

        Ok(())
    }

    pub fn stop(&mut self, graceful: bool) -> Result<()> {
        if let ProcessStatus::Running { pid } = self.status {
            if graceful {
                let _ = signal::terminate_process_group(pid);
                std::thread::sleep(std::time::Duration::from_millis(100));
                if signal::is_process_alive(pid) {
                    let _ = signal::force_kill_process_group(pid);
                }
            } else {
                let _ = signal::force_kill_process_group(pid);
            }

            if let Some(ref mut child) = self.child {
                match child.wait() {
                    Ok(exit_status) => {
                        let code = exit_status
                            .exit_code()
                            .try_into()
                            .unwrap_or(-1);
                        self.status = ProcessStatus::Stopped { exit_code: code };
                    }
                    Err(_) => {
                        self.status = ProcessStatus::Crashed {};
                    }
                }
            }

            self.child = None;
            self.master_pty = None;
            self.output_rx = None;
            self.reader_thread = None;
        }

        Ok(())
    }

    pub fn restart(&mut self) -> Result<()> {
        self.stop(true)?;
        self.spawn()
    }

    pub fn write_input(&mut self, data: &[u8]) -> Result<()> {
        if let Some(ref mut master) = self.master_pty {
            let mut writer = master.take_writer()?;
            use std::io::Write;
            writer.write_all(data)?;
        }
        Ok(())
    }

    pub fn drain_output(&mut self) -> bool {
        let mut had_output = false;

        if let Some(ref rx) = self.output_rx {
            while let Ok(data) = rx.try_recv() {
                self.screen.process_bytes(&data);
                had_output = true;
            }
        }

        if let Some(ref mut child) = self.child {
            if let Ok(Some(exit_status)) = child.try_wait() {
                let code: i32 = exit_status.exit_code().try_into().unwrap_or(-1);
                if code == 0 {
                    self.status = ProcessStatus::Stopped { exit_code: code };
                } else {
                    self.status = ProcessStatus::Crashed {};
                }
                self.child = None;
                self.master_pty = None;
            }
        }

        had_output
    }

    pub fn resize_pty(&mut self, rows: u16, cols: u16) {
        // Only grow the vt100 screen, never shrink it — shrinking destroys
        // content at the right edge that can't be recovered on re-enlarge.
        let screen_cols = self.screen.cols().max(cols);
        let screen_rows = self.screen.rows().max(rows);
        self.screen.resize(screen_rows, screen_cols);

        // But resize the actual PTY to match the pane so running processes
        // get the correct SIGWINCH and format their output to fit.
        if let Some(ref master) = self.master_pty {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }
}

fn read_pty_output(mut reader: Box<dyn Read + Send>, tx: mpsc::Sender<Vec<u8>>) {
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}
