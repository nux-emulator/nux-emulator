//! Frame capture types and backends for the display pipeline.
//!
//! Defines the [`Frame`] type, [`CaptureSource`] enum dispatch, and concrete
//! implementations for dmabuf zero-copy capture and shared-memory fallback.

use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::Arc;

use tokio::sync::watch;

use super::error::{DisplayError, DisplayResult};

/// Pixel format identifier (DRM fourcc code).
///
/// Common values: `ARGB8888 = 0x3432_3841`, `XRGB8888 = 0x3432_3858`.
pub type Fourcc = u32;

/// A single captured frame from the VM display.
#[derive(Debug, Clone)]
pub enum Frame {
    /// Zero-copy frame backed by a dmabuf file descriptor.
    Dmabuf {
        /// Owned dmabuf file descriptor (shared via `Arc` for cheap cloning).
        fd: Arc<OwnedFd>,
        /// Frame width in pixels.
        width: u32,
        /// Frame height in pixels.
        height: u32,
        /// Row stride in bytes.
        stride: u32,
        /// Pixel format as a DRM fourcc code.
        fourcc: Fourcc,
    },
    /// CPU-accessible frame backed by shared memory.
    Shm {
        /// Raw pixel data (BGRA8888 layout).
        data: Arc<Vec<u8>>,
        /// Frame width in pixels.
        width: u32,
        /// Frame height in pixels.
        height: u32,
        /// Row stride in bytes.
        stride: u32,
    },
}

impl Frame {
    /// Returns the frame dimensions as `(width, height)`.
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::Dmabuf { width, height, .. } | Self::Shm { width, height, .. } => {
                (*width, *height)
            }
        }
    }

    /// Returns `true` if this is a dmabuf-backed frame.
    pub fn is_dmabuf(&self) -> bool {
        matches!(self, Self::Dmabuf { .. })
    }
}

/// The active capture backend kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackend {
    /// Zero-copy dmabuf capture.
    Dmabuf,
    /// Shared memory fallback capture.
    Shm,
}

impl std::fmt::Display for CaptureBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dmabuf => write!(f, "dmabuf"),
            Self::Shm => write!(f, "shared-memory"),
        }
    }
}

// ── Dmabuf capture ──────────────────────────────────────────────────────────

/// Dmabuf zero-copy frame capture from crosvm's gfxstream surfaceless output.
///
/// Receives dmabuf file descriptors exported by the VM's GPU backend and
/// wraps them in safe [`OwnedFd`] handles that close on drop.
pub struct DmabufCapture {
    /// Whether dmabuf support was detected at startup.
    supported: bool,
}

impl DmabufCapture {
    /// Create a new dmabuf capture backend.
    pub fn new() -> Self {
        Self { supported: false }
    }

    /// Probe whether the system supports dmabuf import.
    ///
    /// Attempts to find a DRM render node to verify driver and kernel support.
    /// On success, marks this backend as supported.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::DmabufImportFailed` if dmabuf is not available.
    pub fn detect(&mut self) -> DisplayResult<()> {
        let render_nodes = [
            "/dev/dri/renderD128",
            "/dev/dri/renderD129",
            "/dev/dri/renderD130",
        ];

        for path in &render_nodes {
            if std::fs::metadata(path).is_ok() {
                log::info!("dmabuf support detected via render node {path}");
                self.supported = true;
                return Ok(());
            }
        }

        Err(DisplayError::DmabufImportFailed(
            "no DRM render nodes found — dmabuf not available".to_owned(),
        ))
    }

    /// Returns whether dmabuf support was detected.
    pub fn is_supported(&self) -> bool {
        self.supported
    }

    /// Wrap a raw dmabuf file descriptor into a [`Frame::Dmabuf`].
    ///
    /// The returned frame takes ownership of the FD via [`OwnedFd`].
    ///
    /// # Safety
    ///
    /// The caller must ensure `raw_fd` is a valid, open file descriptor
    /// that is not owned by any other resource. Ownership transfers to
    /// the returned `Frame`.
    #[allow(unsafe_code)]
    pub unsafe fn wrap_dmabuf_fd(
        raw_fd: std::os::unix::io::RawFd,
        width: u32,
        height: u32,
        stride: u32,
        fourcc: Fourcc,
    ) -> Frame {
        // SAFETY: Caller guarantees `raw_fd` is valid and exclusively owned.
        let owned = unsafe { OwnedFd::from_raw_fd(raw_fd) };
        Frame::Dmabuf {
            fd: Arc::new(owned),
            width,
            height,
            stride,
            fourcc,
        }
    }

