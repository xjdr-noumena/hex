//! Shim for openai/codex's `codex-http-client` (943a21829d, Apache-2.0), reduced to what the
//! realtime API client uses: `Request`/`Response`, `TransportError`, `RetryAfter`, the
//! `HttpTransport` trait with a reqwest implementation, and an `HttpClientFactory` that only
//! carries reqwest's default proxy behaviour. Codex's network policy, system-proxy routing,
//! custom CA bundles, ChatGPT cookie store, and zstd request compression are not included.

// Codex lints with `large-error-threshold = 256`; `TransportError::Http` is about 170 bytes.
#![allow(clippy::result_large_err)]

mod error;
mod outbound_proxy;
mod request;
mod retry_after;
mod transport;

pub use crate::error::TransportError;
pub use crate::outbound_proxy::HttpClientFactory;
pub use crate::outbound_proxy::OutboundProxyPolicy;
pub use crate::request::EncodedJsonBody;
pub use crate::request::PreparedRequestBody;
pub use crate::request::Request;
pub use crate::request::RequestBody;
pub use crate::request::RequestCompression;
pub use crate::request::Response;
pub use crate::retry_after::RetryAfter;
pub use crate::transport::ByteStream;
pub use crate::transport::HttpTransport;
pub use crate::transport::ReqwestTransport;
pub use crate::transport::StreamResponse;
