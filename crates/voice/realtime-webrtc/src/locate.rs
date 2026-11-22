//! Hex addition (replaces Codex's package-layout lookup): the voice helper ships beside the Hex
//! executable as `hex-voice-host[.exe]`. `HEX_VOICE_HOST` overrides the path for development.

use std::path::PathBuf;

pub const HELPER_NAME: &str = if cfg!(windows) {
    "hex-voice-host.exe"
} else {
    "hex-voice-host"
};

/// The helper executable, when one exists.
pub fn helper_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("HEX_VOICE_HOST") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let path = exe.parent()?.join(HELPER_NAME);
    path.is_file().then_some(path)
}
