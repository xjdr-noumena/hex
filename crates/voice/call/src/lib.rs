//! One GPT-Live voice call, in the order the Codex TUI runs it (codex-rs/tui/src/chatwidget/
//! realtime.rs at openai/codex 943a21829d): the media helper gathers an offer, the call is created
//! with the v3 session (`POST {endpoint}/live`), the answer is applied while the sideband joins
//! `/v1/live/{call_id}`, then the microphone opens. Each `delegation.created` is sent to an
//! OpenCode session, and its reply goes back as `delegation.context.append` for the voice model
//! to speak.

pub mod opencode;

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::RealtimeVoice;
use codex_realtime_api::AuthError;
use codex_realtime_api::AuthProvider;
use codex_realtime_api::Provider;
use codex_realtime_api::RealtimeCallClient;
use codex_realtime_api::RealtimeContextAppendChannel;
use codex_realtime_api::RealtimeEventParser;
use codex_realtime_api::RealtimeSessionConfig;
use codex_realtime_api::RealtimeSessionMode;
use codex_realtime_api::RealtimeTranscriptState;
use codex_realtime_api::RealtimeWebsocketClient;
use codex_realtime_api::ReqwestTransport;
use codex_realtime_api::RetryConfig;
use codex_realtime_webrtc::AudioDeviceSelection;
use codex_realtime_webrtc::RealtimeWebrtcSession;
use futures::future::AbortHandle;
use http::HeaderMap;
use http::HeaderValue;

pub const DEFAULT_INSTRUCTIONS: &str = "You are Hex, the user's voice assistant for their OpenCode coding sessions. \
Talk naturally and briefly. For anything about their code, files, sessions, tasks or anything you would have to look \
up or do, hand the request to OpenCode in the user's own words and say only a short acknowledgement. When OpenCode's \
answer comes back, tell the user its substance in a sentence or two. Never claim something was done before OpenCode \
confirms it. Never read code, file paths or IDs aloud.";

#[derive(Clone, Debug)]
pub struct CallConfig {
    /// Base URL of the GPT-Live endpoint, e.g. `http://127.0.0.1:8080/v1`.
    pub endpoint: String,
    pub key: Option<String>,
    pub model: String,
    pub voice: RealtimeVoice,
    pub instructions: String,
    /// OpenCode session that answers delegations; the most recently updated one when unset.
    pub session: Option<String>,
}

/// What the call reports, for a console or an overlay.
#[derive(Clone, Debug)]
pub enum CallEvent {
    Connected {
        call_id: String,
        setup: Duration,
    },
    UserSaid(String),
    AssistantSaid(String),
    TurnDone {
        role: &'static str,
        text: String,
    },
    Delegated {
        id: String,
        request: String,
        session: String,
    },
    Answered {
        id: String,
        elapsed: Duration,
        chars: usize,
    },
    DelegationFailed {
        id: String,
        error: String,
    },
    Ended {
        reason: String,
    },
}

struct Bearer(Option<String>);

impl AuthProvider for Bearer {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), AuthError> {
        if let Some(key) = &self.0
            && let Ok(value) = HeaderValue::from_str(&format!("Bearer {key}"))
        {
            headers.insert(http::header::AUTHORIZATION, value);
        }
        Ok(())
    }
}

fn provider(base_url: &str) -> Provider {
    Provider {
        name: "oss-live".into(),
        base_url: base_url.trim_end_matches('/').to_owned(),
        query_params: None,
        headers: HeaderMap::new(),
        retry: RetryConfig {
            max_attempts: 4,
            base_delay: Duration::from_millis(200),
            retry_429: false,
            retry_5xx: true,
            retry_transport: true,
        },
        stream_idle_timeout: Duration::from_secs(300),
    }
}

fn session_config(config: &CallConfig) -> RealtimeSessionConfig {
    RealtimeSessionConfig {
        instructions: config.instructions.clone(),
        initial_items: Vec::new(),
        delegation_ack_filler: None,
        model: Some(config.model.clone()),
        session_id: None,
        event_parser: RealtimeEventParser::FramelessBidi,
        session_mode: RealtimeSessionMode::Conversational,
        output_modality: RealtimeOutputModality::Audio,
        voice: config.voice,
    }
}

