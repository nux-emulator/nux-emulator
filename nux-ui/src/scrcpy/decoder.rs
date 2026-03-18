//! FFmpeg H.264 decoder — decodes raw H.264 stream into RGB frames.

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

/// H.264 decoder using FFmpeg.
/// Accepts raw H.264 byte chunks and outputs RGB frames.
pub struct H264Decoder {
    decoder: codec::decoder::Video,
    scaler: Option<scaling::Context>,
    last_width: u32,
    last_height: u32,
}

impl H264Decoder {
    /// Create a new H.264 decoder.
    pub fn new() -> Result<Self, String> {
        ffmpeg::init().map_err(|e| format!("FFmpeg init failed: {e}"))?;

        let codec = codec::decoder::find(codec::Id::H264)
            .ok_or_else(|| "H.264 codec not found".to_owned())?;

        let mut context = codec::Context::new_with_codec(codec);
        context.set_threading(codec::threading::Config {
            kind: codec::threading::Type::Frame,
            count: 2,
        });

        let decoder = context
            .decoder()
            .video()
            .map_err(|e| format!("Failed to open H.264 decoder: {e}"))?;

        Ok(Self {
            decoder,
            scaler: None,
            last_width: 0,
            last_height: 0,
        })
    }

    /// Feed raw H.264 bytes and return any decoded RGB frames.
    pub fn decode_chunk(&mut self, data: &[u8]) -> Vec<DecodedFrame> {
        let mut frames = Vec::new();

        let packet = codec::packet::Packet::copy(data);

        if self.decoder.send_packet(&packet).is_ok() {
            let mut decoded = ffmpeg::frame::Video::empty();
            while self.decoder.receive_frame(&mut decoded).is_ok() {
                if let Some(frame) = self.convert_frame(&decoded) {
                    frames.push(frame);
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
                scaling::Flags::BILINEAR,
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
