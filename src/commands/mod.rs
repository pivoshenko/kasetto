//! Package that contains one module per `kasetto` subcommand.

use std::process::ExitCode;

/// How a command that ran to completion should be scored by the shell.
///
/// Distinct from `Err`: an error means the command could not do its job and
/// prints one `error:` line. `Outcome::Failure` means it did its job, reported
/// the problems it found in its own output, and should still exit non-zero so
/// CI can gate on it — a broken asset in `sync`, a failing check in `doctor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Success,
    Failure,
}

impl Outcome {
    pub(crate) fn code(self) -> ExitCode {
        match self {
            Outcome::Success => ExitCode::SUCCESS,
            Outcome::Failure => ExitCode::FAILURE,
        }
    }

    /// `Failure` when `failed` is true. Reads better than a bool at the two
    /// call sites that decide this from a count.
    pub(crate) fn from_failed(failed: bool) -> Self {
        if failed {
            Outcome::Failure
        } else {
            Outcome::Success
        }
    }
}

#[cfg(test)]
mod outcome_tests {
    use super::Outcome;

    #[test]
    fn from_failed_maps_the_verdict() {
        assert_eq!(Outcome::from_failed(false), Outcome::Success);
        assert_eq!(Outcome::from_failed(true), Outcome::Failure);
    }
}

pub(crate) mod add;
pub(crate) mod clean;
pub(crate) mod completions;
pub(crate) mod doctor;
pub(crate) mod init;
pub(crate) mod list;
pub(crate) mod lock;
pub(crate) mod remove;
pub(crate) mod self_update;
mod source_edit;
pub(crate) mod sync;
pub(crate) mod uninstall;
