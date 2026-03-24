//! Main application window — `AdwApplicationWindow` subclass.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use gtk4 as gtk;
use libadwaita as adw;

use crate::display;
use crate::overlay;
use crate::settings;
use crate::state::UiState;
use crate::toolbar;

use crate::vm_launcher::BootStatus;

/// Events sent from background threads to the UI thread.
#[allow(dead_code)]
enum VmEvent {
    Started,
    StartFailed(String),
    ApkInstalled(String, Result<String, String>),
}

/// Wrapper around the main application window and its key child widgets.
pub struct NuxWindow {
    pub window: adw::ApplicationWindow,
    pub toast_overlay: adw::ToastOverlay,
    pub header_bar: adw::HeaderBar,
    pub status_label: gtk::Label,
    pub fps_label: gtk::Label,
    pub sidebar: gtk::Box,
    pub keymap_overlay_widget: gtk::Fixed,
    pub drop_overlay: gtk::Box,
    pub display_widget: gtk::Overlay,
    pub input_area: gtk::DrawingArea,
    pub state: Rc<UiState>,
}

impl NuxWindow {
    /// Create the main window and wire up all actions and controllers.
    ///
    /// Returns the `AdwApplicationWindow` ready to present. The `NuxWindow`
    /// wrapper is kept alive internally via `Rc` prevent pointers in closures.
    pub fn build(app: &adw::Application) -> adw::ApplicationWindow {
        let state = Rc::new(UiState::default());

        // ── Header bar ───────────────────────────────────────────
        let status_label = gtk::Label::builder()
            .label("Stopped")
            .css_classes(["dim-label"])
            .build();

        let fps_label = gtk::Label::builder()
            .label("0 FPS")
            .visible(false)
            .css_classes(["dim-label"])
            .build();

        let menu_model = build_primary_menu();
        let menu_button = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu_model)
            .tooltip_text("Main Menu")
            .build();

        let header_bar = adw::HeaderBar::new();
        header_bar.pack_start(&status_label);
        header_bar.pack_end(&menu_button);
        header_bar.pack_end(&fps_label);

        // ── Display area ─────────────────────────────────────────
        let (display_widget, input_area) = display::build_display();
        let keymap_overlay_widget = overlay::build_keymap_overlay();

        // Add keymap overlay on top of the display
        display_widget.add_overlay(&keymap_overlay_widget);

        // ── APK drop overlay (visual feedback) ───────────────────
        let drop_overlay = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .spacing(12)
            .visible(false)
            .css_classes(["card", "osd"])
            .build();
        let drop_icon = gtk::Image::builder()
            .icon_name("document-save-symbolic")
            .pixel_size(64)
            .build();
        let drop_label = gtk::Label::builder()
            .label("Drop APK to install")
            .css_classes(["title-2"])
            .build();
        drop_overlay.append(&drop_icon);
        drop_overlay.append(&drop_label);
        display_widget.add_overlay(&drop_overlay);

        // ── Sidebar toolbar ──────────────────────────────────────
        let sidebar = toolbar::build_toolbar();

        // ── Main layout ──────────────────────────────────────────
        let hbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .build();
        hbox.append(&display_widget);
        hbox.append(&sidebar);

        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&hbox));

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        content.append(&header_bar);
        content.append(&toast_overlay);

        // ── Window ───────────────────────────────────────────────
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Nux Emulator")
            .default_width(1024)
            .default_height(768)
            .width_request(800)
            .height_request(600)
            .content(&content)
            .build();

        // Store widget refs for action callbacks
        let nux = Rc::new(NuxWindow {
            window: window.clone(),
            toast_overlay,
            header_bar,
            status_label,
            fps_label,
            sidebar,
            keymap_overlay_widget,
            drop_overlay,
            display_widget,
            input_area,
            state,
        });

        register_window_actions(&nux);
        setup_drag_and_drop(&nux);
        setup_fullscreen_hover(&nux);
        setup_fps_binding(&nux, app);
        setup_close_handler(&nux);
        set_vm_action_sensitivity(&nux, false);

        window
    }
}

// ── Window actions ───────────────────────────────────────────────

