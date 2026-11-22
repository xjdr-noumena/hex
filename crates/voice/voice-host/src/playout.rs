//! One jitter/decode pipeline per audible epoch, run on its own thread.
//!
//! Derived from openai/codex 943a21829d (voice-host/src/playout.rs), Apache-2.0. Modified for
//! Hex: Codex's GStreamer pipeline (rtpjitterbuffer latency 60 ms, rtpopusdepay, opusdec with
//! PLC, audioconvert/audioresample, custom sink) is replaced by the equivalent in-process steps,
//! so the helper needs no bundled native runtime:
//! - a 60 ms reordering buffer keyed by RTP sequence number; late packets are dropped;
//! - libopus decoding at 48 kHz mono, with in-band FEC from the next packet or PLC for a gap;
//! - rubato resampling to the speaker rate, as the capture path does;
//! - the existing bounded `PlaybackWriter`, which paces against the device queue.

use super::buffers::BLOCK;
use super::playback::PlaybackWriter;
use crate::incoming::ReceivedRtp;
use rubato::Resampler;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;

const LATENCY: Duration = Duration::from_millis(/*millis*/ 60);
const FRAME: Duration = Duration::from_millis(/*millis*/ 20);
const FRAME_SAMPLES: usize = 960; // 20 ms at 48 kHz
const MAX_FRAME_SAMPLES: usize = 5760; // 120 ms, the largest Opus frame
const MAX_HELD: usize = 16; // packets held before the oldest gap is skipped
const IDLE: Duration = Duration::from_millis(/*millis*/ 200);
const QUEUE: usize = 256;

pub(super) struct Playout {
    sender: Option<mpsc::SyncSender<Packet>>,
    failed: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

struct Packet {
    seq: u16,
    payload: Vec<u8>,
    at: Instant,
}

impl Playout {
    pub(super) fn new(writer: PlaybackWriter) -> Result<Self, &'static str> {
        let decoder = opus::Decoder::new(/*sample_rate*/ 48_000, opus::Channels::Mono)
            .map_err(|_| "audio decoder unavailable")?;
        let output = Output::new(writer)?;
        let (sender, receiver) = mpsc::sync_channel(QUEUE);
        let failed = Arc::new(AtomicBool::new(false));
        let failure = failed.clone();
        let thread = std::thread::Builder::new()
            .name("voice-playout".into())
            .spawn(move || {
                if Pipeline::new(decoder, output).run(&receiver).is_err() {
                    failure.store(true, Ordering::Release);
                }
            })
            .map_err(|_| "audio playout thread unavailable")?;
        Ok(Self {
            sender: Some(sender),
            failed,
            thread: Some(thread),
        })
    }

    pub(super) fn push(&self, packet: ReceivedRtp) -> Result<(), &'static str> {
        if let Some((seq, payload)) = parse_rtp(packet.as_ref()) {
            let packet = Packet {
                seq,
                payload: payload.to_vec(),
                at: packet.at,
            };
            if let Some(sender) = &self.sender {
                match sender.try_send(packet) {
                    // A saturated decoder sheds the newest burst rather than blocking ingress.
                    Ok(()) | Err(mpsc::TrySendError::Full(_)) => {}
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        return Err("audio playback failed");
                    }
                }
            }
        }
        self.check()
    }

    pub(super) fn check(&self) -> Result<(), &'static str> {
        if self.failed.load(Ordering::Acquire) {
            return Err("audio playback failed");
        }
        Ok(())
    }
}

