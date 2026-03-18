## ADDED Requirements

### Requirement: Virtio-snd audio output
The system SHALL enable crosvm's virtio-snd device so the Android guest can output audio to the host. nux-core SHALL pass `--virtio-snd` to crosvm at VM launch.

#### Scenario: Audio output reaches host
- **WHEN** the VM is running with virtio-snd enabled and an Android app plays audio
- **THEN** the audio SHALL be audible on the host through PipeWire or PulseAudio

#### Scenario: crosvm launched with audio flag
- **WHEN** nux-core spawns the crosvm process
- **THEN** the crosvm command line SHALL include `--virtio-snd` with ALSA backend configuration

### Requirement: Audio latency optimization
The system SHALL configure crosvm's audio buffer with a period size of 256 frames and 2 periods to minimize latency. The system SHALL measure audio round-trip latency at VM startup and log the result.

#### Scenario: Low-latency audio configuration
- **WHEN** crosvm is launched with virtio-snd
- **THEN** the audio buffer SHALL be configured with period size 256 and period count 2

#### Scenario: Latency measurement logged
- **WHEN** the VM starts and audio is initialized
- **THEN** nux-core SHALL measure the audio round-trip latency and log it at info level

#### Scenario: High latency warning
- **WHEN** the measured audio latency exceeds 80ms
- **THEN** the system SHALL emit a warning log and notify the UI to display a latency warning to the user

### Requirement: Audio error handling
The system SHALL handle audio initialization failures gracefully. If the host audio stack is unavailable, the VM SHALL still boot but with audio disabled, and the user SHALL be notified.

#### Scenario: Host audio unavailable
- **WHEN** crosvm cannot connect to the host ALSA/PipeWire/PulseAudio
- **THEN** the VM SHALL boot without audio and nux-core SHALL report the audio failure to the UI for display to the user

### Requirement: Volume control via UI
The system SHALL expose a volume control in the nux-ui toolbar consisting of a mute/unmute toggle button and a volume slider. Volume changes SHALL be applied to the crosvm virtio-snd stream via crosvm's control socket.

#### Scenario: User adjusts volume
- **WHEN** the user moves the volume slider in the toolbar
- **THEN** the system SHALL send a volume adjustment command to crosvm's control socket and the guest audio output level SHALL change accordingly

#### Scenario: User toggles mute
- **WHEN** the user clicks the mute/unmute button in the toolbar
- **THEN** the system SHALL mute or unmute the crosvm audio stream and the button icon SHALL reflect the current mute state

#### Scenario: Volume state persisted
- **WHEN** the user sets a volume level and restarts the VM
- **THEN** the volume level SHALL be restored from the Nux configuration file on next launch

### Requirement: Audio configuration in TOML
The system SHALL support audio configuration in the Nux TOML config file, including `enabled` (bool), `volume` (0-100 integer), and `muted` (bool) fields under an `[audio]` section.

#### Scenario: Audio disabled via config
- **WHEN** the config file contains `[audio]` with `enabled = false`
- **THEN** nux-core SHALL NOT pass `--virtio-snd` to crosvm and no audio device SHALL be available in the guest

#### Scenario: Default audio configuration
- **WHEN** no `[audio]` section exists in the config file
- **THEN** the system SHALL default to `enabled = true`, `volume = 80`, `muted = false`