#[allow(clippy::too_many_lines)]
fn register_window_actions(nux: &Rc<NuxWindow>) {
    use gtk::gio::SimpleAction;

    let win = &nux.window;

    // Toggle fullscreen
    let toggle_fs = SimpleAction::new("toggle-fullscreen", None);
    toggle_fs.connect_activate(glib::clone!(
        #[strong]
        nux,
        move |_, _| {
            toggle_fullscreen(&nux);
        }
    ));
    win.add_action(&toggle_fs);

    // Escape exits fullscreen
    let esc_ctrl = gtk::EventControllerKey::new();
    esc_ctrl.connect_key_pressed(glib::clone!(
        #[strong]
        nux,
        move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape && nux.state.fullscreen.get() {
                toggle_fullscreen(&nux);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    ));
    win.add_controller(esc_ctrl);

    // Screenshot
    let screenshot = SimpleAction::new("screenshot", None);
    screenshot.connect_activate(glib::clone!(
        #[strong]
        nux,
        move |_, _| {
            let launcher = nux.state.launcher.clone();
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
            let path = std::path::PathBuf::from(home).join("nux-screenshot.png");
            match launcher.screenshot(&path) {
                Ok(()) => {
                    log::info!("screenshot saved to {}", path.display());
                    nux.toast_overlay.add_toast(adw::Toast::new(&format!(
                        "Screenshot saved to {}",
                        path.display()
                    )));
                }
                Err(e) => {
                    nux.toast_overlay
                        .add_toast(adw::Toast::new(&format!("Screenshot failed: {e}")));
                }
            }
        }
    ));
    win.add_action(&screenshot);

    // Volume up
    let vol_up = SimpleAction::new("volume-up", None);
    vol_up.connect_activate(glib::clone!(
        #[strong]
        nux,
        move |_, _| {
            nux.state.launcher.volume_up();
        }
    ));
    win.add_action(&vol_up);

    // Volume down
    let vol_down = SimpleAction::new("volume-down", None);
    vol_down.connect_activate(glib::clone!(
        #[strong]
        nux,
        move |_, _| {
            nux.state.launcher.volume_down();
        }
    ));
    win.add_action(&vol_down);

    // Shake (stub)
    let shake = SimpleAction::new("shake", None);
    shake.connect_activate(|_, _| {
        log::info!("shake action triggered");
    });
    win.add_action(&shake);

    // Rotate (stub)
    let rotate = SimpleAction::new("rotate", None);
    rotate.connect_activate(|_, _| {
        log::info!("rotate action triggered");
    });
    win.add_action(&rotate);

    // ── Start VM ──
    let start_vm = SimpleAction::new("start-vm", None);
    start_vm.connect_activate(glib::clone!(
        #[strong]
        nux,
        move |_, _| {
            if nux.state.vm_running.get() {
                nux.toast_overlay
                    .add_toast(adw::Toast::new("VM is already running"));
                return;
            }

            nux.status_label.set_label("Starting...");
            nux.toast_overlay
                .add_toast(adw::Toast::new("Starting VM..."));

            let launcher = nux.state.launcher.clone();
            let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
            let (wl_tx, wl_rx) = std::sync::mpsc::channel::<(
                std::sync::Arc<crate::wayland_compositor::FrameSlot>,
                crate::wayland_compositor::WaylandInput,
            )>();

            std::thread::spawn(move || {
                let frames_sock = "/tmp/cf_avd_0/cvd-1/internal/frames.sock";

                // Start launch_cvd normally — don't interfere with socket creation
                let result = launcher.start();

                // After launch_cvd starts, watch for crosvm process.
                // Crosvm takes ~2s to init gfxstream before connecting to Wayland.
                // In that window: kill webRTC, replace socket with ours.
                if result.is_ok() {
                    // Wait for crosvm to appear (up to 60s)
                    let mut found = false;
                    for _ in 0..600 {
                        let out = std::process::Command::new("pgrep")
                            .args(["-f", "crosvm.*crosvm_control"])
                            .output();
                        if let Ok(o) = out {
                            if o.status.success() && !o.stdout.is_empty() {
                                found = true;
                                break;
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }

                    if found {
                        log::info!("vm: crosvm detected, swapping Wayland socket");

                        // Kill webRTC immediately (it holds the old socket fd)
                        let _ = std::process::Command::new("sudo")
                            .args(["pkill", "-9", "-f", "webRTC"])
                            .output();

                        // Make the directory writable so we can bind our socket as non-root
                        let _ = std::process::Command::new("sudo")
                            .args(["chmod", "777", "/tmp/cf_avd_0/cvd-1/internal"])
                            .output();

                        // Remove the old socket and bind ours
                        let _ = std::fs::remove_file(frames_sock);
                        std::thread::sleep(std::time::Duration::from_millis(50));

                        match crate::wayland_compositor::start_compositor_at_path(frames_sock) {
                            Ok((frame_rx, wayland_input)) => {
                                log::info!("vm: Wayland compositor bound at {frames_sock}");
                                let _ = wl_tx.send((frame_rx, wayland_input));
                            }
                            Err(e) => {
                                log::error!("vm: Wayland compositor failed: {e}");
                            }
                        }
                    } else {
                        log::error!("vm: crosvm not detected after 60s");
                    }
                }

                let _ = tx.send(result);
            });

            // Poll for result on UI thread
            let nux_clone = nux.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
                // Check if Wayland compositor is ready
                if let Ok((frame_slot, wayland_input)) = wl_rx.try_recv() {
                    *nux_clone.state.wayland_frame_slot.borrow_mut() = Some(frame_slot);
                    *nux_clone.state.wayland_input.borrow_mut() = Some(wayland_input);
                }

                match rx.try_recv() {
                    Ok(Ok(())) => {
                        nux_clone.state.vm_running.set(true);
                        nux_clone.status_label.set_label("Booting...");
                        set_vm_action_sensitivity(&nux_clone, true);
                        start_boot_monitor(&nux_clone);
                        glib::ControlFlow::Break
                    }
                    Ok(Err(e)) => {
                        nux_clone.status_label.set_label("Failed");
                        nux_clone
                            .toast_overlay
                            .add_toast(adw::Toast::new(&format!("VM start failed: {e}")));
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        nux_clone.status_label.set_label("Failed");
                        glib::ControlFlow::Break
                    }
                }
            });
        }
    ));
    win.add_action(&start_vm);

    // ── Stop VM ──
    let stop_vm = SimpleAction::new("stop-vm", None);
    stop_vm.connect_activate(glib::clone!(
        #[strong]
        nux,
        move |_, _| {
            if !nux.state.vm_running.get() {
                nux.toast_overlay
                    .add_toast(adw::Toast::new("VM is not running"));
                return;
            }

            nux.status_label.set_label("Stopping...");
            let launcher = nux.state.launcher.clone();
            let _ = launcher.stop();
            nux.state.vm_running.set(false);
            nux.state.vm_booted.set(false);
            nux.status_label.set_label("Stopped");
            // Stop scrcpy display
            if let Some(handle) = nux.state.scrcpy.borrow().as_ref() {
                display::stop_scrcpy(&nux.display_widget, handle);
            }
            *nux.state.scrcpy.borrow_mut() = None;
            display::show_stopped(&nux.display_widget);
            set_vm_action_sensitivity(&nux, false);
            nux.toast_overlay.add_toast(adw::Toast::new("VM stopped"));
        }
    ));
    win.add_action(&stop_vm);

    // Install APK via file chooser
    let install_apk = SimpleAction::new("install-apk", None);
    install_apk.connect_activate(glib::clone!(
        #[strong]
        nux,
        move |_, _| {
            if !nux.state.vm_booted.get() {
                nux.toast_overlay
                    .add_toast(adw::Toast::new("VM must be booted to install APKs"));
                return;
            }
            open_apk_file_chooser(&nux);
        }
    ));
    win.add_action(&install_apk);

    // Toggle keymap overlay
    let toggle_keymap = SimpleAction::new("toggle-keymap-overlay", None);
    toggle_keymap.connect_activate(glib::clone!(
        #[strong]
        nux,
        move |_, _| {
            let vis = nux.keymap_overlay_widget.is_visible();
            if !vis {
                // Load keymap and render hints
                let engine = nux_core::keymap::KeymapEngine::new();
                let keymap_path = std::path::Path::new("keymaps/generic-moba.toml");
                if keymap_path.exists() {
                    if let Err(e) = engine.load_file(keymap_path, (720, 1280)) {
                        log::error!("keymap: load failed: {e}");
                    } else {
                        let hints = engine.overlay_hints();
                        let w = nux.display_widget.child().map(|c| c.width()).unwrap_or(720);
                        let h = nux
                            .display_widget
                            .child()
                            .map(|c| c.height())
                            .unwrap_or(1280);
                        overlay::update_overlay(
                            &nux.keymap_overlay_widget,
                            &hints,
                            w,
                            h,
                            720,
                            1280,
                        );
                        log::info!("keymap: loaded {} hints", hints.len());
                    }
                } else {
                    log::warn!("keymap: no keymap file found at {}", keymap_path.display());
                }
            }
            nux.keymap_overlay_widget.set_visible(!vis);
        }
    ));
    win.add_action(&toggle_keymap);

    // Open settings
    let open_settings = SimpleAction::new("open-settings", None);
    open_settings.connect_activate(glib::clone!(
        #[strong]
        nux,
        move |_, _| {
            let dialog = settings::build_settings_window(&nux.window);
            dialog.present();
        }
    ));
    win.add_action(&open_settings);
}

// ── Fullscreen ───────────────────────────────────────────────────

fn toggle_fullscreen(nux: &NuxWindow) {
    if nux.state.fullscreen.get() {
        // Exit fullscreen
        nux.window.unfullscreen();
        nux.header_bar.set_visible(true);
        nux.sidebar.set_visible(true);
        nux.state.fullscreen.set(false);
    } else {
        // Save pre-fullscreen dimensions
        let (w, h) = (nux.window.width(), nux.window.height());
        nux.state.pre_fs_width.set(w);
        nux.state.pre_fs_height.set(h);

        nux.window.fullscreen();
        nux.header_bar.set_visible(false);
        nux.sidebar.set_visible(false);
        nux.state.fullscreen.set(true);
    }
}

fn setup_fullscreen_hover(nux: &Rc<NuxWindow>) {
    let motion = gtk::EventControllerMotion::new();
    motion.connect_motion(glib::clone!(
        #[strong]
        nux,
        move |_, x, _y| {
            if !nux.state.fullscreen.get() {
                return;
            }
            let width = f64::from(nux.window.width());
            // Reveal sidebar when mouse is within 48px of right edge
            if x > width - 48.0 {
                nux.sidebar.set_visible(true);
            } else {
                nux.sidebar.set_visible(false);
            }
        }
    ));
    nux.window.add_controller(motion);
}

// ── FPS label binding ────────────────────────────────────────────

fn setup_fps_binding(nux: &Rc<NuxWindow>, app: &adw::Application) {
    if let Some(action) = app.lookup_action("toggle-fps") {
        action.connect_state_notify(glib::clone!(
            #[strong]
            nux,
            move |action| {
                let visible = action
                    .state()
                    .and_then(|v| v.get::<bool>())
                    .unwrap_or(false);
                nux.fps_label.set_visible(visible);
            }
        ));
    }
}

// ── APK drag-and-drop ────────────────────────────────────────────

fn setup_drag_and_drop(nux: &Rc<NuxWindow>) {
    let drop_target =
        gtk::DropTarget::new(gtk::gio::File::static_type(), gtk::gdk::DragAction::COPY);

    // Visual feedback on drag enter/motion
    drop_target.connect_enter(glib::clone!(
        #[strong]
        nux,
        move |_, _, _| {
            nux.drop_overlay.set_visible(true);
            gtk::gdk::DragAction::COPY
        }
    ));

    drop_target.connect_leave(glib::clone!(
        #[strong]
        nux,
        move |_| {
            nux.drop_overlay.set_visible(false);
        }
    ));

    drop_target.connect_drop(glib::clone!(
        #[strong]
        nux,
        move |_, value, _, _| {
            nux.drop_overlay.set_visible(false);

            if !nux.state.vm_running.get() {
                nux.toast_overlay
                    .add_toast(adw::Toast::new("VM must be running to install APKs"));
                return false;
            }

            let file: gtk::gio::File = match value.get() {
                Ok(f) => f,
                Err(_) => return false,
            };

            let Some(path) = file.path() else {
                return false;
            };

            if path.extension().is_some_and(|ext| ext == "apk") {
                log::info!("APK dropped: {}", path.display());
                nux.toast_overlay.add_toast(adw::Toast::new(&format!(
                    "Installing {}...",
                    path.file_name().and_then(|n| n.to_str()).unwrap_or("APK")
                )));
                // Actual install via nux-core::adb will be wired in integration phase
                true
            } else {
                nux.toast_overlay
                    .add_toast(adw::Toast::new("Only APK files can be installed"));
                false
            }
        }
    ));

    nux.window.add_controller(drop_target);
}

// ── VM-state action sensitivity ──────────────────────────────────

fn set_vm_action_sensitivity(nux: &NuxWindow, running: bool) {
    let vm_actions = [
        "screenshot",
        "volume-up",
        "volume-down",
        "shake",
        "rotate",
        "install-apk",
    ];
    for name in vm_actions {
        if let Some(action) = nux.window.lookup_action(name) {
            if let Some(simple) = action.downcast_ref::<gtk::gio::SimpleAction>() {
                simple.set_enabled(running);
            }
        }
    }
}

// ── Boot monitor ─────────────────────────────────────────────────

fn start_boot_monitor(nux: &Rc<NuxWindow>) {
    let nux_clone = nux.clone();
    glib::timeout_add_seconds_local(3, move || {
        if !nux_clone.state.vm_running.get() {
            return glib::ControlFlow::Break;
        }

        let launcher = nux_clone.state.launcher.clone();
        let status = launcher.check_boot_status();

        match status {
            BootStatus::Booted => {
                if !nux_clone.state.vm_booted.get() {
                    nux_clone.state.vm_booted.set(true);
                    nux_clone.status_label.set_label("Running");
                    nux_clone
                        .toast_overlay
                        .add_toast(adw::Toast::new("Android booted!"));

                    // Use Wayland compositor for native display
                    let display_handle = if let Some(frame_slot) =
                        nux_clone.state.wayland_frame_slot.borrow_mut().take()
                    {
                        let wayland_input = nux_clone.state.wayland_input.borrow_mut().take();
                        if let Some(wl_input) = wayland_input {
                            log::info!("display: using native Wayland compositor");
                            nux_clone
                                .toast_overlay
                                .add_toast(adw::Toast::new("Native display connected"));
                            display::start_wayland_display(
                                &nux_clone.display_widget,
                                &nux_clone.input_area,
                                &nux_clone.window,
                                frame_slot,
                                wl_input,
                            )
                        } else {
                            log::error!("display: no Wayland input handle");
                            return glib::ControlFlow::Continue;
                        }
                    } else {
                        log::error!("display: no Wayland frame receiver available");
                        nux_clone
                            .toast_overlay
                            .add_toast(adw::Toast::new("Display connection failed"));
                        return glib::ControlFlow::Continue;
                    };
                    *nux_clone.state.scrcpy.borrow_mut() = Some(display_handle);

                    // Enable WiFi in background
                    let launcher2 = launcher.clone();
                    std::thread::spawn(move || {
                        let _ = launcher2.enable_wifi();
                    });
                }
                glib::ControlFlow::Continue
            }
            BootStatus::Booting => {
                nux_clone.status_label.set_label("Booting...");
                glib::ControlFlow::Continue
            }
            BootStatus::NotConnected => {
                if !nux_clone.state.launcher.is_running() {
                    nux_clone.state.vm_running.set(false);
                    nux_clone.state.vm_booted.set(false);
                    nux_clone.status_label.set_label("Stopped");
                    set_vm_action_sensitivity(&nux_clone, false);
                    return glib::ControlFlow::Break;
                }
                nux_clone.status_label.set_label("Starting...");
                glib::ControlFlow::Continue
            }
        }
    });
}

// ── APK file chooser ─────────────────────────────────────────────

fn open_apk_file_chooser(nux: &Rc<NuxWindow>) {
    let filter = gtk::FileFilter::new();
    filter.add_pattern("*.apk");
    filter.set_name(Some("Android APK"));

    let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);

    let dialog = gtk::FileDialog::builder()
        .title("Select APK to Install")
        .filters(&filters)
        .modal(true)
        .build();

    dialog.open(
        Some(&nux.window),
        gtk::gio::Cancellable::NONE,
        glib::clone!(
            #[strong]
            nux,
            move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        let filename = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("APK")
                            .to_owned();
                        log::info!("APK selected: {}", path.display());
                        nux.toast_overlay
                            .add_toast(adw::Toast::new(&format!("Installing {filename}...")));
                        nux.state.apk_installing.set(true);

                        let launcher = nux.state.launcher.clone();
                        let path_clone = path.clone();
                        let filename_clone = filename.clone();

                        let (tx, rx) =
                            std::sync::mpsc::channel::<(String, Result<String, String>)>();

                        std::thread::spawn(move || {
                            let result = launcher.install_apk(&path_clone);
                            let _ = tx.send((filename_clone, result));
                        });

                        let nux_clone = nux.clone();
                        glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
                            match rx.try_recv() {
                                Ok((name, result)) => {
                                    nux_clone.state.apk_installing.set(false);
                                    match result {
                                        Ok(msg) => {
                                            nux_clone.toast_overlay.add_toast(adw::Toast::new(
                                                &format!("{name}: {msg}"),
                                            ));
                                        }
                                        Err(e) => {
                                            nux_clone.toast_overlay.add_toast(adw::Toast::new(
                                                &format!("{name}: {e}"),
                                            ));
                                        }
                                    }
                                    glib::ControlFlow::Break
                                }
                                Err(std::sync::mpsc::TryRecvError::Empty) => {
                                    glib::ControlFlow::Continue
                                }
                                Err(_) => {
                                    nux_clone.state.apk_installing.set(false);
                                    glib::ControlFlow::Break
                                }
                            }
                        });
                    }
                }
            }
        ),
    );
}

