//! VM management for Nux Emulator.
//!
//! Provides crosvm process lifecycle management, KVM detection,
//! command building, and control socket communication.

pub mod command;
pub mod config;
pub mod control;
pub mod detect;
pub mod error;
pub mod process;
pub mod state;

use config::VmConfig;
use control::ControlClient;
use error::{VmError, VmResult};
use process::VmProcess;
use state::VmState;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};

/// Maps `VmState` to a u8 for atomic storage.
fn state_to_u8(s: VmState) -> u8 {
    match s {
        VmState::Idle => 0,
        VmState::Starting => 1,
        VmState::Running => 2,
        VmState::Paused => 3,
        VmState::Stopping => 4,
        VmState::Stopped => 5,
        VmState::Crashed => 6,
        VmState::Failed => 7,
    }
}

fn u8_to_state(v: u8) -> VmState {
    match v {
        0 => VmState::Idle,
        1 => VmState::Starting,
        2 => VmState::Running,
        3 => VmState::Paused,
        4 => VmState::Stopping,
        5 => VmState::Stopped,
        6 => VmState::Crashed,
        _ => VmState::Failed,
    }
}

/// Top-level VM manager that orchestrates all VM operations.
pub struct VmManager {
    config: VmConfig,
    state: AtomicU8,
    process: Option<VmProcess>,
    control: Option<ControlClient>,
}

impl VmManager {
    /// Create a new VM manager with the given configuration.
    pub fn new(config: VmConfig) -> Self {
        Self {
            config,
            state: AtomicU8::new(state_to_u8(VmState::Idle)),
            process: None,
            control: None,
        }
    }

    /// Get the current VM state.
    pub fn state(&self) -> VmState {
        u8_to_state(self.state.load(Ordering::Relaxed))
    }

    fn set_state(&self, new_state: VmState) {
        self.state.store(state_to_u8(new_state), Ordering::Relaxed);
    }

    fn transition(&self, target: VmState, operation: &str) -> VmResult<()> {
        let current = self.state();
        if !current.can_transition_to(target) {
            return Err(VmError::InvalidStateTransition {
                state: current.to_string(),
                operation: operation.to_owned(),
            });
        }
        self.set_state(target);
        Ok(())
    }

    /// Start the VM.
    ///
    /// Validates config, checks KVM readiness, builds the crosvm command,
    /// spawns the process, and connects the control socket.
    ///
    /// # Errors
    ///
    /// Returns errors for invalid config, missing KVM, or spawn failures.
    pub async fn start(&mut self) -> VmResult<()> {
        self.transition(VmState::Starting, "start")?;

        // Validate config
        if let Err(e) = self.config.validate() {
            self.set_state(VmState::Failed);
            return Err(e);
        }

        // Check KVM readiness
        let report = detect::check_kvm_readiness();
        if !report.ready {
            self.set_state(VmState::Failed);
            let failures: Vec<String> = report
                .checks
                .iter()
                .filter(|c| !c.passed)
                .map(|c| c.detail.clone())
                .collect();
            return Err(VmError::KvmNotAvailable(failures.join("; ")));
        }

        // Build command
        let args = command::build_command(&self.config);
        let socket_path = self.config.effective_socket_path();
        let pid_file = pid_file_path();

        // Spawn process
        match VmProcess::spawn(&args, pid_file, socket_path.clone()).await {
            Ok(proc) => {
                self.process = Some(proc);
            }
            Err(e) => {
                self.set_state(VmState::Failed);
                return Err(e);
            }
        }

        // Connect control socket (with retries — crosvm needs time to create it)
        let mut client = ControlClient::new(socket_path, self.config.ram_mb);
        let mut connected = false;
        for _ in 0..10 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Check if process is still running
            if let Some(proc) = &mut self.process {
                if !proc.is_running() {
                    let (code, stderr) = proc.wait().await?;
                    self.set_state(VmState::Failed);
                    return Err(VmError::CrosvmStartFailed {
                        exit_code: code,
                        stderr,
                    });
                }
            }

            if client.connect().await.is_ok() {
                connected = true;
                break;
            }
        }

