//! FFmpeg H.264 decoder — low-latency decoding of raw Annex B stream into RGB frames.
//!
//! Uses a simple Annex B start code scanner to split raw TCP chunks into
//! complete NAL units before feeding them to FFmpeg's decoder.

use ffmpeg_next as ffmpeg;
use ffmpeg_next::codec;
use ffmpeg_next::format::Pixel;
use ffmpeg_next::software::scaling;

/// Decoded BGRA frame ready for cairo rendering.
#[derive(Clone)]
pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // BGRA32 (cairo native format)
    pub stride: usize,
}

/// H.264 stream decoder with Annex B NAL accumulator.
pub struct H264Decoder {
    decoder: codec::decoder::Video,
    scaler: Option<scaling::Context>,
    last_width: u32,
    last_height: u32,
    buf: Vec<u8>,
    positions: Vec<usize>,
    seen_sps: bool,
}

impl H264Decoder {
    pub fn new() -> Result<Self, String> {
        ffmpeg::init().map_err(|e| format!("FFmpeg init failed: {e}"))?;

        let codec = codec::decoder::find(codec::Id::H264)
            .ok_or_else(|| "H.264 codec not found".to_owned())?;

        let mut context = codec::Context::new_with_codec(codec);

        context.set_threading(codec::threading::Config {
            kind: codec::threading::Type::Slice,
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
            buf: Vec::with_capacity(256 * 1024),
            positions: Vec::with_capacity(128),
            seen_sps: false,
        })
    }

    /// Feed raw H.264 Annex B data (arbitrary TCP chunk).
    pub fn feed_raw(&mut self, data: &[u8]) -> Vec<DecodedFrame> {
        self.buf.extend_from_slice(data);

        let mut frames = Vec::new();

        self.positions.clear();
        find_start_codes_into(&self.buf, &mut self.positions);

        if self.positions.len() < 2 {
            return frames;
        }

        if !self.seen_sps {
            let mut sps_idx = None;
            for (i, &pos) in self.positions.iter().enumerate() {
                let nal_type = nal_unit_type(&self.buf, pos);
                if nal_type == 7 {
                    sps_idx = Some(i);
                    self.seen_sps = true;
                    log::info!("decoder: found SPS at position {pos}, starting decode");
                    break;
                }
            }
            if let Some(idx) = sps_idx {
                let sps_pos = self.positions[idx];
                let tail = self.buf[sps_pos..].to_vec();
                self.buf.clear();
                self.buf.extend_from_slice(&tail);
                return self.feed_raw(&[]);
            }
            let last = *self.positions.last().unwrap();
            let tail = self.buf[last..].to_vec();
            self.buf.clear();
            self.buf.extend_from_slice(&tail);
            return frames;
        }

        let last_complete = *self.positions.last().unwrap();
        for i in 0..self.positions.len() - 1 {
            let start = self.positions[i];
            let end = self.positions[i + 1];
            let nal_data = &self.buf[start..end];

            let packet = codec::packet::Packet::copy(nal_data);
            if self.decoder.send_packet(&packet).is_ok() {
                let mut decoded = ffmpeg::frame::Video::empty();
                while self.decoder.receive_frame(&mut decoded).is_ok() {
                    if let Some(frame) = self.convert_frame(&decoded) {
                        frames.push(frame);
                    }
                }
            }
        }

        let tail = self.buf[last_complete..].to_vec();
        self.buf.clear();
        self.buf.extend_from_slice(&tail);

        frames
    }

    /// Flush buffered frames.
    pub fn flush(&mut self) -> Vec<DecodedFrame> {
        let mut frames = Vec::new();
        if !self.buf.is_empty() {
            let packet = codec::packet::Packet::copy(&self.buf);
            if self.decoder.send_packet(&packet).is_ok() {
                let mut decoded = ffmpeg::frame::Video::empty();
                while self.decoder.receive_frame(&mut decoded).is_ok() {
                    if let Some(frame) = self.convert_frame(&decoded) {
                        frames.push(frame);
                    }
                }
            }
            self.buf.clear();
        }
        self.decoder.send_eof().ok();
        let mut decoded = ffmpeg::frame::Video::empty();
        while self.decoder.receive_frame(&mut decoded).is_ok() {
            if let Some(frame) = self.convert_frame(&decoded) {
                frames.push(frame);
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
                Pixel::BGRA,
                width,
                height,
                scaling::Flags::POINT,
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

fn find_start_codes_into(data: &[u8], positions: &mut Vec<usize>) {
    let len = data.len();
    let mut i = 0;
    while i + 2 < len {
        if data[i] == 0 && data[i + 1] == 0 {
            if data[i + 2] == 1 {
                positions.push(i);
                i += 3;
                continue;
            } else if i + 3 < len && data[i + 2] == 0 && data[i + 3] == 1 {
                positions.push(i);
                i += 4;
                continue;
            }
        }
        i += 1;
    }
}

fn nal_unit_type(data: &[u8], start_code_pos: usize) -> u8 {
    let header_pos = if start_code_pos + 3 < data.len()
        && data[start_code_pos + 2] == 0
        && data[start_code_pos + 3] == 1
    {
        start_code_pos + 4
    } else {
        start_code_pos + 3
    };
    if header_pos < data.len() {
        data[header_pos] & 0x1F
    } else {
        0
    }
}