impl Drop for Playout {
    fn drop(&mut self) {
        drop(self.sender.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The RTP payload of one packet (RFC 3550 header, CSRCs, extension and padding removed).
fn parse_rtp(data: &[u8]) -> Option<(u16, &[u8])> {
    if data.len() < 12 || data[0] >> 6 != 2 {
        return None;
    }
    let csrc = usize::from(data[0] & 0x0f);
    let seq = u16::from_be_bytes([data[2], data[3]]);
    let mut start = 12 + 4 * csrc;
    if data[0] & 0x10 != 0 {
        let header = data.get(start..start + 4)?;
        start += 4 + 4 * usize::from(u16::from_be_bytes([header[2], header[3]]));
    }
    let mut end = data.len();
    if data[0] & 0x20 != 0 {
        end = end.checked_sub(usize::from(*data.last()?))?;
    }
    (start < end).then(|| (seq, &data[start..end]))
}

struct Pipeline {
    decoder: opus::Decoder,
    output: Output,
    held: BTreeMap<u64, (Vec<u8>, Instant)>,
    newest: Option<u64>, // highest extended sequence number seen
    next: Option<u64>,   // next sequence number to play, while playing
    last_audio: Instant,
    pcm: Vec<f32>,
}

impl Pipeline {
    fn new(decoder: opus::Decoder, output: Output) -> Self {
        Self {
            decoder,
            output,
            held: BTreeMap::new(),
            newest: None,
            next: None,
            last_audio: Instant::now(),
            pcm: vec![0.0; MAX_FRAME_SAMPLES],
        }
    }

    fn run(mut self, receiver: &mpsc::Receiver<Packet>) -> Result<(), &'static str> {
        let mut tick = Instant::now() + FRAME;
        loop {
            match receiver.recv_timeout(tick.saturating_duration_since(Instant::now())) {
                Ok(packet) => {
                    self.hold(packet);
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            }
            let now = Instant::now();
            tick = (tick + FRAME).max(now);
            self.play(now)?;
        }
    }

    fn extend(&self, seq: u16) -> u64 {
        let Some(newest) = self.newest else {
            return u64::from(seq) + (1 << 16);
        };
        // Pick the candidate nearest the newest sequence number seen.
        let base = newest & !0xffff;
        [base.wrapping_sub(1 << 16), base, base + (1 << 16)]
            .into_iter()
            .map(|b| b | u64::from(seq))
            .min_by_key(|candidate| candidate.abs_diff(newest))
            .unwrap_or(newest)
    }

    fn hold(&mut self, packet: Packet) {
        let seq = self.extend(packet.seq);
        if self.next.is_some_and(|next| seq < next) {
            return; // arrived after its playout point
        }
        self.newest = Some(self.newest.map_or(seq, |n| n.max(seq)));
        self.held.insert(seq, (packet.payload, packet.at));
        while self.held.len() > MAX_HELD {
            // Too much queued: skip to the oldest held packet so delay stays bounded.
            if let Some((&oldest, _)) = self.held.iter().next() {
                if self.next.is_some_and(|next| next < oldest) {
                    self.next = Some(oldest);
                } else {
                    self.held.remove(&oldest);
                }
            }
        }
    }

    fn play(&mut self, now: Instant) -> Result<(), &'static str> {
        let next = match self.next {
            Some(next) => next,
            None => {
                // Start once the earliest held packet has waited out the reordering deadline.
                let Some((&first, (_, at))) = self.held.iter().next() else {
                    return Ok(());
                };
                if now.saturating_duration_since(*at) < LATENCY {
                    return Ok(());
                }
                self.next = Some(first);
                first
            }
        };
        let samples = if let Some((payload, _)) = self.held.remove(&next) {
            self.decoder
                .decode_float(&payload, &mut self.pcm, /*fec*/ false)
                .unwrap_or(0)
        } else if self.held.is_empty() {
            if now.saturating_duration_since(self.last_audio) >= IDLE {
                self.next = None; // the speaker stopped: wait for the next burst
            }
            return Ok(());
        } else if let Some((payload, _)) = self.held.get(&(next + 1)) {
            let pcm = &mut self.pcm[..FRAME_SAMPLES];
            self.decoder
                .decode_float(payload, pcm, /*fec*/ true)
                .unwrap_or(0)
        } else {
            let pcm = &mut self.pcm[..FRAME_SAMPLES];
            self.decoder
                .decode_float(&[], pcm, /*fec*/ false)
                .unwrap_or(0)
        };
        self.next = Some(next + 1);
        if samples > 0 {
            self.last_audio = now;
            self.output.write(&self.pcm[..samples])?;
        }
        Ok(())
    }
}

/// 48 kHz mono to the speaker's rate, written in bounded blocks.
struct Output {
    writer: PlaybackWriter,
    resampler: Option<rubato::Async<f32>>,
    input: Vec<f32>,
    scratch: Vec<f32>,
    cancelled: bool,
}

impl Output {
    fn new(writer: PlaybackWriter) -> Result<Self, &'static str> {
        let resampler = if writer.rate() == 48_000 {
            None
        } else {
            Some(
                rubato::Async::new_sinc(
                    f64::from(writer.rate()) / 48_000.0,
                    /*max_resample_ratio_relative*/ 1.0,
                    &rubato::SincInterpolationParameters::default(),
                    FRAME_SAMPLES,
                    /*nbr_channels*/ 1,
                    rubato::FixedAsync::Input,
                )
                .map_err(|_| "audio resampler unavailable")?,
            )
        };
        let scratch = vec![0.0; resampler.as_ref().map_or(0, |r| r.output_frames_max())];
        Ok(Self {
            writer,
            resampler,
            input: Vec::new(),
            scratch,
            cancelled: false,
        })
    }

    fn write(&mut self, pcm: &[f32]) -> Result<(), &'static str> {
        if self.cancelled {
            return Ok(());
        }
        let Some(resampler) = &mut self.resampler else {
            return Self::emit(&self.writer, pcm, &mut self.cancelled);
        };
        self.input.extend_from_slice(pcm);
        while self.input.len() >= resampler.input_frames_next() {
            let input =
                InterleavedSlice::new(&self.input[..], /*channels*/ 1, self.input.len())
                    .map_err(|_| "invalid audio resampler input")?;
            let capacity = self.scratch.len();
            let mut output =
                InterleavedSlice::new_mut(&mut self.scratch, /*channels*/ 1, capacity)
                    .map_err(|_| "invalid audio resampler output")?;
            let (consumed, produced) = resampler
                .process_into_buffer(&input, &mut output, /*indexing*/ None)
                .map_err(|_| "failed to resample speaker audio")?;
            self.input.drain(..consumed);
            Self::emit(&self.writer, &self.scratch[..produced], &mut self.cancelled)?;
        }
        Ok(())
    }

    fn emit(
        writer: &PlaybackWriter,
        pcm: &[f32],
        cancelled: &mut bool,
    ) -> Result<(), &'static str> {
        for block in pcm.chunks(BLOCK) {
            let bytes: Vec<u8> = block.iter().flat_map(|s| s.to_le_bytes()).collect();
            let mut offset = 0;
            while offset < bytes.len() {
                match writer.write(&bytes[offset..]) {
                    Ok(written) => offset += written,
                    // The epoch ended (suppression or device change); a new Playout follows.
                    Err("speaker writer cancelled") => {
                        *cancelled = true;
                        return Ok(());
                    }
                    Err(error) => {
                        writer.fail_if_current();
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "playout_tests.rs"]
mod tests;
