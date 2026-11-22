//! Shim for openai/codex's `codex-protocol` (943a21829d, Apache-2.0), reduced to the realtime
//! types `codex-realtime-api` uses. The definitions are copied from codex-rs/protocol/src
//! (`protocol.rs` and `realtime.rs`) without their `JsonSchema` and `TS` derives, so the wire
//! format is unchanged.

pub mod protocol;
pub mod realtime;
