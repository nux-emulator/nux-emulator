//! Settings dialog — `AdwPreferencesWindow` with per-category pages.

use adw::prelude::*;
use gtk::glib;
use gtk4 as gtk;
use libadwaita as adw;

/// Build and return the settings `AdwPreferencesWindow`.
///
/// The window is modal relative to the given parent. Each settings
/// category is an `AdwPreferencesPage`. Values are read from / written
/// to `nux-core::config` in the integration phase; for now the widgets
/// are wired with sensible defaults.
pub fn build_settings_window(parent: &impl IsA<gtk::Window>) -> adw::PreferencesWindow {
    let win = adw::PreferencesWindow::builder()
        .title("Settings")
        .modal(true)
        .transient_for(parent)
        .default_width(600)
        .default_height(500)
        .build();

    win.add(&build_performance_page(&win));
    win.add(&build_display_page(&win));
    win.add(&build_device_page());
    win.add(&build_root_page());
    win.add(&build_google_services_page(&win));
    win.add(&build_about_page());

    win
}

/// Show a restart-required toast on the preferences window.
fn show_restart_toast(win: &adw::PreferencesWindow) {
    let toast = adw::Toast::new("VM restart required for changes to take effect");
    toast.set_timeout(3);
    win.add_toast(toast);
}

// ── Performance page ─────────────────────────────────────────────

fn build_performance_page(win: &adw::PreferencesWindow) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Performance")
        .icon_name("speedometer-symbolic")
        .build();

    let group = adw::PreferencesGroup::builder()
        .title("Hardware")
        .description("Changes require a VM restart")
        .build();

    // CPU cores
    #[allow(clippy::cast_precision_loss)]
    let host_cores = std::thread::available_parallelism()
        .map(|n| n.get() as f64)
        .unwrap_or(8.0);
    let cpu_adj = gtk::Adjustment::new(2.0, 1.0, host_cores, 1.0, 1.0, 0.0);
    let cpu_row = adw::SpinRow::new(Some(&cpu_adj), 1.0, 0);
    cpu_row.set_title("CPU Cores");
    cpu_row.set_subtitle("Number of virtual CPU cores");
    cpu_row.connect_value_notify(glib::clone!(
        #[weak]
        win,
        move |_| {
            show_restart_toast(&win);
        }
    ));
    group.add(&cpu_row);

    // RAM
    let ram_row = adw::ComboRow::builder()
        .title("RAM")
        .subtitle("Memory allocated to the VM")
        .model(&gtk::StringList::new(&["2 GB", "4 GB", "6 GB", "8 GB"]))
        .build();
    ram_row.connect_selected_notify(glib::clone!(
        #[weak]
        win,
        move |_| {
            show_restart_toast(&win);
        }
    ));
    group.add(&ram_row);

    // DPI
    let dpi_adj = gtk::Adjustment::new(320.0, 120.0, 640.0, 20.0, 40.0, 0.0);
    let dpi_row = adw::SpinRow::new(Some(&dpi_adj), 20.0, 0);
    dpi_row.set_title("DPI");
    dpi_row.set_subtitle("Display density (120–640)");
    dpi_row.connect_value_notify(glib::clone!(
        #[weak]
        win,
        move |_| {
            show_restart_toast(&win);
        }
    ));
    group.add(&dpi_row);

    page.add(&group);
    page
}

// ── Display page ─────────────────────────────────────────────────

fn build_display_page(win: &adw::PreferencesWindow) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Display")
        .icon_name("video-display-symbolic")
        .build();

    let group = adw::PreferencesGroup::builder().title("Resolution").build();

    let res_row = adw::ComboRow::builder()
        .title("Resolution Preset")
        .model(&gtk::StringList::new(&[
            "720 × 1280",
            "1080 × 1920",
            "1440 × 2560",
            "Custom",
        ]))
        .selected(1)
        .build();

    // Custom width/height rows (shown only when "Custom" is selected)
    let width_adj = gtk::Adjustment::new(1080.0, 320.0, 3840.0, 1.0, 10.0, 0.0);
    let width_row = adw::SpinRow::new(Some(&width_adj), 1.0, 0);
    width_row.set_title("Width");
    width_row.set_visible(false);

    let height_adj = gtk::Adjustment::new(1920.0, 480.0, 2160.0, 1.0, 10.0, 0.0);
    let height_row = adw::SpinRow::new(Some(&height_adj), 1.0, 0);
    height_row.set_title("Height");
    height_row.set_visible(false);

    res_row.connect_selected_notify(glib::clone!(
        #[weak]
        win,
        #[weak]
        width_row,
        #[weak]
        height_row,
        move |row| {
            let is_custom = row.selected() == 3;
            width_row.set_visible(is_custom);
            height_row.set_visible(is_custom);
            show_restart_toast(&win);
        }
    ));

    group.add(&res_row);
    group.add(&width_row);
    group.add(&height_row);
    page.add(&group);
    page
}

