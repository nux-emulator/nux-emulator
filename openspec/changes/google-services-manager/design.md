## Context

Nux ships a pre-built AOSP 16 system image with MicroG baked in for Google Services compatibility. Many Android games require Play Services for authentication (Google Play Games), in-app purchases, and cloud saves. MicroG covers most cases, but some games enforce signature checks that only pass with official GApps. Users need a managed way to switch providers without manual image surgery.

The `gservices` module sits between the ADB bridge (for guest communication) and the config system (for persisting choices). It also touches the instance's system image via overlay mounts.

Constraints:
- Rust-only, no C++ FFI
- Must work with VM stopped (for image modification) and running (for detection)
- ADB bridge must be available for guest-side queries
- Config system must be initialized before provider state can be persisted
- System image modifications must be per-instance (never touch the shared base image)

## Goals / Non-Goals

**Goals:**
- Provide a clean API for querying, switching, and removing Google Services providers
- Support MicroG (default), GApps (OpenGApps/MindTheGapps), and None as provider modes
- Download GApps packages with integrity verification before flashing
- Modify only per-instance overlays, preserving the shared base system image
- Signal restart requirements to the UI layer after provider changes
- Persist provider state in the config system

**Non-Goals:**
- Building or patching MicroG from source
- Google account sign-in or token management
- Automatic background updates of GApps packages
- Supporting arbitrary custom GApps variants beyond OpenGApps and MindTheGapps
- Multi-instance simultaneous provider switching

## Decisions

### 1. Overlay-based system image modification (not direct image patching)

**Choice**: Use a writable overlay filesystem layer per instance rather than modifying system.img directly.
**Rationale**: The base system.img is shared across instances and should remain immutable. An overlay approach (using crosvm's writable disk overlay or a separate partition image) lets each instance have its own provider state without duplicating the full system image. Reverting to MicroG or None is a matter of resetting the overlay.
**Alternatives considered**: Direct system.img copy-and-modify per instance (wastes ~2-4GB per instance), runtime bind-mount injection via init.d scripts (fragile, Android SELinux complications).

### 2. ADB-based provider detection

**Choice**: Detect the active provider by querying package manager via ADB (`pm list packages` for known package names, `dumpsys` for version info).
**Rationale**: This is the most reliable method — it checks what's actually installed in the running guest rather than inferring from host-side image state. Known package identifiers: `com.google.android.gms` (GApps), `com.google.android.gsf` (GApps framework), `org.microg.gms.droidguard` (MicroG).
**Alternatives considered**: Parsing the overlay contents on the host side (doesn't account for guest-side state), reading a marker file (extra moving part).

### 3. Two-phase flash: download then apply

**Choice**: Separate the GApps workflow into download (with hash verification) and apply (overlay modification) phases.
**Rationale**: Downloads can fail or be interrupted. By separating phases, a cached valid package can be re-applied without re-downloading. The apply phase runs only when the VM is stopped, preventing filesystem corruption.
**Alternatives considered**: Stream-and-apply in one step (no retry without re-download, risk of partial writes).

### 4. Provider state machine

**Choice**: Model provider transitions as an explicit state machine: `MicroG → GApps`, `MicroG → None`, `GApps → MicroG`, `GApps → None`, `None → MicroG`, `None → GApps`. Each transition has defined steps (download if needed, modify overlay, update config, signal restart).
**Rationale**: Prevents invalid intermediate states. Each transition can validate preconditions (e.g., VM must be stopped for flash, ADB must be available for detection). Makes error handling and rollback explicit.

### 5. reqwest for downloads, sha256 for verification

**Choice**: Use `reqwest` (async HTTP client) for downloading GApps packages and `sha2` crate for SHA-256 hash verification.
**Rationale**: `reqwest` is the de facto Rust HTTP client with good async support and TLS. SHA-256 hashes are published alongside OpenGApps and MindTheGapps releases. Verifying before apply prevents flashing corrupted packages.
**Alternatives considered**: `ureq` (sync-only, simpler but blocks), `curl` via command (not Rust-native).

### 6. Cache downloaded packages in XDG data directory

**Choice**: Store downloaded GApps zips under `$XDG_DATA_HOME/nux/cache/gapps/`.
**Rationale**: Follows XDG conventions already established by the config system. Cached packages can be reused across instances or after failed apply attempts. Cache can be cleared without affecting instance state.

## Risks / Trade-offs

- **[Risk] GApps download URLs change or become unavailable** → Mitigation: Support both OpenGApps and MindTheGapps as sources. URL patterns are configurable. Fail gracefully with clear error messages pointing users to manual download.
- **[Risk] Overlay modification corrupts instance state** → Mitigation: Back up the overlay before modification. If apply fails, restore the backup. The base image is never touched.
- **[Risk] SELinux denials after GApps flash** → Mitigation: The Android image build includes permissive SELinux policy for the GApps-related contexts. Document known SELinux issues.
- **[Risk] ADB not available when detection is requested** → Mitigation: Fall back to config-persisted provider state. Mark detection result as "cached" vs "live" so the UI can indicate staleness.
- **[Trade-off] VM must be stopped for provider switch** → Acceptable for v1. Live-switching would require runtime remounting which is complex and risky. A restart is a clear, safe boundary.

## Open Questions

- Should we support user-provided GApps zip files (local path) in addition to auto-download? Leaning yes for power users, but could defer to v2.
- Exact OpenGApps variant to default to (pico, nano, micro) — pico is smallest and sufficient for Play Services + Play Store.
- Whether to expose a "repair" action that re-applies the current provider's overlay if the guest state diverges from config.
