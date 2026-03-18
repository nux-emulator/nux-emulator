//! nux-ui — GTK4 + libadwaita frontend for the Nux Android emulator.

mod display;
mod overlay;
mod settings;
mod state;
mod toolbar;
pub mod vm_launcher;
mod window;

use adw::prelude::*;
use gtk::glib;
use gtk4 as gtk;
use libadwaita as adw;

use window::NuxWindow;

const APP_ID: &str = "io.github.nux-emulator";

#[allow(unsafe_code)]
fn main() {
    // Force X11 backend so we can embed scrcpy's X11 window
    unsafe {
        std::env::set_var("GDK_BACKEND", "x11");
    }

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gtk::gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_activate(on_activate);
    app.connect_open(on_open);

    // Register app-level actions before run
    register_app_actions(&app);

    app.run();
}

fn on_activate(app: &adw::Application) {
    if let Some(win) = app.active_window() {
        win.present();
        return;
    }
    let window = NuxWindow::build(app);
    window.present();
}

fn on_open(app: &adw::Application, files: &[gtk::gio::File], _hint: &str) {
    on_activate(app);

    // Queue APK installs for any .apk files passed as arguments
    for file in files {
        if let Some(path) = file.path() {
            if path.extension().is_some_and(|ext| ext == "apk") {
                log::info!("APK file argument: {}", path.display());
                // APK install will be wired to nux-core::adb in integration phase
            }
        }
    }
}

fn register_app_actions(app: &adw::Application) {
    use gtk::gio::SimpleAction;

    // Quit
    let quit = SimpleAction::new("quit", None);
    quit.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            app.quit();
        }
    ));
    app.add_action(&quit);
    app.set_accels_for_action("app.quit", &["<Ctrl>q"]);

    // Toggle FPS
    let toggle_fps = SimpleAction::new_stateful("toggle-fps", None, &false.to_variant());
    toggle_fps.connect_activate(|action, _| {
        let current = action
            .state()
            .and_then(|v| v.get::<bool>())
            .unwrap_or(false);
        action.set_state(&(!current).to_variant());
    });
    app.add_action(&toggle_fps);

    // About
    let about = SimpleAction::new("about", None);
    about.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            let dialog = adw::AboutDialog::builder()
                .application_name("Nux Emulator")
                .application_icon("applications-games")
                .version(env!("CARGO_PKG_VERSION"))
                .developer_name("Nux Emulator Contributors")
                .license_type(gtk::License::Gpl30)
                .website("https://github.com/nux-emulator/nux")
                .issue_url("https://github.com/nux-emulator/nux/issues")
                .build();
            if let Some(win) = app.active_window() {
                dialog.present(Some(&win));
            }
        }
    ));
    app.add_action(&about);

    // Fullscreen (app-level accelerators, action lives on window)
    app.set_accels_for_action("win.toggle-fullscreen", &["<Ctrl>f", "F11"]);
    app.set_accels_for_action("win.open-settings", &["<Ctrl>comma"]);
}
