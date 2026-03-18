# Nux Emulator

A gaming-focused Android emulator for Linux with a polished GNOME-native UI.

Nux runs Android apps and games on Linux desktops using KVM hardware acceleration (via crosvm) and GPU passthrough (via gfxstream), delivering near-native performance. The interface is built with GTK4 and libadwaita for a seamless GNOME experience.

## Feature Highlights

- **KVM-accelerated** — Full hardware virtualization via crosvm for near-native CPU performance
- **GPU passthrough** — gfxstream backend with support for NVIDIA, AMD, and Intel GPUs
- **GNOME-native UI** — GTK4 + libadwaita interface, Wayland-first with full X11 support
- **Android 16 (AOSP)** — Modern Android base with x86_64 architecture
- **Root support** — Boot image patching via Magisk, KernelSU, or APatch
- **MicroG by default** — Privacy-respecting Google services replacement, with optional GApps

## Building

### Prerequisites

- **Rust** 1.85.0+ (install via [rustup](https://rustup.rs/))
- **GTK4** 4.16+ development libraries
- **libadwaita** 1.6+ development libraries

#### Fedora

```bash
sudo dnf install gtk4-devel libadwaita-devel
```

#### Ubuntu 24.04+

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev
```

#### Arch Linux

```bash
sudo pacman -S gtk4 libadwaita
```

### Build

```bash
cargo build --workspace
```

### Run

```bash
cargo run -p nux-ui
```

## Project Structure

```
nux-emulator/
├── Cargo.toml              # Workspace root
├── nux-core/               # Core library (VM, input, config, networking)
│   ├── Cargo.toml
│   └── src/lib.rs
├── nux-ui/                 # GTK4 + libadwaita frontend
│   ├── Cargo.toml
│   └── src/main.rs
├── keymaps/                # Keymap configuration files
├── scripts/                # Development helper scripts
├── .github/workflows/      # CI/CD workflows
├── LICENSE                 # GPLv3
└── README.md
```

## License

Nux Emulator is licensed under the [GNU General Public License v3.0](LICENSE).
