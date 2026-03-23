//! FFmpeg H.264 decoder — decodes raw H.264 Annex B stream into RGB frames.

use ffmpeg_next as ffmpeg;
use ffmpeg_next::codec;
use ffmpeg_next::format::Pixel;
use ffmpeg_next::software::scaling;

/// Decoded RGB frame ready for rendering.
#[derive(Clone)]
pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // RGB24
    pub stride: usize,
}

/// H.264 decoder with proper access unit splitting.
pub struct H264Decoder {
    decoder: codec::decoder::Video,
    scaler: Option<scaling::Context>,
    last_width: u32,
    last_height: u32,
    buffer: Vec<u8>,
}

impl H264Decoder {
    pub fn new() -> Result<Self, String> {
        ffmpeg::init().map_err(|e| format!("FFmpeg init failed: {e}"))?;

        let codec = codec::decoder::find(codec::Id::H264)
            .ok_or_else(|| "H.264 codec not found".to_owned())?;

        let mut context = codec::Context::new_with_codec(codec);
        context.set_threading(codec::threading::Config {
            kind: codec::threading::Type::Frame,
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

    /// Feed raw H.264 bytes, split into access units, decode, return frames.
    pub fn decode_chunk(&mut self, data: &[u8]) -> Vec<DecodedFrame> {
        self.buffer.extend_from_slice(data);

        let mut frames = Vec::new();

        // Split on access unit boundaries and decode each
        while let Some((au, rest)) = split_access_unit(&self.buffer) {
            self.buffer = rest;

            let packet = codec::packet::Packet::copy(&au);
            if self.decoder.send_packet(&packet).is_ok() {
                let mut decoded = ffmpeg::frame::Video::empty();
                while self.decoder.receive_frame(&mut decoded).is_ok() {
                    if let Some(frame) = self.convert_frame(&decoded) {
                        frames.push(frame);
                    }
                }
            }
        }

        frames
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

/// Split buffer at the next access unit boundary.
/// An access unit starts with a VCL NAL (type 1 or 5) preceded by SPS/PPS.
/// We split at the second occurrence of SPS (type 7) or IDR/non-IDR slice start.
fn split_access_unit(buf: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let starts = find_all_start_codes(buf);
    if starts.len() < 2 {
        return None;
    }

    // Find frame boundaries: SPS (7) or slice NALs (1, 5) with first_mb_in_slice == 0
    let mut frame_starts = Vec::new();
    for &pos in &starts {
        let nal_type = nal_type_at(buf, pos)?;
        if nal_type == 7 || nal_type == 1 || nal_type == 5 {
            frame_starts.push(pos);
        }
    }

    if frame_starts.len() < 2 {
        // Prevent unbounded buffer growth
        if buf.len() > 256 * 1024 {
            let end = *starts.last().unwrap();
            return Some((buf[..end].to_vec(), buf[end..].to_vec()));
        }
        return None;
    }

    let end = frame_starts[1];
    Some((buf[..end].to_vec(), buf[end..].to_vec()))
}

fn nal_type_at(buf: &[u8], pos: usize) -> Option<u8> {
    let offset = if buf.get(pos + 2) == Some(&0x01) {
        pos + 3
    } else if buf.get(pos + 3) == Some(&0x01) {
        pos + 4
    } else {
        return None;
    };
    buf.get(offset).map(|b| b & 0x1F)
}

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
