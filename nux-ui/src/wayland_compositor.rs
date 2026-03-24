//! Minimal Wayland compositor for receiving frames from crosvm.
//!
//! Implements just enough of the Wayland protocol for crosvm's virtio-gpu
//! display to connect and send frames via wl_shm buffers.
//! Input events are queued from the GTK4 thread and dispatched on the
//! compositor thread (Wayland resources are not thread-safe).

use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::io::{FromRawFd, OwnedFd};
use std::sync::{Arc, Mutex, mpsc};
use wayland_server::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_pointer, wl_region, wl_seat, wl_shm,
    wl_shm_pool, wl_subcompositor, wl_subsurface, wl_surface, wl_touch,
};
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource};

use wayland_protocols::wp::linux_dmabuf::zv1::server::{
    zwp_linux_buffer_params_v1, zwp_linux_dmabuf_v1,
};
use wayland_protocols::xdg::shell::server::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::wayland_protocol::{wp_virtio_gpu_metadata_v1, wp_virtio_gpu_surface_metadata_v1};

// ── Public types ──

/// A raw ARGB8888 frame from crosvm.
#[derive(Clone)]
pub struct WaylandFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Vec<u8>,
}

/// Input events sent from GTK4 UI thread to compositor thread.
#[allow(dead_code)]
pub enum InputEvent {
    PointerEnter(f64, f64),
    PointerMotion(f64, f64),
    PointerButton(u32, bool),
    Key(u32, bool),
}

/// Handle for sending input events to crosvm.
#[derive(Clone)]
#[allow(dead_code)]
pub struct WaylandInput {
    tx: mpsc::Sender<InputEvent>,
}

impl std::fmt::Debug for WaylandInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaylandInput").finish()
    }
}

#[allow(dead_code)]
impl WaylandInput {
    pub fn pointer_enter(&self, x: f64, y: f64) {
        let _ = self.tx.send(InputEvent::PointerEnter(x, y));
    }
    pub fn pointer_motion(&self, x: f64, y: f64) {
        let _ = self.tx.send(InputEvent::PointerMotion(x, y));
    }
    pub fn pointer_button(&self, button: u32, pressed: bool) {
        let _ = self.tx.send(InputEvent::PointerButton(button, pressed));
    }
    pub fn key(&self, keycode: u32, pressed: bool) {
        let _ = self.tx.send(InputEvent::Key(keycode, pressed));
    }
}

// ── Compositor state ──

pub struct Compositor {
    frame_tx: mpsc::Sender<WaylandFrame>,
    input_rx: mpsc::Receiver<InputEvent>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    surface: Option<wl_surface::WlSurface>,
    serial: u32,
    /// Keep the keymap memfd alive until the compositor is dropped
    _keymap_fd: Option<OwnedFd>,
}

impl Compositor {
    fn process_input(&mut self) {
        let time = millis();
        while let Ok(event) = self.input_rx.try_recv() {
            match event {
                InputEvent::PointerEnter(x, y) => {
                    self.serial += 1;
                    let s = self.serial;
                    if let (Some(ptr), Some(surf)) = (&self.pointer, &self.surface) {
                        ptr.enter(s, surf, x, y);
                        ptr.frame();
                    }
                }
                InputEvent::PointerMotion(x, y) => {
                    if let Some(ptr) = &self.pointer {
                        ptr.motion(time, x, y);
                        ptr.frame();
                    }
                }
                InputEvent::PointerButton(button, pressed) => {
                    self.serial += 1;
                    let s = self.serial;
                    let state = if pressed {
                        wl_pointer::ButtonState::Pressed
                    } else {
                        wl_pointer::ButtonState::Released
                    };
                    if let Some(ptr) = &self.pointer {
                        ptr.button(s, time, button, state);
                        ptr.frame();
                    }
                }
                InputEvent::Key(keycode, pressed) => {
                    self.serial += 1;
                    let s = self.serial;
                    let state = if pressed {
                        wl_keyboard::KeyState::Pressed
                    } else {
                        wl_keyboard::KeyState::Released
                    };
                    if let Some(kb) = &self.keyboard {
                        kb.key(s, time, keycode, state);
                    }
                }
            }
        }
    }
}

fn millis() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u32
}

// ── Data types for Wayland objects ──

