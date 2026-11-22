# Place a WebRTC voice call from the terminal

`hex-voice-call` connects to a compatible OSS Live endpoint and sends delegated
requests to an OpenCode session. Its sibling executable, `hex-voice-host`, owns
the microphone, speaker and WebRTC connection. Both binaries are built from source.

```sh
cargo build --release --locked -p hex-voice -p codex-voice-host
target/release/hex-voice-call --help
```

On Linux, building requires CMake, a C compiler, pkg-config and ALSA development
headers. The helper encodes and decodes Opus in-process.

Start OpenCode with a scratch session whose working directory exists. Set
`HEX_VOICE_ENDPOINT` to your server's API base URL (including `/v1`) and
`HEX_VOICE_KEY` to its bearer key if authentication is required. Run this in a
terminal with headphones, replacing the example ID with your scratch session ID.

```sh
target/release/hex-voice-call --session ses_YOUR_SCRATCH_SESSION
```

After the connected message, speak and pause. The client sends microphone audio
to the endpoint over WebRTC. A delegation sends a real prompt to the selected
OpenCode session and returns its reply to the voice endpoint. Omitting
`--session` selects the newest open root session whose directory is available.
Press Ctrl-C to hang up.

The Linux smoke test used an OSS Live tone worker. It exercised media setup,
turn detection, an OpenCode request and reply, and hang-up. The tone worker
emits beeps and a placeholder request. Speech recognition and synthesized
speech require a suitable server backend.

The [call crate](call/src/lib.rs) coordinates signaling and delegation. The
[media helper](voice-host/src/transport.rs) negotiates WebRTC, including ICE-TCP
for loopback forwarding. The port is based on [OpenAI Codex](https://github.com/openai/codex/tree/943a21829d0bc3462a725f9fb3cb704c21b64b91).