// ── Device page ──────────────────────────────────────────────────

fn build_device_page() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Device")
        .icon_name("phone-symbolic")
        .build();

    let group = adw::PreferencesGroup::builder()
        .title("Device Identity")
        .build();

    let model_row = adw::ComboRow::builder()
        .title("Device Model")
        .model(&gtk::StringList::new(&["Pixel 9", "Samsung S24", "Custom"]))
        .build();
    group.add(&model_row);

    let orient_row = adw::ComboRow::builder()
        .title("Default Orientation")
        .model(&gtk::StringList::new(&["Portrait", "Landscape"]))
        .build();
    group.add(&orient_row);

    page.add(&group);
    page
}

// ── Root page ────────────────────────────────────────────────────

fn build_root_page() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Root")
        .icon_name("security-high-symbolic")
        .build();

    let group = adw::PreferencesGroup::builder()
        .title("Root Manager")
        .build();

    let mode_row = adw::ComboRow::builder()
        .title("Root Mode")
        .model(&gtk::StringList::new(&[
            "None", "Magisk", "KernelSU", "APatch",
        ]))
        .build();
    group.add(&mode_row);

    let status_row = adw::ActionRow::builder()
        .title("Boot Image Status")
        .subtitle("Unpatched")
        .build();
    group.add(&status_row);

    page.add(&group);
    page
}

// ── Google Services page ─────────────────────────────────────────

fn build_google_services_page(win: &adw::PreferencesWindow) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Google Services")
        .icon_name("network-server-symbolic")
        .build();

    let group = adw::PreferencesGroup::builder().title("Provider").build();

    let provider_row = adw::ComboRow::builder()
        .title("Google Services Provider")
        .model(&gtk::StringList::new(&["MicroG", "GApps", "None"]))
        .build();
    provider_row.connect_selected_notify(glib::clone!(
        #[weak]
        win,
        move |_| {
            show_restart_toast(&win);
        }
    ));
    group.add(&provider_row);

    page.add(&group);
    page
}

// ── About page ───────────────────────────────────────────────────

fn build_about_page() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("About")
        .icon_name("help-about-symbolic")
        .build();

    let group = adw::PreferencesGroup::builder().build();

    let name_row = adw::ActionRow::builder()
        .title("Nux Emulator")
        .subtitle(concat!("Version ", env!("CARGO_PKG_VERSION")))
        .build();
    group.add(&name_row);

    let license_row = adw::ActionRow::builder()
        .title("License")
        .subtitle("GNU General Public License v3.0")
        .build();
    group.add(&license_row);

    let repo_row = adw::ActionRow::builder()
        .title("Source Code")
        .subtitle("https://github.com/nux-emulator/nux")
        .activatable(true)
        .build();
    repo_row.connect_activated(|_| {
        let _ = gtk::gio::AppInfo::launch_default_for_uri(
            "https://github.com/nux-emulator/nux",
            gtk::gio::AppLaunchContext::NONE,
        );
    });
    group.add(&repo_row);

    let issues_row = adw::ActionRow::builder()
        .title("Report an Issue")
        .subtitle("https://github.com/nux-emulator/nux/issues")
        .activatable(true)
        .build();
    issues_row.connect_activated(|_| {
        let _ = gtk::gio::AppInfo::launch_default_for_uri(
            "https://github.com/nux-emulator/nux/issues",
            gtk::gio::AppLaunchContext::NONE,
        );
    });
    group.add(&issues_row);

    page.add(&group);
    page
}
