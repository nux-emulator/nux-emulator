## ADDED Requirements

### Requirement: Root manager APK installation
The system SHALL install the selected root manager's APK into the running VM via the ADB bridge. The system MUST support installing Magisk, KernelSU, and APatch APKs.

#### Scenario: Install Magisk APK
- **WHEN** the user selects Magisk as the root manager and initiates patching
- **THEN** the system installs the Magisk APK into the VM via `adb install`

#### Scenario: Install KernelSU APK
- **WHEN** the user selects KernelSU as the root manager and initiates patching
- **THEN** the system installs the KernelSU APK into the VM via `adb install`

#### Scenario: Install APatch APK
- **WHEN** the user selects APatch as the root manager and initiates patching
- **THEN** the system installs the APatch APK into the VM via `adb install`

#### Scenario: APK installation failure
- **WHEN** the ADB install command fails
- **THEN** the system reports the error to the caller with the ADB error output

### Requirement: Stock boot image push to VM
The system SHALL push the stock boot.img from the host instance directory to a known path inside the VM via the ADB bridge, so the root manager app can access it for patching.

#### Scenario: Successful push
- **WHEN** the patching workflow pushes the stock boot image
- **THEN** the file is transferred to `/sdcard/boot.img` inside the VM via `adb push`

#### Scenario: Push failure
- **WHEN** the ADB push command fails (e.g., VM not running, disk full)
- **THEN** the system reports the error to the caller

### Requirement: Patched boot image pull from VM
The system SHALL pull the patched boot image from the VM back to the host after the user completes patching inside the root manager app. The system MUST store the pulled image using the patched image naming convention.

#### Scenario: Successful pull of Magisk-patched image
- **WHEN** the user has completed Magisk patching and the system pulls the patched image
- **THEN** the patched file is retrieved from the VM via `adb pull` and stored as `boot_magisk.img` in the instance directory

#### Scenario: Pull failure
- **WHEN** the ADB pull command fails or the expected patched file does not exist in the VM
- **THEN** the system reports the error to the caller

### Requirement: Patching workflow orchestration
The system SHALL provide a method that orchestrates the full patching sequence: install APK, push stock image, wait for user to patch, pull patched image, and update instance config. Each step MUST report progress to the caller.

#### Scenario: Complete patching workflow
- **WHEN** the user initiates the patching workflow for a given root manager
- **THEN** the system executes the steps in order: install APK, push stock boot.img, signal readiness for user patching, pull patched image, update config root mode

#### Scenario: Workflow aborted mid-step
- **WHEN** any step in the patching workflow fails
- **THEN** the system stops the workflow, reports which step failed, and leaves the instance config unchanged

### Requirement: Config update after successful patching
The system SHALL update the instance config's `root.mode` field to the newly patched root manager only after the patched boot image has been successfully pulled and stored.

#### Scenario: Config updated on success
- **WHEN** the patched boot image is successfully stored on the host
- **THEN** the instance config `root.mode` is set to the corresponding manager (e.g., `magisk`)

#### Scenario: Config unchanged on failure
- **WHEN** the patched boot image pull fails
- **THEN** the instance config `root.mode` remains at its previous value