struct SurfaceData {
    buffer: Mutex<Option<wl_buffer::WlBuffer>>,
}
struct ShmPoolData {
    fd: Arc<OwnedFd>,
    len: usize,
    /// Persistent mmap — shared with buffers via Arc
    mmap: Arc<PoolMmap>,
}

/// Persistent mmap of a shm pool. Unmapped on drop.
struct PoolMmap {
    ptr: *mut u8,
    len: usize,
}

unsafe impl Send for PoolMmap {}
unsafe impl Sync for PoolMmap {}

impl Drop for PoolMmap {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                libc::munmap(self.ptr.cast(), self.len);
            }
        }
    }
}

struct BufferData {
    mmap: Arc<PoolMmap>,
    offset: i32,
    width: i32,
    height: i32,
    stride: i32,
}
struct RegionData;
struct MetadataGlobalData;
struct SurfaceMetadataData {
    #[allow(dead_code)]
    surface: wl_surface::WlSurface,
}
struct SubcompositorData;
struct SubsurfaceData;
struct SeatData;
struct PointerData;
struct KeyboardData;
struct TouchData;
struct DmabufData;
struct DmabufParamsData;
struct XdgWmBaseData;
struct XdgSurfaceData;
struct XdgToplevelData;

// ── GlobalDispatch implementations ──

impl GlobalDispatch<wl_compositor::WlCompositor, ()> for Compositor {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        r: New<wl_compositor::WlCompositor>,
        _: &(),
        di: &mut DataInit<'_, Self>,
    ) {
        di.init(r, ());
    }
}

impl GlobalDispatch<wl_shm::WlShm, ()> for Compositor {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        r: New<wl_shm::WlShm>,
        _: &(),
        di: &mut DataInit<'_, Self>,
    ) {
        let shm = di.init(r, ());
        shm.format(wl_shm::Format::Argb8888);
        shm.format(wl_shm::Format::Xrgb8888);
    }
}

impl GlobalDispatch<wl_subcompositor::WlSubcompositor, SubcompositorData> for Compositor {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        r: New<wl_subcompositor::WlSubcompositor>,
        _: &SubcompositorData,
        di: &mut DataInit<'_, Self>,
    ) {
        di.init(r, SubcompositorData);
    }
}

impl GlobalDispatch<wl_seat::WlSeat, SeatData> for Compositor {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        r: New<wl_seat::WlSeat>,
        _: &SeatData,
        di: &mut DataInit<'_, Self>,
    ) {
        let seat = di.init(r, SeatData);
        seat.capabilities(wl_seat::Capability::Pointer | wl_seat::Capability::Keyboard);
    }
}

impl GlobalDispatch<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1, DmabufData> for Compositor {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        r: New<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1>,
        _: &DmabufData,
        di: &mut DataInit<'_, Self>,
    ) {
        let dmabuf = di.init(r, DmabufData);
        dmabuf.format(0x34325241); // ARGB8888
        dmabuf.format(0x34325258); // XRGB8888
    }
}

impl GlobalDispatch<xdg_wm_base::XdgWmBase, XdgWmBaseData> for Compositor {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        r: New<xdg_wm_base::XdgWmBase>,
        _: &XdgWmBaseData,
        di: &mut DataInit<'_, Self>,
    ) {
        di.init(r, XdgWmBaseData);
    }
}

impl GlobalDispatch<wp_virtio_gpu_metadata_v1::WpVirtioGpuMetadataV1, MetadataGlobalData>
    for Compositor
{
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        r: New<wp_virtio_gpu_metadata_v1::WpVirtioGpuMetadataV1>,
        _: &MetadataGlobalData,
        di: &mut DataInit<'_, Self>,
    ) {
        di.init(r, MetadataGlobalData);
    }
}

// ── Dispatch implementations ──

