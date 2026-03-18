## Why

Android gaming on Linux often requires Google Play Services for game authentication, in-app purchases, and cloud saves. Nux ships MicroG pre-installed as a lightweight open-source alternative, but some games strictly require official GApps. Users need a clear way to switch between MicroG, full GApps, or a clean AOSP experience — without manually patching system images or running shell commands.

## What Changes

- Introduce `nux-core::gservices` module managing the Google Services provider lifecycle
- Support three provider modes: MicroG (default, pre-installed), GApps (user-initiated flash), and None (pure AOSP)
- Detect the currently active provider and its version by querying the Android guest via ADB
- Download OpenGApps or MindTheGapps packages from known URLs with integrity verification
- Flash GApps into the system image via overlay or direct modification, replacing MicroG
- Remove both MicroG and GApps when switching to None mode
- Persist the selected provider in `GoogleServicesConfig` via the config system
- Signal that a VM restart is required after any provider switch
- Expose provider status (current provider, version, pending restart) for UI consumption

## Non-goals

- Building MicroG from source — it ships pre-built in the Android image
- Google account management or sign-in flows
- Play Store alternatives (F-Droid, Aurora Store, etc.)
- Automatic GApps updates after initial flash
- Modifying the base AOSP system image at build time — this is runtime overlay only

## Capabilities

### New Capabilities
- `gservices-provider-detection`: Detect the active Google Services provider and version from the running Android guest
- `gservices-provider-switching`: Switch between MicroG, GApps, and None modes including download, flash, removal, and restart signaling
- `gservices-status`: Expose provider status (active provider, version, pending actions) for UI and config consumption

### Modified Capabilities
- `config-schema`: Add fields to `GoogleServicesConfig` for tracking installed provider version and GApps package source preference

## Impact

- **Code**: New `nux-core/src/gservices/` module tree. Depends on `nux-core::adb` for guest queries and package operations, `nux-core::config` for persisting provider choice.
- **Dependencies**: `reqwest` (or similar) for GApps package download, `sha2` for integrity verification. Depends on `adb-bridge` and `config-system` changes.
- **Filesystem**: Downloaded GApps packages cached under XDG data directory. System image overlays stored per-instance.
- **Systems**: Requires running VM with ADB connectivity for detection. GApps flash modifies the instance's system image overlay and requires VM restart.
