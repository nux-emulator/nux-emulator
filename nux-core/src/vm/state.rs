//! VM state machine.

use std::fmt;

/// Possible states of the VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmState {
    /// No VM process exists.
    Idle,
    /// VM is being started.
    Starting,
    /// VM is running normally.
    Running,
    /// VM is paused (vCPUs halted).
    Paused,
    /// VM is being stopped gracefully.
    Stopping,
    /// VM has been stopped cleanly.
    Stopped,
    /// VM process exited unexpectedly.
    Crashed,
    /// VM failed to start.
    Failed,
}

impl fmt::Display for VmState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Paused => write!(f, "paused"),
            Self::Stopping => write!(f, "stopping"),
            Self::Stopped => write!(f, "stopped"),
            Self::Crashed => write!(f, "crashed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl VmState {
    /// Check whether a transition from `self` to `target` is valid.
    pub fn can_transition_to(self, target: Self) -> bool {
        matches!(
            (self, target),
            (Self::Idle, Self::Starting)
                | (Self::Starting | Self::Paused, Self::Running)
                | (Self::Starting, Self::Failed)
                | (Self::Running, Self::Paused | Self::Stopping | Self::Crashed)
                | (Self::Paused, Self::Stopping | Self::Crashed)
                | (Self::Stopping, Self::Stopped | Self::Crashed)
                | (Self::Stopped | Self::Crashed | Self::Failed, Self::Idle)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions() {
        assert!(VmState::Idle.can_transition_to(VmState::Starting));
        assert!(VmState::Starting.can_transition_to(VmState::Running));
        assert!(VmState::Starting.can_transition_to(VmState::Failed));
        assert!(VmState::Running.can_transition_to(VmState::Paused));
        assert!(VmState::Running.can_transition_to(VmState::Stopping));
        assert!(VmState::Running.can_transition_to(VmState::Crashed));
        assert!(VmState::Paused.can_transition_to(VmState::Running));
        assert!(VmState::Paused.can_transition_to(VmState::Stopping));
        assert!(VmState::Stopping.can_transition_to(VmState::Stopped));
        assert!(VmState::Stopping.can_transition_to(VmState::Crashed));
        assert!(VmState::Stopped.can_transition_to(VmState::Idle));
        assert!(VmState::Crashed.can_transition_to(VmState::Idle));
        assert!(VmState::Failed.can_transition_to(VmState::Idle));
    }

    #[test]
    fn invalid_transitions() {
        assert!(!VmState::Idle.can_transition_to(VmState::Running));
        assert!(!VmState::Idle.can_transition_to(VmState::Paused));
        assert!(!VmState::Running.can_transition_to(VmState::Idle));
        assert!(!VmState::Running.can_transition_to(VmState::Starting));
        assert!(!VmState::Paused.can_transition_to(VmState::Idle));
        assert!(!VmState::Stopped.can_transition_to(VmState::Running));
        assert!(!VmState::Crashed.can_transition_to(VmState::Running));
        assert!(!VmState::Failed.can_transition_to(VmState::Running));
    }

    #[test]
    fn display_format() {
        assert_eq!(VmState::Idle.to_string(), "idle");
        assert_eq!(VmState::Running.to_string(), "running");
        assert_eq!(VmState::Crashed.to_string(), "crashed");
    }
}
