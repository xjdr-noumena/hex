// Derived from openai/codex 943a21829d (codex-rs/codex-client/src/lib.rs), Apache-2.0.
// Modified for Hex: no `sse` module, and no `HttpClient`/`RequestBuilder` aliases (the
// codex-http-client shim has neither).

// Codex lints with `large-error-threshold = 256`; `TransportError::Http` is about 170 bytes.
#![allow(clippy::result_large_err)]

mod provider;
mod retry;
mod telemetry;

pub use crate::provider::Provider;
pub use crate::provider::RetryConfig;
pub use crate::retry::RetryOn;
pub use crate::retry::RetryOperation;
pub use crate::retry::RetryPolicy;
pub use crate::retry::backoff;
pub use crate::retry::run_with_retry;
pub use crate::telemetry::RequestTelemetry;
pub use codex_http_client::*;
