// Derived from openai/codex 943a21829d (codex-rs/codex-api/src/error.rs), Apache-2.0. Modified for
// Hex: only the variants the realtime client produces (no rate-limit, quota, policy, or
// misalignment errors, and no provider error-body parsing).
use codex_client::TransportError;
use http::StatusCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("api error {status}: {message}")]
    Api { status: StatusCode, message: String },
    #[error("stream error: {0}")]
    Stream(String),
    #[error("invalid request: {message}")]
    InvalidRequest { message: String },
}

impl From<crate::AuthError> for ApiError {
    fn from(error: crate::AuthError) -> Self {
        Self::Transport(error.into())
    }
}
