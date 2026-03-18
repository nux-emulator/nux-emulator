//! crosvm process spawning, monitoring, and lifecycle management.

#![allow(unsafe_code)]

use super::error::{VmError, VmResult};
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::time::{Duration, timeout};

/// Manages a crosvm child process.
#[derive(Debug)]
pub struct VmProcess {
    child: Child,
    pid: u32,
    pid_file: PathBuf,
    socket_path: PathBuf,
}

impl VmProcess {
    /// Spawn a new crosvm process from the given command arguments.
    ///
    /// # Errors
    ///
    /// Returns `VmError::CrosvmNotFound` if the binary doesn't exist,
    /// or `VmError::Io` on other spawn failures.
    pub async fn spawn(
        args: &[std::ffi::OsString],
        pid_file: PathBuf,
        socket_path: PathBuf,
    ) -> VmResult<Self> {
        // Clean up any orphaned process first
        cleanup_orphan(&pid_file).await;

        // Create parent directories for PID file and socket
        if let Some(parent) = pid_file.parent() {
            std::fs::create_dir_all(parent).map_err(VmError::Io)?;
        }
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent).map_err(VmError::Io)?;
        }

        let program = &args[0];
        let cmd_args = &args[1..];

        let child = unsafe {
            Command::new(program)
                .args(cmd_args)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .pre_exec(|| {
                    // If parent dies, child gets SIGTERM
                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                    Ok(())
                })
                .spawn()
        };

        let child = match child {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(VmError::CrosvmNotFound(program.into()));
            }
            Err(e) => return Err(VmError::Io(e)),
        };

        let pid = child.id().unwrap_or(0);

        // Write PID file
        std::fs::write(&pid_file, pid.to_string()).map_err(VmError::Io)?;

        Ok(Self {
            child,
            pid,
            pid_file,
            socket_path,
        })
    }

    /// Get the process ID.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Wait for the process to exit, returning (`exit_code`, `stderr`).
    ///
    /// # Errors
    ///
    /// Returns `VmError::Io` if waiting on the process fails.
    pub async fn wait(&mut self) -> VmResult<(i32, String)> {
        let status = self.child.wait().await.map_err(VmError::Io)?;
        let stderr = self.read_stderr().await;
        let code = status.code().unwrap_or(-1);
        Ok((code, stderr))
    }

    /// Wait for exit with a timeout. Returns `None` if the timeout expires.
    ///
    /// # Errors
    ///
    /// Returns `VmError::Io` if waiting on the process fails.
    pub async fn wait_timeout(&mut self, duration: Duration) -> VmResult<Option<(i32, String)>> {
        match timeout(duration, self.wait()).await {
            Ok(result) => result.map(Some),
            Err(_) => Ok(None),
        }
    }

    /// Send SIGTERM to the process.
    ///
    /// # Errors
    ///
    /// Returns `VmError::ProcessSignal` if the signal cannot be sent.
    pub fn signal_term(&self) -> VmResult<()> {
        send_signal(self.pid, libc::SIGTERM)
    }

    /// Send SIGKILL to the process.
    ///
    /// # Errors
    ///
    /// Returns `VmError::ProcessSignal` if the signal cannot be sent.
    pub fn signal_kill(&self) -> VmResult<()> {
        send_signal(self.pid, libc::SIGKILL)
    }

    /// Stop the process gracefully: SIGTERM, wait up to `timeout_secs`, then SIGKILL.
    ///
    /// # Errors
    ///
    /// Returns `VmError::Io` if waiting on the process fails.
    pub async fn stop(&mut self, timeout_secs: u64) -> VmResult<(i32, String)> {
        let _ = self.signal_term();

        if let Some(result) = self.wait_timeout(Duration::from_secs(timeout_secs)).await? {
            self.cleanup();
            Ok(result)
        } else {
            let _ = self.signal_kill();
            let result = self.wait().await?;
            self.cleanup();
            Ok(result)
        }
    }

    /// Force-kill the process immediately.
    ///
    /// # Errors
    ///
    /// Returns `VmError::Io` if waiting on the process fails.
    pub async fn force_kill(&mut self) -> VmResult<(i32, String)> {
        let _ = self.signal_kill();
        let result = self.wait().await?;
        self.cleanup();
        Ok(result)
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Clean up PID file and socket.
    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.pid_file);
        let _ = std::fs::remove_file(&self.socket_path);
    }

    async fn read_stderr(&mut self) -> String {
        let Some(stderr) = self.child.stderr.as_mut() else {
            return String::new();
        };
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf).await;
        buf
    }
}