/// Runs one call until `stop` resolves or the server ends it.
pub async fn run(
    config: CallConfig,
    on_event: impl Fn(CallEvent) + Send + Sync + 'static,
    stop: impl std::future::Future<Output = ()>,
) -> Result<()> {
    let on_event = Arc::new(on_event);
    let started = Instant::now();
    let requested_session = config.session.clone();
    let session = tokio::task::spawn_blocking(move || {
        opencode::resolve_session(requested_session.as_deref())
    })
    .await??;

    // 1. The helper gathers an offer (it owns WebRTC, the devices and echo cancellation).
    let (_abort, registration) = AbortHandle::new_pair();
    let media = tokio::task::spawn_blocking(move || {
        RealtimeWebrtcSession::start(registration, AudioDeviceSelection::default())
    })
    .await?
    .context("voice helper (hex-voice-host) failed to start")?;
    let handle = media.handle;

    // 2. Create the call with the v3 session.
    let auth: codex_realtime_api::SharedAuthProvider = Arc::new(Bearer(config.key.clone()));
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    let transport = ReqwestTransport::new(factory.build_reqwest_client()?);
    let mut headers = HeaderMap::new();
    headers.insert("openai-alpha", HeaderValue::from_static("quicksilver=v2"));
    let calls = RealtimeCallClient::new(transport, provider(&config.endpoint), auth.clone());
    let answer = calls
        .create_with_session_and_headers(media.offer_sdp, session_config(&config), headers.clone())
        .await
        .context("call creation failed")?;

    // 3. Apply the answer and join the sideband concurrently, as Codex does.
    let answer_handle = handle.clone();
    let sdp = answer.sdp.clone();
    let applying = tokio::task::spawn_blocking(move || answer_handle.apply_answer_sdp(sdp));
    let mut sideband_headers = headers.clone();
    auth.add_auth_headers(&mut sideband_headers)?;
    let ws = RealtimeWebsocketClient::new(provider(&config.endpoint), factory)
        .with_webrtc_sideband_base_url(config.endpoint.clone());
    let conn = ws
        .connect_webrtc_sideband(
            session_config(&config),
            &answer.call_id,
            sideband_headers,
            HeaderMap::new(),
            RealtimeTranscriptState::default(),
        )
        .await
        .context("sideband connection failed")?;
    applying
        .await?
        .map_err(|e| anyhow::anyhow!("media connection failed: {e}"))?;
    handle.set_microphone_muted(false)?;
    handle.set_speaker_suppressed(false);
    on_event(CallEvent::Connected {
        call_id: answer.call_id.clone(),
        setup: started.elapsed(),
    });

    // 4. Events until the user stops or the server ends the call.
    let writer = conn
        .writer()
        .with_context_append_channel(RealtimeContextAppendChannel::Speakable);
    let events = async {
        loop {
            let event = match conn.next_event().await {
                Ok(Some(event)) => event,
                Ok(None) => return "the server ended the call".to_owned(),
                Err(e) => return format!("sideband error: {e}"),
            };
            match event {
                RealtimeEvent::InputTranscriptDelta(d) => on_event(CallEvent::UserSaid(d.delta)),
                RealtimeEvent::OutputTranscriptDelta(d) => {
                    on_event(CallEvent::AssistantSaid(d.delta))
                }
                RealtimeEvent::InputTranscriptDone(d) => on_event(CallEvent::TurnDone {
                    role: "user",
                    text: d.text,
                }),
                RealtimeEvent::OutputTranscriptDone(d) => on_event(CallEvent::TurnDone {
                    role: "assistant",
                    text: d.text,
                }),
                RealtimeEvent::HandoffRequested(h) => {
                    let request = if h.input_transcript.trim().is_empty() {
                        h.active_transcript
                            .iter()
                            .map(|e| format!("{}: {}", e.role, e.text))
                            .collect::<Vec<_>>()
                            .join("\n")
                    } else {
                        h.input_transcript.clone()
                    };
                    on_event(CallEvent::Delegated {
                        id: h.handoff_id.clone(),
                        request: request.clone(),
                        session: session.clone(),
                    });
                    let (writer, on_event, session) =
                        (writer.clone(), on_event.clone(), session.clone());
                    tokio::spawn(async move {
                        let t0 = Instant::now();
                        let asked =
                            tokio::task::spawn_blocking(move || opencode::ask(&session, &request))
                                .await;
                        let text = match asked {
                            Ok(Ok(text)) => {
                                on_event(CallEvent::Answered {
                                    id: h.handoff_id.clone(),
                                    elapsed: t0.elapsed(),
                                    chars: text.chars().count(),
                                });
                                text
                            }
                            Ok(Err(e)) => {
                                on_event(CallEvent::DelegationFailed {
                                    id: h.handoff_id.clone(),
                                    error: format!("{e:#}"),
                                });
                                format!("I couldn't get that done: {e}")
                            }
                            Err(e) => format!("I couldn't get that done: {e}"),
                        };
                        let _ = writer
                            .send_conversation_handoff_append(h.handoff_id, text)
                            .await;
                    });
                }
                RealtimeEvent::Error(message) => return format!("server error: {message}"),
                _ => {}
            }
            if let Some(err) = handle.take_error() {
                return format!("media: {}", err.1);
            }
        }
    };
    let reason = tokio::select! {
        reason = events => reason,
        () = stop => "hung up".to_owned(),
    };
    let _ = conn.writer().close_transport().await; // Close 1000: the server ends the call
    handle.close();
    on_event(CallEvent::Ended { reason });
    Ok(())
}
