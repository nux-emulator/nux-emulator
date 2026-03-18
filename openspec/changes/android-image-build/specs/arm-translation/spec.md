## ADDED Requirements

### Requirement: libndk_translation included
The build SHALL include `libndk_translation` for transparent ARM → x86_64 binary translation, enabling ARM-only apps and games to run on the x86_64 image.

#### Scenario: Translation libraries present
- **WHEN** the system image is built
- **THEN** ARM translation libraries SHALL be present at `/system/lib/arm/` (32-bit) and `/system/lib64/arm64/` (64-bit)

#### Scenario: ARM app executes
- **WHEN** an ARM-only APK is installed and launched
- **THEN** the app SHALL execute via binary translation without the user needing to perform any manual configuration

### Requirement: Translation is optional build flag
ARM translation integration SHALL be controlled by a build flag (`WITH_ARM_TRANSLATION`) defaulting to `true`, allowing builds without it for licensing or size reasons.

#### Scenario: Build with translation enabled
- **WHEN** `WITH_ARM_TRANSLATION=true` (or unset, using default)
- **THEN** libndk_translation SHALL be included in the system image

#### Scenario: Build with translation disabled
- **WHEN** `WITH_ARM_TRANSLATION=false` is set
- **THEN** libndk_translation SHALL be excluded from the system image and the image SHALL still boot and function correctly for x86_64-native apps

### Requirement: build.prop flags for translation
The build SHALL set the required `build.prop` properties to enable the Android runtime to use libndk_translation for ARM binary execution.

#### Scenario: Runtime properties set
- **WHEN** the system image is built with ARM translation enabled
- **THEN** `ro.dalvik.vm.native.bridge` SHALL be set to `libndk_translation.so` and `ro.enable.native.bridge.exec` SHALL be set to `1`

#### Scenario: Properties absent when disabled
- **WHEN** the system image is built with ARM translation disabled
- **THEN** `ro.dalvik.vm.native.bridge` SHALL be set to `0` and native bridge properties SHALL not reference libndk_translation
