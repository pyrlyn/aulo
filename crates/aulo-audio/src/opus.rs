//! Opus for remote audio (T6.7): encodes 20 ms frames of 16 kHz mono PCM16 into
//! one Opus packet each, and decodes packets a remote client sends over
//! `VoiceService.Talk`. PCM16 stays the default; this is the low-bandwidth
//! alternative (`AUDIO_FORMAT_OPUS_16KHZ_MONO`).
//!
//! The decoder's input comes from a remote client, so it is untrusted: packet
//! size and frames per packet are capped before libopus sees the bytes, and
//! the output buffer is fixed.

use aulo_speech::ErrorDetail;
use opus::{Application, Bitrate, Channels};

use crate::FRAME_SAMPLES;

const SAMPLE_RATE_HZ: u32 = 16_000;

/// A single Opus frame is at most 1275 bytes (RFC 6716 §3.2.1); the rest of the
/// cap leaves room for the table of contents and a few frames.
pub const MAX_PACKET_BYTES: usize = 1500;
/// Opus allows 48 frames per packet; a client that streams 20 ms frames sends
/// one, so a larger count is abuse or a broken encoder, not a use case.
pub const MAX_FRAMES_PER_PACKET: usize = 6;
/// The longest packet Opus defines is 120 ms, which bounds the decode buffer.
pub const MAX_PACKET_SAMPLES: usize = 1920;
/// Wideband speech in VoIP mode. VBR averages about half this on speech; the
/// headroom is for the STT engine, which needs clean consonants more than a
/// small stream.
pub const DEFAULT_BITRATE_BPS: u32 = 24_000;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OpusError {
    /// libopus refused to create the codec or to process a packet.
    #[error("opus codec failed: {0}")]
    Codec(ErrorDetail),
    /// The encoder takes exactly one 20 ms frame.
    #[error("opus frame must be {max} samples, got {0}", max = FRAME_SAMPLES)]
    FrameSize(usize),
    /// An empty packet would make libopus conceal a lost packet, i.e. invent audio.
    #[error("opus packet is empty")]
    EmptyPacket,
    #[error("opus packet is {0} bytes; at most {max}", max = MAX_PACKET_BYTES)]
    PacketTooLarge(usize),
    #[error("opus packet holds {0} frames; at most {max}", max = MAX_FRAMES_PER_PACKET)]
    TooManyFrames(usize),
}

impl OpusError {
    fn codec(error: &opus::Error) -> Self {
        Self::Codec(ErrorDetail::new(&error.to_string()))
    }
}

/// 20 ms PCM16 frames in, one Opus packet per frame out.
pub struct OpusEncoder {
    inner: opus::Encoder,
    packet: [u8; MAX_PACKET_BYTES],
}

impl OpusEncoder {
    pub fn new(bitrate_bps: u32) -> Result<Self, OpusError> {
        let bits = i32::try_from(bitrate_bps)
            .map_err(|_| OpusError::Codec(ErrorDetail::new("bitrate out of range")))?;
        let mut inner = opus::Encoder::new(SAMPLE_RATE_HZ, Channels::Mono, Application::Voip)
            .map_err(|e| OpusError::codec(&e))?;
        inner
            .set_bitrate(Bitrate::Bits(bits))
            .map_err(|e| OpusError::codec(&e))?;
        Ok(Self {
            inner,
            packet: [0; MAX_PACKET_BYTES],
        })
    }

    /// The packet borrows the encoder's buffer, so it is valid until the next call.
    pub fn encode(&mut self, frame: &[i16]) -> Result<&[u8], OpusError> {
        if frame.len() != FRAME_SAMPLES {
            return Err(OpusError::FrameSize(frame.len()));
        }
        let len = self
            .inner
            .encode(frame, &mut self.packet)
            .map_err(|e| OpusError::codec(&e))?;
        Ok(&self.packet[..len])
    }

    /// Samples of delay between input and decoded output, for aligning a
    /// round trip.
    pub fn lookahead(&mut self) -> Result<usize, OpusError> {
        let samples = self
            .inner
            .get_lookahead()
            .map_err(|e| OpusError::codec(&e))?;
        Ok(usize::try_from(samples).unwrap_or(0))
    }
}

// libopus state has no `Debug`, and the buffers are noise in a log line.
impl std::fmt::Debug for OpusEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpusEncoder").finish_non_exhaustive()
    }
}

/// Packets from an untrusted client in, PCM16 out.
pub struct OpusDecoder {
    inner: opus::Decoder,
    pcm: [i16; MAX_PACKET_SAMPLES],
}

impl OpusDecoder {
    pub fn new() -> Result<Self, OpusError> {
        let inner =
            opus::Decoder::new(SAMPLE_RATE_HZ, Channels::Mono).map_err(|e| OpusError::codec(&e))?;
        Ok(Self {
            inner,
            pcm: [0; MAX_PACKET_SAMPLES],
        })
    }

    /// The samples borrow the decoder's buffer, so they are valid until the next call.
    pub fn decode(&mut self, packet: &[u8]) -> Result<&[i16], OpusError> {
        if packet.is_empty() {
            return Err(OpusError::EmptyPacket);
        }
        if packet.len() > MAX_PACKET_BYTES {
            return Err(OpusError::PacketTooLarge(packet.len()));
        }
        let frames = opus::packet::get_nb_frames(packet).map_err(|e| OpusError::codec(&e))?;
        if frames > MAX_FRAMES_PER_PACKET {
            return Err(OpusError::TooManyFrames(frames));
        }
        let samples = self
            .inner
            .decode(packet, &mut self.pcm, false)
            .map_err(|e| OpusError::codec(&e))?;
        Ok(&self.pcm[..samples])
    }
}

impl std::fmt::Debug for OpusDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpusDecoder").finish_non_exhaustive()
    }
}
