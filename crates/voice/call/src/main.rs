//! `hex-voice-call`: one GPT-Live voice call from the terminal, with delegations answered by an
//! OpenCode session. Needs `hex-voice-host` beside this binary (cargo builds both into the same
//! target directory) or at `HEX_VOICE_HOST`.
//!
//!   hex-voice-call --endpoint http://127.0.0.1:8080/v1 [--key KEY] [--session ses_...]

use std::io::Write;

use clap::Parser;
use codex_protocol::protocol::RealtimeVoice;
use hex_voice::CallConfig;
use hex_voice::CallEvent;

#[derive(Parser)]
#[command(about = "One GPT-Live voice call; delegations go to an OpenCode session")]
struct Args {
    /// GPT-Live endpoint base URL (OSS Live, or https://api.openai.com/v1).
    #[arg(
        long,
        env = "HEX_VOICE_ENDPOINT",
        default_value = "http://127.0.0.1:8080/v1"
    )]
    endpoint: String,
    /// Bearer key for the endpoint.
    #[arg(long, env = "HEX_VOICE_KEY", hide_env_values = true)]
    key: Option<String>,
    #[arg(long, default_value = "gpt-live-1-codex")]
    model: String,
    /// OpenCode session for delegations (default: newest root session with an existing directory).
    #[arg(long)]
    session: Option<String>,
    /// Session instructions for the voice model.
    #[arg(long)]
    instructions: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("HEX_VOICE_LOG")
                .unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let args = Args::parse();
    let config = CallConfig {
        endpoint: args.endpoint,
        key: args.key,
        model: args.model,
        voice: RealtimeVoice::Cove,
        instructions: args
            .instructions
            .unwrap_or_else(|| hex_voice::DEFAULT_INSTRUCTIONS.to_owned()),
        session: args.session,
    };
    eprintln!("connecting to {} … (Ctrl-C hangs up)", config.endpoint);
    hex_voice::run(config, print_event, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
}

fn print_event(event: CallEvent) {
    let mut out = std::io::stdout().lock();
    let _ = match event {
        CallEvent::Connected { call_id, setup } => writeln!(
            out,
            "● connected {call_id} in {} ms — talk now",
            setup.as_millis()
        ),
        CallEvent::UserSaid(t) => write!(out, "{t}"),
        CallEvent::AssistantSaid(t) => write!(out, "{t}"),
        CallEvent::TurnDone { role, text } => writeln!(out, "\n[{role}] {text}"),
        CallEvent::Delegated {
            id,
            request,
            session,
        } => writeln!(out, "\n→ OpenCode {session} ({id}): {request}"),
        CallEvent::Answered { id, elapsed, chars } => writeln!(
            out,
            "← {id}: {chars} chars in {:.1} s",
            elapsed.as_secs_f64()
        ),
        CallEvent::DelegationFailed { id, error } => writeln!(out, "✗ {id}: {error}"),
        CallEvent::Ended { reason } => writeln!(out, "\n■ call ended: {reason}"),
    };
    let _ = out.flush();
}
