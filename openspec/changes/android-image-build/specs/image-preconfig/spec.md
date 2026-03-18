## ADDED Requirements

### Requirement: Setup wizard skipped
The image SHALL be pre-configured to skip the Android setup wizard on first boot.

#### Scenario: First boot bypasses setup
- **WHEN** the image boots for the first time
- **THEN** the setup wizard SHALL NOT appear and the device SHALL boot directly to the home screen

#### Scenario: Setup wizard skip property set
- **WHEN** the system image is built
- **THEN** `ro.setupwizard.mode` SHALL be set to `DISABLED` in `build.prop`

### Requirement: ADB enabled by default
The image SHALL have ADB (Android Debug Bridge) enabled by default without requiring developer options activation.

#### Scenario: ADB accessible on boot
- **WHEN** the image boots and the Nux emulator connects via ADB
- **THEN** ADB SHALL be accessible without any manual enablement steps on the Android side

#### Scenario: ADB properties set
- **WHEN** the system image is built
- **THEN** `ro.adb.secure` SHALL be set to `0`, `persist.sys.usb.config` SHALL include `adb`, and `ro.debuggable` SHALL be set to `1`

### Requirement: Emulator-optimized default settings
The image SHALL include default settings optimized for emulator use, reducing unnecessary overhead and improving responsiveness.

#### Scenario: Animations reduced
- **WHEN** the image boots
- **THEN** window animation scale, transition animation scale, and animator duration scale SHALL be set to `0.5` or lower

#### Scenario: Screen timeout extended
- **WHEN** the image boots with default settings
- **THEN** the screen timeout SHALL be set to a value of at least 30 minutes to prevent unnecessary screen-off during emulator use

#### Scenario: Stay awake while charging enabled
- **WHEN** the image boots (the virtual device is always "charging")
- **THEN** the stay-awake-while-charging setting SHALL be enabled by default

### Requirement: Developer options pre-enabled
The image SHALL have developer options enabled by default.

#### Scenario: Developer options accessible
- **WHEN** the user opens Settings on first boot
- **THEN** Developer Options SHALL be visible and accessible without the "tap build number 7 times" ritual