/// Send a signal to a process by PID.
#[allow(clippy::cast_possible_wrap)]
fn send_signal(pid: u32, signal: i32) -> VmResult<()> {
    let ret = unsafe { libc::kill(pid as i32, signal) };
    if ret != 0 {
        let err = std::io::Error::last_os_error();
        // ESRCH means process doesn't exist — not an error for our purposes
        if err.raw_os_error() == Some(libc::ESRCH) {
            return Ok(());
        }
        return Err(VmError::ProcessSignal(err.to_string()));
    }
    Ok(())
}

/// Check if a process with the given PID exists.
#[allow(clippy::cast_possible_wrap)]
fn process_exists(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

/// Clean up an orphaned crosvm process from a previous run.
async fn cleanup_orphan(pid_file: &Path) {
    let Ok(content) = std::fs::read_to_string(pid_file) else {
        return;
    };
    let Ok(pid) = content.trim().parse::<u32>() else {
        let _ = std::fs::remove_file(pid_file);
        return;
    };

    if process_exists(pid) {
        log::warn!("found orphaned crosvm process (PID {pid}), terminating");
        let _ = send_signal(pid, libc::SIGTERM);
        tokio::time::sleep(Duration::from_millis(500)).await;
        if process_exists(pid) {
            let _ = send_signal(pid, libc::SIGKILL);
        }
    }

    let _ = std::fs::remove_file(pid_file);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_nonexistent_binary() {
        let dir = tempfile::tempdir().unwrap();
        let args = vec![
            std::ffi::OsString::from("/nonexistent/crosvm"),
            "run".into(),
        ];
        let result = VmProcess::spawn(
            &args,
            dir.path().join("vm.pid"),
            dir.path().join("control.sock"),
        )
        .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            VmError::CrosvmNotFound(_) => {}
            other => panic!("expected CrosvmNotFound, got: {other}"),
        }
    }

    #[tokio::test]
    async fn spawn_and_wait_for_exit() {
        let dir = tempfile::tempdir().unwrap();
        let args = vec![std::ffi::OsString::from("echo"), "hello".into()];
        let mut proc = VmProcess::spawn(
            &args,
            dir.path().join("vm.pid"),
            dir.path().join("control.sock"),
        )
        .await
        .unwrap();

        assert!(proc.pid() > 0);
        assert!(dir.path().join("vm.pid").exists());

        let (code, _) = proc.wait().await.unwrap();
        assert_eq!(code, 0);
    }

    #[tokio::test]
    async fn force_kill_running_process() {
        let dir = tempfile::tempdir().unwrap();
        let args = vec![std::ffi::OsString::from("sleep"), "60".into()];
        let mut proc = VmProcess::spawn(
            &args,
            dir.path().join("vm.pid"),
            dir.path().join("control.sock"),
        )
        .await
        .unwrap();

        assert!(proc.is_running());
        let (code, _) = proc.force_kill().await.unwrap();
        assert_ne!(code, 0);
        assert!(!dir.path().join("vm.pid").exists());
    }

    #[tokio::test]
    async fn stop_with_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let args = vec![std::ffi::OsString::from("sleep"), "60".into()];
        let mut proc = VmProcess::spawn(
            &args,
            dir.path().join("vm.pid"),
            dir.path().join("control.sock"),
        )
        .await
        .unwrap();

        let (code, _) = proc.stop(1).await.unwrap();
        assert_ne!(code, 0);
    }

    #[tokio::test]
    async fn cleanup_stale_pid_file() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("vm.pid");
        std::fs::write(&pid_file, "999999999").unwrap();

        cleanup_orphan(&pid_file).await;
        assert!(!pid_file.exists());
    }

    #[test]
    fn signal_nonexistent_process() {
        let result = send_signal(999_999_999, libc::SIGTERM);
        assert!(result.is_ok());
    }
}
