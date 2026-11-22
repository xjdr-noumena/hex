// Derived from openai/codex 943a21829d (codex-rs/codex-api/src/lib.rs), Apache-2.0. Modified for
// Hex: only the realtime layer (WebRTC call creation and the realtime/sideband WebSocket client)
// and the auth, error, provider, telemetry, and endpoint-session pieces it needs.

// Codex lints with `large-error-threshold = 256`; `ApiError` carries a `TransportError`.
#![allow(clippy::result_large_err)]

pub(crate) mod auth;
pub(crate) mod endpoint;
pub(crate) mod error;
pub(crate) mod provider;
pub(crate) mod telemetry;

pub use codex_client::RequestTelemetry;
pub use codex_client::ReqwestTransport;
pub use codex_client::TransportError;

pub use crate::auth::AgentIdentityTelemetry;
pub use crate::auth::AuthError;
pub use crate::auth::AuthHeaderTelemetry;
pub use crate::auth::AuthHeadersFuture;
pub use crate::auth::AuthProvider;
pub use crate::auth::AuthProviderFuture;
pub use crate::auth::AuthRefreshFuture;
pub use crate::auth::SharedAuthProvider;
pub use crate::auth::auth_header_telemetry;
pub use crate::endpoint::RealtimeCallClient;
pub use crate::endpoint::RealtimeCallResponse;
pub use crate::endpoint::RealtimeContextAppendChannel;
pub use crate::endpoint::RealtimeEventParser;
pub use crate::endpoint::RealtimeOutputModality;
pub use crate::endpoint::RealtimeSessionConfig;
pub use crate::endpoint::RealtimeSessionMode;
pub use crate::endpoint::RealtimeTranscriptState;
pub use crate::endpoint::RealtimeWebsocketClient;
pub use crate::endpoint::RealtimeWebsocketConnection;
pub use crate::endpoint::RealtimeWebsocketEvents;
pub use crate::endpoint::RealtimeWebsocketWriter;
pub use crate::endpoint::session_update_session_json;
pub use crate::error::ApiError;
pub use crate::provider::Provider;
pub use crate::provider::RetryConfig;
pub use codex_protocol::protocol::RealtimeAudioFrame;
pub use codex_protocol::protocol::RealtimeEvent;
