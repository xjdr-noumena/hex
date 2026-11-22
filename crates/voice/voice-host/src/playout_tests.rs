// Hex: replaces Codex's GStreamer playout tests with tests of the in-process pipeline.
use super::*;

fn rtp(seq: u16, payload: &[u8]) -> Vec<u8> {
    let mut packet = vec![0x80, 111];
    packet.extend_from_slice(&seq.to_be_bytes());
    packet.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    packet.extend_from_slice(payload);
    packet
}

#[test]
fn parses_payload_after_csrc_extension_and_padding() {
    assert_eq!(parse_rtp(&rtp(7, b"abc")), Some((7, &b"abc"[..])));
    // One CSRC, a one-word extension and two bytes of padding.
    let mut packet = vec![0xb1, 111, 0, 9, 0, 0, 0, 0, 0, 0, 0, 1];
    packet.extend_from_slice(&[0, 0, 0, 2]); // CSRC
    packet.extend_from_slice(&[0xbe, 0xde, 0, 1, 1, 2, 3, 4]); // extension header + 1 word
    packet.extend_from_slice(b"opus");
    packet.extend_from_slice(&[0, 2]); // padding, last byte = count
    assert_eq!(parse_rtp(&packet), Some((9, &b"opus"[..])));
    assert_eq!(parse_rtp(&[0x40, 0, 0, 1]), None);
}

#[test]
fn extends_sequence_numbers_across_wraparound() {
    let decoder = opus::Decoder::new(48_000, opus::Channels::Mono).unwrap();
    let mut pipeline = Pipeline {
        decoder,
        output: Output {
            writer: test_writer(),
            resampler: None,
            input: Vec::new(),
            scratch: Vec::new(),
            cancelled: false,
        },
        held: BTreeMap::new(),
        newest: None,
        next: None,
        last_audio: Instant::now(),
        pcm: vec![0.0; MAX_FRAME_SAMPLES],
    };
    let now = Instant::now();
    for seq in [65534u16, 65535, 0, 1] {
        pipeline.hold(Packet {
            seq,
            payload: vec![0xf8],
            at: now,
        });
    }
    let order: Vec<u64> = pipeline.held.keys().copied().collect();
    assert_eq!(order.len(), 4);
    assert!(order.windows(2).all(|w| w[1] == w[0] + 1), "{order:?}");
}

fn test_writer() -> PlaybackWriter {
    let buffers = std::sync::Arc::new(super::super::buffers::Buffers::new(48_000, 48_000));
    super::super::playback::PlaybackPort::new(buffers, 48_000).writer()
}
