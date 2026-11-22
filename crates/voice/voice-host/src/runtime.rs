//! Native runtime initialization.
//!
//! Derived from openai/codex 943a21829d (voice-host/src/runtime.rs), Apache-2.0. Modified for
//! Hex: Codex loads a privately bundled GStreamer runtime here. Hex decodes in-process
//! (playout.rs), so there is nothing to load; `initializeRuntime` still answers `runtimeReady`
//! so the control protocol and its ordering checks are unchanged.

use std::io;

pub(super) struct Runtime;

impl Runtime {
    pub(super) fn initialize() -> io::Result<Self> {
        Ok(Self)
    }
}