// ── Primary menu ─────────────────────────────────────────────────

fn build_primary_menu() -> gtk::gio::Menu {
    let menu = gtk::gio::Menu::new();
    menu.append(Some("_About Nux Emulator"), Some("app.about"));
    menu.append(Some("_Quit"), Some("app.quit"));
    menu
}

// ── Close handler (graceful shutdown + state persistence) ────────

fn setup_close_handler(nux: &Rc<NuxWindow>) {
    nux.window.connect_close_request(glib::clone!(
        #[strong]
        nux,
        move |_win| {
            // Save window state
            save_window_state(&nux);

            if nux.state.apk_installing.get() {
                // Show confirmation dialog
                let dialog = adw::AlertDialog::builder()
                    .heading("APK Install in Progress")
                    .body(
                        "An APK is currently being installed. \
                         Are you sure you want to quit?",
                    )
                    .build();
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("quit", "Quit Anyway");
                dialog.set_response_appearance("quit", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");

                let window = nux.window.clone();
                dialog.connect_response(None, move |_, response| {
                    if response == "quit" {
                        window.destroy();
                    }
                });
                dialog.present(Some(&nux.window));
                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        }
    ));
}

// ── Window state persistence ─────────────────────────────────────

/// Placeholder for saving window geometry to nux-core config.
///
/// In the integration phase this writes to `[ui.window]` in the TOML
/// config via `nux_core::config::save`.
fn save_window_state(nux: &NuxWindow) {
    let (width, height) = if nux.state.fullscreen.get() {
        (nux.state.pre_fs_width.get(), nux.state.pre_fs_height.get())
    } else {
        (nux.window.width(), nux.window.height())
    };
    let maximized = nux.window.is_maximized();
    log::info!("saving window state: {width}x{height}, maximized={maximized}",);
    // Will write to nux_core::config in integration phase
}
