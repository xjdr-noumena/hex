//! Shim for openai/codex's `codex-build-info` (Apache-2.0): the build commit the voice client
//! sends in `hello`. The helper compares it with its own `STABLE_GIT_COMMIT` (default "dev"),
//! so both sides read the same variable at compile time.

use std::sync::OnceLock;

static OVERRIDE: OnceLock<&'static str> = OnceLock::new();

pub const BUILD_COMMIT: &str = match option_env!("STABLE_GIT_COMMIT") {
    Some(commit) => commit,
    None => "dev",
};

#[derive(Clone, Copy, Debug)]
pub struct BuildInfo {
    build_commit: &'static str,
}

impl BuildInfo {
    pub fn get() -> Self {
        Self {
            build_commit: OVERRIDE.get().copied().unwrap_or(BUILD_COMMIT),
        }
    }

    /// Tests pin the commit their fake helper reports.
    pub fn initialize(build_commit: &'static str) {
        let _ = OVERRIDE.set(build_commit);
    }

    pub fn build_commit(&self) -> &str {
        self.build_commit
    }
}
