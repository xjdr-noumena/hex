// Derived from openai/codex 943a21829d (codex-rs/http-client/src/outbound_proxy.rs), Apache-2.0.
// Modified for Hex: only the `ReqwestDefault` policy; the factory carries no network policy,
// cookie store, or system-proxy resolution, and builds plain reqwest clients.
use crate::error::TransportError;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;

/// Selects how clients built by an [`HttpClientFactory`] route outbound traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboundProxyPolicy {
    /// Preserve reqwest's built-in proxy behavior (`HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY`).
    /// WebSocket connections are made directly.
    ReqwestDefault,
}

/// Builds outbound HTTP clients and WebSocket connectors from one application policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpClientFactory {
    outbound_proxy_policy: OutboundProxyPolicy,
}

impl HttpClientFactory {
    /// Creates a factory from the outbound proxy policy resolved by the application.
    pub const fn new(outbound_proxy_policy: OutboundProxyPolicy) -> Self {
        Self {
            outbound_proxy_policy,
        }
    }

    /// Returns the outbound proxy policy used for clients built by this factory.
    pub const fn outbound_proxy_policy(&self) -> OutboundProxyPolicy {
        self.outbound_proxy_policy
    }

    /// Builds a reqwest client that verifies TLS with rustls (aws-lc-rs) against the platform's
    /// trusted roots, for use with [`crate::ReqwestTransport`].
    pub fn build_reqwest_client(&self) -> Result<reqwest::Client, TransportError> {
        ensure_rustls_crypto_provider();
        match self.outbound_proxy_policy {
            OutboundProxyPolicy::ReqwestDefault => reqwest::Client::builder()
                .build()
                .map_err(|err| TransportError::Build(err.to_string())),
        }
    }
}