    /// Start capturing dmabuf frames.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::DmabufImportFailed` if detection was not run.
    async fn start(&self, sender: watch::Sender<Option<Frame>>) -> DisplayResult<()> {
        if !self.supported {
            return Err(DisplayError::DmabufImportFailed(
                "dmabuf not detected — call detect() first".to_owned(),
            ));
        }

        // In production, this loop receives dmabuf FDs from crosvm's
        // virtio-gpu export mechanism. For now, hold the channel open
        // until pipeline shutdown.
        log::info!("dmabuf capture started, waiting for frames from crosvm");
        sender.closed().await;
        Ok(())
    }
}

impl Default for DmabufCapture {
    fn default() -> Self {
        Self::new()
    }
}

// ── Shared memory capture ───────────────────────────────────────────────────

/// Shared memory fallback frame capture.
///
/// Maps crosvm's shared memory rendering region via `mmap` and reads
/// frame data on each render completion.
pub struct ShmCapture {
    _private: (),
}

impl ShmCapture {
    /// Create a new shared memory capture backend.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Create a [`Frame::Shm`] from raw pixel data.
    ///
    /// Wraps the data in an `Arc<Vec<u8>>` for safe sharing across the
    /// watch channel.
    pub fn frame_from_bytes(data: Vec<u8>, width: u32, height: u32, stride: u32) -> Frame {
        Frame::Shm {
            data: Arc::new(data),
            width,
            height,
            stride,
        }
    }

    /// Start capturing shared memory frames.
    ///
    /// # Errors
    ///
    /// Returns a `DisplayError` if the capture source fails.
    async fn start(&self, sender: watch::Sender<Option<Frame>>) -> DisplayResult<()> {
        // In production, this loop maps the crosvm shared memory region
        // and reads frames on render completion signals. For now, hold
        // the channel open until pipeline shutdown.
        log::info!("shared memory capture started, waiting for frames from crosvm");
        sender.closed().await;
        Ok(())
    }
}

impl Default for ShmCapture {
    fn default() -> Self {
        Self::new()
    }
}

// ── Enum dispatch ───────────────────────────────────────────────────────────

/// Capture source that dispatches to the active backend.
///
/// Uses enum dispatch instead of trait objects to avoid async dyn-compatibility
/// issues while keeping a single concrete type for the pipeline to hold.
pub enum CaptureSource {
    /// Dmabuf zero-copy backend.
    Dmabuf(DmabufCapture),
    /// Shared memory fallback backend.
    Shm(ShmCapture),
}

impl CaptureSource {
    /// Start the capture loop, sending frames into `sender`.
    ///
    /// # Errors
    ///
    /// Returns a `DisplayError` if the capture backend fails.
    pub async fn start(&self, sender: watch::Sender<Option<Frame>>) -> DisplayResult<()> {
        match self {
            Self::Dmabuf(cap) => cap.start(sender).await,
            Self::Shm(cap) => cap.start(sender).await,
        }
    }

    /// Returns the backend kind for this capture source.
    pub fn backend(&self) -> CaptureBackend {
        match self {
            Self::Dmabuf(_) => CaptureBackend::Dmabuf,
            Self::Shm(_) => CaptureBackend::Shm,
        }
    }
}

impl std::fmt::Debug for CaptureSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dmabuf(_) => f.debug_tuple("CaptureSource::Dmabuf").finish(),
            Self::Shm(_) => f.debug_tuple("CaptureSource::Shm").finish(),
        }
    }
}

// ── Auto-detection ──────────────────────────────────────────────────────────

