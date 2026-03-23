//! FFmpeg H.264 decoder — uses avformat to properly parse the raw H.264 stream.

use ffmpeg_next as ffmpeg;
use ffmpeg_next::format::Pixel;
use ffmpeg_next::software::scaling;
use std::io::Read;
use std::process::ChildStdout;

/// Decoded RGB frame ready for rendering.
#[derive(Clone)]
pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // RGB24
    pub stride: usize,
}

/// H.264 decoder that reads from a pipe using FFmpeg's full demuxer + decoder pipeline.
/// This properly handles NAL unit boundaries, unlike raw chunk feeding.
pub struct H264Decoder {
    decoder: ffmpeg::codec::decoder::Video,
    scaler: Option<scaling::Context>,
    last_width: u32,
    last_height: u32,
    /// Accumulates raw H.264 bytes and feeds complete access units.
    buffer: Vec<u8>,
}

impl H264Decoder {
    pub fn new() -> Result<Self, String> {
        ffmpeg::init().map_err(|e| format!("FFmpeg init failed: {e}"))?;

        let codec = ffmpeg::codec::decoder::find(ffmpeg::codec::Id::H264)
            .ok_or_else(|| "H.264 codec not found".to_owned())?;

        let mut context = ffmpeg::codec::Context::new_with_codec(codec);
        context.set_threading(ffmpeg::codec::threading::Config {
            kind: ffmpeg::codec::threading::Type::Frame,
            count: 4,
        });

        let decoder = context
            .decoder()
            .video()
            .map_err(|e| format!("Failed to open decoder: {e}"))?;

        Ok(Self {
            decoder,
            scaler: None,
            last_width: 0,
            last_height: 0,
            buffer: Vec::with_capacity(256 * 1024),
        })
    }

    /// Read from the pipe, find complete access units, decode, and return frames.
    /// An access unit = all NAL units between two frame-starting NALs.
    pub fn read_and_decode(
        &mut self,
        stdout: &mut ChildStdout,
    ) -> Result<Vec<DecodedFrame>, String> {
        let mut read_buf = [0u8; 32768];
        let n = stdout
            .read(&mut read_buf)
            .map_err(|e| format!("Read: {e}"))?;
        if n == 0 {
            return Err("EOF".to_owned());
        }

        self.buffer.extend_from_slice(&read_buf[..n]);

        let mut frames = Vec::new();

        // Find access unit boundaries and decode them
        while let Some((au, remaining)) = split_access_unit(&self.buffer) {
            self.buffer = remaining;

            let packet = ffmpeg::codec::packet::Packet::copy(&au);
            if self.decoder.send_packet(&packet).is_ok() {
                let mut decoded = ffmpeg::frame::Video::empty();
                while self.decoder.receive_frame(&mut decoded).is_ok() {
                    if let Some(frame) = self.convert_frame(&decoded) {
                        frames.push(frame);
                    }
                }
            }
        }

        Ok(frames)
    }

    fn convert_frame(&mut self, frame: &ffmpeg::frame::Video) -> Option<DecodedFrame> {
        let width = frame.width();
        let height = frame.height();

        if width == 0 || height == 0 {
            return None;
        }

        if width != self.last_width || height != self.last_height || self.scaler.is_none() {
            self.scaler = scaling::Context::get(
                frame.format(),
                width,
                height,
                Pixel::RGB24,
                width,
                height,
                scaling::Flags::FAST_BILINEAR,
            )
            .ok();
            self.last_width = width;
            self.last_height = height;
        }

        let scaler = self.scaler.as_mut()?;
        let mut rgb_frame = ffmpeg::frame::Video::empty();
        scaler.run(frame, &mut rgb_frame).ok()?;

        let stride = rgb_frame.stride(0);
        let data_len = stride * height as usize;
        let data = rgb_frame.data(0)[..data_len].to_vec();

        Some(DecodedFrame {
            width,
            height,
            data,
            stride,
        })
    }
}

/// Split buffer into a complete access unit and the remainder.
/// An access unit starts with SPS/PPS/IDR/non-IDR slice NAL and ends
/// at the next such NAL. We detect frame boundaries by NAL type.
fn split_access_unit(buf: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let positions = find_all_start_codes(buf);

    if positions.len() < 2 {
        return None;
    }

    // Find the second frame-starting NAL — that marks the end of the first access unit
    let mut frame_starts = Vec::new();
    for &pos in &positions {
        let nal_type = get_nal_type(buf, pos);
        // Frame-starting NAL types: 1 (non-IDR slice), 5 (IDR slice), 7 (SPS), 8 (PPS)
        if matches!(nal_type, Some(1 | 5 | 7)) {
            frame_starts.push(pos);
        }
    }

    if frame_starts.len() < 2 {
        // Not enough frame boundaries yet — but if buffer is large, flush first NAL group
        if buf.len() > 128 * 1024 && positions.len() >= 2 {
            let end = *positions.last().unwrap();
            let au = buf[..end].to_vec();
            let rest = buf[end..].to_vec();
            return Some((au, rest));
        }
        return None;
    }

    // Return everything up to the second frame start
    let end = frame_starts[1];
    let au = buf[..end].to_vec();
    let rest = buf[end..].to_vec();
    Some((au, rest))
}

/// Get NAL unit type from the byte after the start code.
fn get_nal_type(buf: &[u8], start_pos: usize) -> Option<u8> {
    let offset = if buf.get(start_pos + 2) == Some(&0x01) {
        start_pos + 3
    } else if buf.get(start_pos + 3) == Some(&0x01) {
        start_pos + 4
    } else {
        return None;
    };

    buf.get(offset).map(|b| b & 0x1F)
}

/// Find all H.264 start code positions in the buffer.
fn find_all_start_codes(buf: &[u8]) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut i = 0;

    while i < buf.len().saturating_sub(3) {
        if buf[i] == 0x00 && buf[i + 1] == 0x00 {
            if buf[i + 2] == 0x01 || (buf[i + 2] == 0x00 && i + 3 < buf.len() && buf[i + 3] == 0x01)
            {
                positions.push(i);
                i += 3;
                continue;
            }
        }
        i += 1;
    }

    positions
}
