//! Display pipeline for the Nux Emulator.
//!
//! Captures frames from crosvm's GPU output (via dmabuf zero-copy or shared
//! memory fallback) and delivers them to the UI layer through a
//! `tokio::sync::watch` channel. The presentation widget in `nux-ui`
//! consumes frames from this channel.
//!
//! # Architecture
//!
//! ```text
//! crosvm (gfxstream) ──► CaptureSource ──► watch channel ──► nux-ui widget
//!                         (dmabuf/shm)
//! ```
//!
//! The pipeline auto-detects the best capture backend at startup:
//! dmabuf if the GPU driver supports it, shared memory otherwise.

pub mod capture;
pub mod config;
pub mod error;
pub mod sync;

use capture::{CaptureBackend, CaptureSource, Frame, detect_capture_backend};
use config::DisplayPipelineConfig;
use error::{DisplayError, DisplayResult};
use sync::{FpsCounter, FramePacer};
use tokio::sync::watch;

/// Top-level display pipeline orchestrator.
///
/// Manages capture backend selection, frame delivery channel, and
/// presentation timing state. The UI layer receives frames from the
/// watch channel receiver returned by [`receiver`](Self::receiver).
pub struct DisplayPipeline {
    config: DisplayPipelineConfig,
    backend: CaptureBackend,
    capture: CaptureSource,
    tx: watch::Sender<Option<Frame>>,
    rx: watch::Receiver<Option<Frame>>,
    fps_counter: FpsCounter,
    pacer: FramePacer,
}

impl DisplayPipeline {
    /// Create a new display pipeline with the given configuration.
    ///
    /// Validates the config, detects the best capture backend, and
    /// creates the frame delivery channel.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::ConfigValidation` if the config is invalid.
    pub fn new(config: DisplayPipelineConfig) -> DisplayResult<Self> {
        config.validate()?;

        let capture = detect_capture_backend();
        let backend = capture.backend();
        let (tx, rx) = watch::channel(None::<Frame>);
        let pacer = FramePacer::new(config.vsync);
        let fps_counter = FpsCounter::new();

        log::info!(
            "display pipeline initialized: {}x{}, backend={backend}, vsync={}",
            config.width,
            config.height,
            config.vsync,
        );

        Ok(Self {
            config,
            backend,
            capture,
            tx,
            rx,
            fps_counter,
            pacer,
        })
    }

    /// Create a new display pipeline with default configuration.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::ConfigValidation` if the default config is
    /// somehow invalid (should not happen).
    pub fn with_defaults() -> DisplayResult<Self> {
        Self::new(DisplayPipelineConfig::default())
    }

    /// Start the frame capture loop.
    ///
    /// Runs the selected capture backend as an async task that sends
    /// frames into the watch channel. The returned future completes when
    /// the capture source is exhausted or the pipeline is shut down.
    ///
    /// # Errors
    ///
    /// Returns a `DisplayError` if the capture backend fails to start.
    pub async fn start_capture(&self) -> DisplayResult<()> {
        self.capture.start(self.tx.clone()).await
    }

    /// Get a clone of the frame watch channel receiver.
    ///
    /// The UI layer uses this to poll for the latest frame on each
    /// frame clock tick.
    pub fn receiver(&self) -> watch::Receiver<Option<Frame>> {
        self.rx.clone()
    }

    /// Get the active capture backend.
    pub fn backend(&self) -> CaptureBackend {
        self.backend
    }

    /// Get a reference to the display configuration.
    pub fn config(&self) -> &DisplayPipelineConfig {
        &self.config
    }

    /// Get a mutable reference to the FPS counter.
    pub fn fps_counter_mut(&mut self) -> &mut FpsCounter {
        &mut self.fps_counter
    }

    /// Get a reference to the FPS counter.
    pub fn fps_counter(&self) -> &FpsCounter {
        &self.fps_counter
    }

    /// Get a mutable reference to the frame pacer.
    pub fn pacer_mut(&mut self) -> &mut FramePacer {
        &mut self.pacer
    }

    /// Get a reference to the frame pacer.
    pub fn pacer(&self) -> &FramePacer {
        &self.pacer
    }

    /// Send a frame into the pipeline (used by capture backends).
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::ChannelClosed` if all receivers have been dropped.
    pub fn send_frame(&self, frame: Frame) -> DisplayResult<()> {
        self.tx
            .send(Some(frame))
            .map_err(|_| DisplayError::ChannelClosed)
    }

    /// Shut down the pipeline by dropping the sender.
    ///
    /// This causes the capture task to complete and all receivers to
    /// see `None` as the final value.
    pub fn shutdown(self) {
        drop(self.tx);
        log::info!("display pipeline shut down");
    }
}

impl std::fmt::Debug for DisplayPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DisplayPipeline")
            .field("config", &self.config)
            .field("backend", &self.backend)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_creates_with_defaults() {
        let pipeline = DisplayPipeline::with_defaults();
        assert!(pipeline.is_ok());
    }

    #[test]
    fn pipeline_rejects_invalid_config() {
        let config = DisplayPipelineConfig {
            width: 0,
            ..Default::default()
        };
        let result = DisplayPipeline::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn pipeline_backend_is_detected() {
        let pipeline = DisplayPipeline::with_defaults().unwrap();
        let backend = pipeline.backend();
        assert!(backend == CaptureBackend::Dmabuf || backend == CaptureBackend::Shm);
    }

    #[test]
    fn pipeline_receiver_gets_sent_frames() {
        let pipeline = DisplayPipeline::with_defaults().unwrap();
        let rx = pipeline.receiver();

        assert!(rx.borrow().is_none());

        let frame = capture::ShmCapture::frame_from_bytes(vec![0; 4], 1, 1, 4);
        pipeline.send_frame(frame).unwrap();

        assert!(rx.borrow().is_some());
    }

    #[test]
    fn pipeline_config_accessible() {
        let config = DisplayPipelineConfig {
            width: 2560,
            height: 1440,
            ..Default::default()
        };
        let pipeline = DisplayPipeline::new(config).unwrap();
        assert_eq!(pipeline.config().width, 2560);
        assert_eq!(pipeline.config().height, 1440);
    }

    #[test]
    fn pipeline_pacer_respects_config_vsync() {
        let config = DisplayPipelineConfig {
            vsync: false,
            ..Default::default()
        };
        let pipeline = DisplayPipeline::new(config).unwrap();
        assert!(!pipeline.pacer().vsync_enabled());
    }

    #[test]
    fn pipeline_shutdown_closes_channel() {
        let pipeline = DisplayPipeline::with_defaults().unwrap();
        let rx = pipeline.receiver();
        pipeline.shutdown();
        assert!(rx.borrow().is_none());
    }
}
