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

/// Wrapper around the main application window and its key child widgets.
pub struct NuxWindow {
    pub window: adw::ApplicationWindow,
    pub toast_overlay: adw::ToastOverlay,
    pub header_bar: adw::HeaderBar,
    pub _status_label: gtk::Label,
    pub fps_label: gtk::Label,
    pub sidebar: gtk::Box,
    pub keymap_overlay_widget: gtk::Box,
    pub drop_overlay: gtk::Box,
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
        let gl_area = display::build_display();
        let keymap_overlay_widget = overlay::build_keymap_overlay();

        let display_overlay = gtk::Overlay::builder()
            .hexpand(true)
            .vexpand(true)
            .child(&gl_area)
            .build();
        display_overlay.add_overlay(&keymap_overlay_widget);

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
        display_overlay.add_overlay(&drop_overlay);

        // ── Sidebar toolbar ──────────────────────────────────────
        let sidebar = toolbar::build_toolbar();

        // ── Main layout ──────────────────────────────────────────
        let hbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .build();
        hbox.append(&display_overlay);
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
            _status_label: status_label,
            fps_label,
            sidebar,
            keymap_overlay_widget,
            drop_overlay,
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

    // Screenshot (stub)
    let screenshot = SimpleAction::new("screenshot", None);
    screenshot.connect_activate(glib::clone!(
        #[strong]
        nux,
        move |_, _| {
            log::info!("screenshot action triggered");
            nux.toast_overlay
                .add_toast(adw::Toast::new("Screenshot saved"));
        }
    ));
    win.add_action(&screenshot);

    // Volume up (stub)
    let vol_up = SimpleAction::new("volume-up", None);
    vol_up.connect_activate(|_, _| {
        log::info!("volume-up action triggered");
    });
    win.add_action(&vol_up);

    // Volume down (stub)
    let vol_down = SimpleAction::new("volume-down", None);
    vol_down.connect_activate(|_, _| {
        log::info!("volume-down action triggered");
    });
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

    // Install APK via file chooser
    let install_apk = SimpleAction::new("install-apk", None);
    install_apk.connect_activate(glib::clone!(
        #[strong]
        nux,
        move |_, _| {
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
    let vm_actions = ["screenshot", "volume-up", "volume-down", "shake", "rotate"];
    for name in vm_actions {
        if let Some(action) = nux.window.lookup_action(name) {
            if let Some(simple) = action.downcast_ref::<gtk::gio::SimpleAction>() {
                simple.set_enabled(running);
            }
        }
    }
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
                        log::info!("APK selected: {}", path.display());
                        nux.toast_overlay.add_toast(adw::Toast::new(&format!(
                            "Installing {}...",
                            path.file_name().and_then(|n| n.to_str()).unwrap_or("APK")
                        )));
                        // Actual install via nux-core::adb in integration phase
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
