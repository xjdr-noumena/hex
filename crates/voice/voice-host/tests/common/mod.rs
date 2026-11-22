//! Launches the helper.
//!
//! Derived from openai/codex 943a21829d (voice-host/tests/common/mod.rs), Apache-2.0. Modified
//! for Hex: Cargo's built binary directly; there is no Bazel runtime to put on PATH.

use anyhow::Result;
use tokio::process::Command;

pub(crate) fn voice_host_command() -> Result<Command> {
    Ok(Command::new(env!("CARGO_BIN_EXE_hex-voice-host")))
}