/// Detect the best available capture backend.
///
/// Tries dmabuf first; falls back to shared memory if unavailable.
/// Logs the selected backend at info level.
pub fn detect_capture_backend() -> CaptureSource {
    let mut dmabuf = DmabufCapture::new();
    if dmabuf.detect().is_ok() {
        log::info!("capture backend: dmabuf (zero-copy)");
        CaptureSource::Dmabuf(dmabuf)
    } else {
        log::warn!("dmabuf not available, falling back to shared memory capture");
        log::info!("capture backend: shared-memory");
        CaptureSource::Shm(ShmCapture::new())
    }
}

// ── Extension trait ─────────────────────────────────────────────────────────

/// Extension trait for extracting the raw FD from a dmabuf frame.
pub trait DmabufFrameExt {
    /// Returns the raw file descriptor if this is a dmabuf frame.
    fn dmabuf_raw_fd(&self) -> Option<std::os::unix::io::RawFd>;
}

impl DmabufFrameExt for Frame {
    fn dmabuf_raw_fd(&self) -> Option<std::os::unix::io::RawFd> {
        match self {
            Self::Dmabuf { fd, .. } => Some(fd.as_raw_fd()),
            Self::Shm { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_dimensions() {
        let frame = ShmCapture::frame_from_bytes(vec![0; 100], 10, 10, 40);
        assert_eq!(frame.dimensions(), (10, 10));
        assert!(!frame.is_dmabuf());
    }

    #[test]
    fn shm_frame_from_bytes() {
        let data = vec![0xAB; 1920 * 1080 * 4];
        let frame = ShmCapture::frame_from_bytes(data, 1920, 1080, 1920 * 4);
        match &frame {
            Frame::Shm {
                data,
                width,
                height,
                stride,
            } => {
                assert_eq!(*width, 1920);
                assert_eq!(*height, 1080);
                assert_eq!(*stride, 1920 * 4);
                assert_eq!(data.len(), 1920 * 1080 * 4);
            }
            Frame::Dmabuf { .. } => panic!("expected Shm frame"),
        }
    }

    #[test]
    fn capture_backend_display() {
        assert_eq!(CaptureBackend::Dmabuf.to_string(), "dmabuf");
        assert_eq!(CaptureBackend::Shm.to_string(), "shared-memory");
    }

    #[test]
    fn dmabuf_capture_default_not_supported() {
        let cap = DmabufCapture::new();
        assert!(!cap.is_supported());
    }

    #[test]
    fn shm_capture_creates_valid_frames() {
        let frame = ShmCapture::frame_from_bytes(vec![0; 40], 10, 1, 40);
        assert_eq!(frame.dimensions(), (10, 1));
        assert!(!frame.is_dmabuf());
    }

    #[test]
    fn watch_channel_delivers_latest_frame() {
        let (tx, rx) = watch::channel(None::<Frame>);

        let frame1 = ShmCapture::frame_from_bytes(vec![1; 4], 1, 1, 4);
        tx.send(Some(frame1)).unwrap();

        let frame2 = ShmCapture::frame_from_bytes(vec![2; 4], 1, 1, 4);
        tx.send(Some(frame2)).unwrap();

        let latest = rx.borrow().clone();
        assert!(latest.is_some());
        if let Some(Frame::Shm { data, .. }) = latest {
            assert_eq!(data[0], 2, "should see the latest frame, not stale");
        }
    }

    #[test]
    fn dmabuf_frame_ext_returns_none_for_shm() {
        let frame = ShmCapture::frame_from_bytes(vec![0; 4], 1, 1, 4);
        assert!(frame.dmabuf_raw_fd().is_none());
    }

    #[tokio::test]
    async fn dmabuf_start_fails_without_detect() {
        let cap = DmabufCapture::new();
        let (tx, _rx) = watch::channel(None::<Frame>);
        let result = cap.start(tx).await;
        assert!(result.is_err());
    }

    #[test]
    fn detect_capture_backend_returns_a_backend() {
        let source = detect_capture_backend();
        let backend = source.backend();
        assert!(backend == CaptureBackend::Dmabuf || backend == CaptureBackend::Shm);
    }

    #[test]
    fn capture_source_debug_format() {
        let source = CaptureSource::Shm(ShmCapture::new());
        let dbg = format!("{source:?}");
        assert!(dbg.contains("Shm"));
    }
}