        if !connected {
            // Process is running but socket isn't available — still mark as running
            // since some crosvm configurations don't create a socket immediately
            log::warn!("control socket not available, VM may not support runtime control");
        }

        self.control = Some(client);
        self.set_state(VmState::Running);
        Ok(())
    }

    /// Stop the VM gracefully.
    ///
    /// # Errors
    ///
    /// Returns an error if the VM is not in a stoppable state.
    pub async fn stop(&mut self) -> VmResult<()> {
        self.transition(VmState::Stopping, "stop")?;

        // Try control socket stop first
        if let Some(ctrl) = &mut self.control {
            let _ = ctrl.stop().await;
        }

        // Wait for process to exit
        if let Some(proc) = &mut self.process {
            let _ = proc.stop(10).await;
        }

        self.control = None;
        self.process = None;
        self.set_state(VmState::Stopped);
        Ok(())
    }

    /// Pause the VM.
    ///
    /// # Errors
    ///
    /// Returns an error if the VM is not running or the control socket fails.
    pub async fn pause(&mut self) -> VmResult<()> {
        self.transition(VmState::Paused, "pause")?;

        if let Some(ctrl) = &mut self.control {
            if let Err(e) = ctrl.pause().await {
                self.set_state(VmState::Running); // Revert
                return Err(e);
            }
        }

        Ok(())
    }

    /// Resume a paused VM.
    ///
    /// # Errors
    ///
    /// Returns an error if the VM is not paused or the control socket fails.
    pub async fn resume(&mut self) -> VmResult<()> {
        self.transition(VmState::Running, "resume")?;

        if let Some(ctrl) = &mut self.control {
            if let Err(e) = ctrl.resume().await {
                self.set_state(VmState::Paused); // Revert
                return Err(e);
            }
        }

        Ok(())
    }

    /// Force-kill the VM immediately.
    ///
    /// # Errors
    ///
    /// Returns an error if the process signal fails.
    pub async fn force_kill(&mut self) -> VmResult<()> {
        let current = self.state();
        match current {
            VmState::Running | VmState::Paused | VmState::Starting | VmState::Stopping => {}
            VmState::Idle | VmState::Stopped | VmState::Crashed | VmState::Failed => {
                // Already stopped — clean up any stale resources
                if let Some(proc) = &self.process {
                    proc.cleanup();
                }
                self.process = None;
                self.control = None;
                return Ok(());
            }
        }

        if let Some(proc) = &mut self.process {
            let _ = proc.force_kill().await;
        }

        self.control = None;
        self.process = None;
        self.set_state(VmState::Stopped);
        Ok(())
    }
}

/// Get the default PID file path.
fn pid_file_path() -> PathBuf {
    let uid = nix::unistd::getuid();
    PathBuf::from(format!("/run/user/{uid}/nux/vm.pid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> VmConfig {
        VmConfig::default()
    }

    #[test]
    fn initial_state_is_idle() {
        let mgr = VmManager::new(test_config());
        assert_eq!(mgr.state(), VmState::Idle);
    }

    #[test]
    fn invalid_transition_rejected() {
        let mgr = VmManager::new(test_config());
        let result = mgr.transition(VmState::Running, "start");
        assert!(result.is_err());
    }

    #[test]
    fn valid_transition_accepted() {
        let mgr = VmManager::new(test_config());
        let result = mgr.transition(VmState::Starting, "start");
        assert!(result.is_ok());
        assert_eq!(mgr.state(), VmState::Starting);
    }

    #[tokio::test]
    async fn force_kill_when_idle_is_noop() {
        let mut mgr = VmManager::new(test_config());
        let result = mgr.force_kill().await;
        assert!(result.is_ok());
        assert_eq!(mgr.state(), VmState::Idle);
    }
}
