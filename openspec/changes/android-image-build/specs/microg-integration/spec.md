## ADDED Requirements

### Requirement: MicroG included as privileged system apps
The build SHALL include MicroG components (GmsCore, GsfProxy, FakeStore) as privileged system apps in the system partition.

#### Scenario: MicroG packages in system image
- **WHEN** the system image is built
- **THEN** `GmsCore.apk`, `GsfProxy.apk`, and `FakeStore.apk` SHALL be present under `/system/priv-app/` with appropriate directory structure

#### Scenario: MicroG has privileged permissions
- **WHEN** Android boots with the built image
- **THEN** MicroG components SHALL have privileged app permissions and SHALL be recognized as system apps

### Requirement: Signature spoofing patch applied
The build SHALL apply the signature spoofing framework patch to allow MicroG to impersonate Google Play Services.

#### Scenario: Spoofing patch present
- **WHEN** the AOSP source is prepared for building
- **THEN** the signature spoofing patch SHALL be applied to the framework, enabling the `android.permission.FAKE_PACKAGE_SIGNATURE` permission

#### Scenario: MicroG registers as GSF
- **WHEN** Android boots and MicroG initializes
- **THEN** MicroG SHALL successfully register as the Google Services Framework provider and apps querying Google Play Services availability SHALL receive a positive response

### Requirement: MicroG included via PRODUCT_PACKAGES
MicroG integration SHALL use the standard `PRODUCT_PACKAGES` mechanism in product makefiles, not post-build injection.

#### Scenario: Build system integration
- **WHEN** the product makefile is parsed
- **THEN** MicroG packages SHALL be listed in `PRODUCT_PACKAGES` and built/copied as part of the normal AOSP build flow

### Requirement: MicroG default permissions granted
The build SHALL include a default-permissions XML file that pre-grants required permissions to MicroG components so they function without user interaction on first boot.

#### Scenario: Permissions pre-granted
- **WHEN** Android boots for the first time
- **THEN** MicroG components SHALL have their required permissions (location, network, accounts) pre-granted without prompting the user