impl Dispatch<wl_compositor::WlCompositor, ()> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_compositor::WlCompositor,
        req: wl_compositor::Request,
        _: &(),
        _: &DisplayHandle,
        di: &mut DataInit<'_, Self>,
    ) {
        match req {
            wl_compositor::Request::CreateSurface { id } => {
                di.init(
                    id,
                    SurfaceData {
                        buffer: Mutex::new(None),
                    },
                );
            }
            wl_compositor::Request::CreateRegion { id } => {
                di.init(id, RegionData);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_surface::WlSurface, SurfaceData> for Compositor {
    fn request(
        state: &mut Self,
        _: &Client,
        resource: &wl_surface::WlSurface,
        req: wl_surface::Request,
        _: &SurfaceData,
        _: &DisplayHandle,
        di: &mut DataInit<'_, Self>,
    ) {
        let sd = resource.data::<SurfaceData>().unwrap();
        match req {
            wl_surface::Request::Attach { buffer, .. } => {
                *sd.buffer.lock().unwrap() = buffer;
            }
            wl_surface::Request::Commit => {
                if state.surface.is_none() {
                    state.surface = Some(resource.clone());
                    log::info!("wayland: surface registered for input");
                }
                let buf_ref = sd.buffer.lock().unwrap();
                if let Some(ref buffer) = *buf_ref {
                    if let Some(buf_data) = buffer.data::<BufferData>() {
                        if let Some(frame) = read_buffer_pixels(buf_data) {
                            let _ = state.frame_tx.send(frame);
                        }
                    }
                    buffer.release();
                }
            }
            wl_surface::Request::Frame { callback } => {
                let cb = di.init(callback, ());
                cb.done(0);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_callback::WlCallback,
        _: wl_callback::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl Dispatch<wl_region::WlRegion, RegionData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_region::WlRegion,
        _: wl_region::Request,
        _: &RegionData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl Dispatch<wl_shm::WlShm, ()> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_shm::WlShm,
        req: wl_shm::Request,
        _: &(),
        _: &DisplayHandle,
        di: &mut DataInit<'_, Self>,
    ) {
        if let wl_shm::Request::CreatePool { id, fd, size } = req {
            let raw_fd = fd.as_raw_fd();
            let owned = unsafe { OwnedFd::from_raw_fd(nix::unistd::dup(raw_fd).unwrap()) };
            let len = size as usize;

            // Persistent mmap — stays alive via Arc, shared with buffers
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    len,
                    libc::PROT_READ,
                    libc::MAP_SHARED,
                    owned.as_raw_fd(),
                    0,
                )
            };
            let mmap_ptr = if ptr == libc::MAP_FAILED {
                log::error!(
                    "wayland: pool mmap failed: {}",
                    std::io::Error::last_os_error()
                );
                std::ptr::null_mut()
            } else {
                ptr.cast::<u8>()
            };

            let mmap = Arc::new(PoolMmap { ptr: mmap_ptr, len });
            di.init(
                id,
                ShmPoolData {
                    fd: Arc::new(owned),
                    len,
                    mmap,
                },
            );
        }
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ShmPoolData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_shm_pool::WlShmPool,
        req: wl_shm_pool::Request,
        data: &ShmPoolData,
        _: &DisplayHandle,
        di: &mut DataInit<'_, Self>,
    ) {
        if let wl_shm_pool::Request::CreateBuffer {
            id,
            offset,
            width,
            height,
            stride,
            ..
        } = req
        {
            di.init(
                id,
                BufferData {
                    mmap: Arc::clone(&data.mmap),
                    offset,
                    width,
                    height,
                    stride,
                },
            );
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, BufferData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_buffer::WlBuffer,
        _: wl_buffer::Request,
        _: &BufferData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl Dispatch<wl_subcompositor::WlSubcompositor, SubcompositorData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_subcompositor::WlSubcompositor,
        req: wl_subcompositor::Request,
        _: &SubcompositorData,
        _: &DisplayHandle,
        di: &mut DataInit<'_, Self>,
    ) {
        if let wl_subcompositor::Request::GetSubsurface { id, .. } = req {
            di.init(id, SubsurfaceData);
        }
    }
}

impl Dispatch<wl_subsurface::WlSubsurface, SubsurfaceData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_subsurface::WlSubsurface,
        _: wl_subsurface::Request,
        _: &SubsurfaceData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, SeatData> for Compositor {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &wl_seat::WlSeat,
        req: wl_seat::Request,
        _: &SeatData,
        _: &DisplayHandle,
        di: &mut DataInit<'_, Self>,
    ) {
        match req {
            wl_seat::Request::GetPointer { id } => {
                let pointer = di.init(id, PointerData);
                state.pointer = Some(pointer);
                log::info!("wayland: pointer created");
            }
            wl_seat::Request::GetKeyboard { id } => {
                let keyboard = di.init(id, KeyboardData);
                // Send required keymap
                let keymap = "xkb_keymap {\n\
                    xkb_keycodes \"(unnamed)\" { minimum = 8; maximum = 255; };\n\
                    xkb_types \"(unnamed)\" { type \"ONE_LEVEL\" { modifiers= none; levels[1]= \"Any\"; }; };\n\
                    xkb_compat \"(unnamed)\" { };\n\
                    xkb_symbols \"(unnamed)\" { };\n\
                };\n";
                let keymap_bytes = keymap.as_bytes();
                if let Ok(fd) = nix::sys::memfd::memfd_create(
                    &std::ffi::CString::new("keymap").unwrap(),
                    nix::sys::memfd::MemFdCreateFlag::MFD_CLOEXEC,
                ) {
                    nix::unistd::write(&fd, keymap_bytes).ok();
                    nix::unistd::lseek(fd.as_raw_fd(), 0, nix::unistd::Whence::SeekSet).ok();
                    keyboard.keymap(
                        wl_keyboard::KeymapFormat::XkbV1,
                        fd.as_fd(),
                        keymap_bytes.len() as u32,
                    );
                    // Keep fd alive — dropping it before flush_clients would SEGV
                    state._keymap_fd = Some(fd);
                }
                state.keyboard = Some(keyboard);
                log::info!("wayland: keyboard created");
            }
            wl_seat::Request::GetTouch { id } => {
                di.init(id, TouchData);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, PointerData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_pointer::WlPointer,
        _: wl_pointer::Request,
        _: &PointerData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}
impl Dispatch<wl_keyboard::WlKeyboard, KeyboardData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_keyboard::WlKeyboard,
        _: wl_keyboard::Request,
        _: &KeyboardData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}
impl Dispatch<wl_touch::WlTouch, TouchData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_touch::WlTouch,
        _: wl_touch::Request,
        _: &TouchData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl Dispatch<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1, DmabufData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
        req: zwp_linux_dmabuf_v1::Request,
        _: &DmabufData,
        _: &DisplayHandle,
        di: &mut DataInit<'_, Self>,
    ) {
        if let zwp_linux_dmabuf_v1::Request::CreateParams { params_id } = req {
            di.init(params_id, DmabufParamsData);
        }
    }
}

impl Dispatch<zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1, DmabufParamsData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        r: &zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
        req: zwp_linux_buffer_params_v1::Request,
        _: &DmabufParamsData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match req {
            zwp_linux_buffer_params_v1::Request::Add { .. } => {}
            zwp_linux_buffer_params_v1::Request::Create { .. }
            | zwp_linux_buffer_params_v1::Request::CreateImmed { .. } => {
                r.failed();
            }
            _ => {}
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, XdgWmBaseData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &xdg_wm_base::XdgWmBase,
        req: xdg_wm_base::Request,
        _: &XdgWmBaseData,
        _: &DisplayHandle,
        di: &mut DataInit<'_, Self>,
    ) {
        if let xdg_wm_base::Request::GetXdgSurface { id, .. } = req {
            di.init(id, XdgSurfaceData);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, XdgSurfaceData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        r: &xdg_surface::XdgSurface,
        req: xdg_surface::Request,
        _: &XdgSurfaceData,
        _: &DisplayHandle,
        di: &mut DataInit<'_, Self>,
    ) {
        if let xdg_surface::Request::GetToplevel { id } = req {
            let toplevel = di.init(id, XdgToplevelData);
            toplevel.configure(720, 1280, vec![]);
            r.configure(1);
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, XdgToplevelData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &xdg_toplevel::XdgToplevel,
        _: xdg_toplevel::Request,
        _: &XdgToplevelData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl Dispatch<wp_virtio_gpu_metadata_v1::WpVirtioGpuMetadataV1, MetadataGlobalData> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wp_virtio_gpu_metadata_v1::WpVirtioGpuMetadataV1,
        req: wp_virtio_gpu_metadata_v1::Request,
        _: &MetadataGlobalData,
        _: &DisplayHandle,
        di: &mut DataInit<'_, Self>,
    ) {
        let wp_virtio_gpu_metadata_v1::Request::GetSurfaceMetadata { id, surface } = req;
        di.init(id, SurfaceMetadataData { surface });
    }
}

impl Dispatch<wp_virtio_gpu_surface_metadata_v1::WpVirtioGpuSurfaceMetadataV1, SurfaceMetadataData>
    for Compositor
{
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wp_virtio_gpu_surface_metadata_v1::WpVirtioGpuSurfaceMetadataV1,
        req: wp_virtio_gpu_surface_metadata_v1::Request,
        _: &SurfaceMetadataData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        let wp_virtio_gpu_surface_metadata_v1::Request::SetScanoutId { scanout_id } = req;
        log::info!("wayland: surface scanout_id = {scanout_id}");
    }
}

// ── Buffer reading ──

fn read_buffer_pixels(buf: &BufferData) -> Option<WaylandFrame> {
    if buf.mmap.ptr.is_null() || buf.width <= 0 || buf.height <= 0 {
        return None;
    }
    let pixel_offset = buf.offset as usize;
    let pixel_len = (buf.stride * buf.height) as usize;
    if pixel_offset + pixel_len > buf.mmap.len {
        return None;
    }

    // Read directly from persistent mmap — zero syscall overhead
    let data =
        unsafe { std::slice::from_raw_parts(buf.mmap.ptr.add(pixel_offset), pixel_len) }.to_vec();

    Some(WaylandFrame {
        width: buf.width as u32,
        height: buf.height as u32,
        stride: buf.stride as u32,
        data,
    })
}

// ── Public API ──

/// Start the Wayland compositor on a socket path.
/// Returns (frame_receiver, input_handle).
pub fn start_compositor_at_path(
    socket_path: &str,
) -> Result<(mpsc::Receiver<WaylandFrame>, WaylandInput), String> {
    let _ = std::fs::remove_file(socket_path);
    if let Some(parent) = std::path::Path::new(socket_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let (frame_tx, frame_rx) = mpsc::channel();
    let (input_tx, input_rx) = mpsc::channel();

    let mut display = wayland_server::Display::<Compositor>::new()
        .map_err(|e| format!("Failed to create Wayland display: {e}"))?;

    let dh = display.handle();
    dh.create_global::<Compositor, wl_compositor::WlCompositor, ()>(4, ());
    dh.create_global::<Compositor, wl_shm::WlShm, ()>(1, ());
    dh.create_global::<Compositor, wl_subcompositor::WlSubcompositor, SubcompositorData>(
        1,
        SubcompositorData,
    );
    dh.create_global::<Compositor, wl_seat::WlSeat, SeatData>(5, SeatData);
    dh.create_global::<Compositor, zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1, DmabufData>(
        3, DmabufData,
    );
    dh.create_global::<Compositor, xdg_wm_base::XdgWmBase, XdgWmBaseData>(2, XdgWmBaseData);
    dh.create_global::<Compositor, wp_virtio_gpu_metadata_v1::WpVirtioGpuMetadataV1, MetadataGlobalData>(1, MetadataGlobalData);

    let unix_listener = std::os::unix::net::UnixListener::bind(socket_path)
        .map_err(|e| format!("Failed to bind socket at {socket_path}: {e}"))?;
    unix_listener
        .set_nonblocking(true)
        .map_err(|e| format!("set_nonblocking: {e}"))?;

    let mut state = Compositor {
        frame_tx,
        input_rx,
        pointer: None,
        keyboard: None,
        surface: None,
        serial: 0,
        _keymap_fd: None,
    };

    log::info!("wayland: compositor listening on {socket_path}");

    std::thread::spawn(move || {
        loop {
            match unix_listener.accept() {
                Ok((stream, _)) => {
                    log::info!("wayland: client connected");
                    if let Err(e) = display.handle().insert_client(stream, Arc::new(())) {
                        log::error!("wayland: insert_client: {e}");
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => log::error!("wayland: accept: {e}"),
            }

            state.process_input();
            display.dispatch_clients(&mut state).ok();
            display.flush_clients().ok();

            std::thread::sleep(std::time::Duration::from_micros(500));
        }
    });

    let wayland_input = WaylandInput { tx: input_tx };
    Ok((frame_rx, wayland_input))
}
