//! Shim for openai/codex's `codex-websocket-client` (943a21829d, Apache-2.0): the
//! `WebSocketConnector`, `WebSocketTlsMode`, and `WebSocketConnection` API the realtime client
//! uses, over upstream tokio-tungstenite with rustls. Codex resolves proxies, enforces its
//! network policy, and attaches ChatGPT cookies here; this shim always dials directly, and
//! `WebSocketTlsMode::TungsteniteDefault` verifies against the platform's trusted roots.

use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use codex_http_client::HttpClientFactory;
use futures::Sink;
use futures::Stream;
use rustls::ClientConfig;
use tokio::net::TcpStream;
use tokio_tungstenite::Connector;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream as TungsteniteStream;
use tokio_tungstenite::connect_async_tls_with_config;
use tokio_tungstenite::tungstenite::Error as WebSocketError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::handshake::client::Response;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

/// Connects WebSockets for one application configuration.
#[derive(Clone)]
pub struct WebSocketConnector {
    tls_config: Option<Arc<ClientConfig>>,
    tcp_nodelay: bool,
}

/// Selects the TLS configuration used for WebSocket connections.
#[derive(Clone)]
pub enum WebSocketTlsMode {
    /// Use an existing Rustls configuration.
    Rustls(Arc<ClientConfig>),
    /// Let Tungstenite build its default TLS configuration (platform roots) when the target
    /// requires TLS.
    TungsteniteDefault,
}

impl WebSocketConnector {
    /// Creates a connector with the selected WebSocket TLS configuration. The factory is accepted
    /// for API compatibility with Codex; its only policy, `ReqwestDefault`, dials directly.
    pub fn new_with_tls_mode(
        _http_client_factory: &HttpClientFactory,
        tls_mode: WebSocketTlsMode,
    ) -> Result<Self, Infallible> {
        let tls_config = match tls_mode {
            WebSocketTlsMode::Rustls(config) => Some(config),
            WebSocketTlsMode::TungsteniteDefault => None,
        };
        Ok(Self {
            tls_config,
            tcp_nodelay: false,
        })
    }

    /// Disables Nagle's algorithm for latency-sensitive WebSocket connections.
    pub fn with_tcp_nodelay(mut self) -> Self {
        self.tcp_nodelay = true;
        self
    }

    /// Connects a WebSocket directly to the request's destination.
    pub async fn connect(
        &self,
        request: Request,
        config: WebSocketConfig,
    ) -> Result<(WebSocketConnection, Response), WebSocketError> {
        let (inner, response) = connect_async_tls_with_config(
            request,
            Some(config),
            self.tcp_nodelay,
            self.tls_config.clone().map(Connector::Rustls),
        )
        .await?;
        Ok((WebSocketConnection { inner }, response))
    }
}

/// An established WebSocket independent of its transport layers.
pub struct WebSocketConnection {
    inner: TungsteniteStream<MaybeTlsStream<TcpStream>>,
}

impl std::fmt::Debug for WebSocketConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocketConnection")
            .finish_non_exhaustive()
    }
}

impl Stream for WebSocketConnection {
    type Item = Result<Message, WebSocketError>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().inner).poll_next(context)
    }
}

impl Sink<Message> for WebSocketConnection {
    type Error = WebSocketError;

    fn poll_ready(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_ready(context)
    }

    fn start_send(self: Pin<&mut Self>, message: Message) -> Result<(), Self::Error> {
        Pin::new(&mut self.get_mut().inner).start_send(message)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(context)
    }

    fn poll_close(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_close(context)
    }
}
