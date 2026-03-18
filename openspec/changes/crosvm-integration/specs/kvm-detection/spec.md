## ADDED Requirements

### Requirement: Detect KVM device availability
The system SHALL check that `/dev/kvm` exists and is a character device before attempting any VM operations.

#### Scenario: KVM device present and accessible
- **WHEN** the system checks for KVM support and `/dev/kvm` exists with read/write permissions for the current user
- **THEN** the device availability check SHALL pass

#### Scenario: KVM device missing
- **WHEN** the system checks for KVM support and `/dev/kvm` does not exist
- **THEN** the system SHALL return an error indicating KVM is not available and suggest enabling KVM in the kernel configuration

#### Scenario: KVM device permission denied
- **WHEN** the system checks for KVM support and `/dev/kvm` exists but the current user lacks read/write permissions
- **THEN** the system SHALL return an error indicating insufficient permissions and suggest adding the user to the `kvm` group

### Requirement: Verify KVM API version
The system SHALL verify the KVM API version by issuing a `KVM_GET_API_VERSION` ioctl on `/dev/kvm` and confirming it returns the expected stable version (12).

#### Scenario: Compatible KVM API version
- **WHEN** the system queries the KVM API version and it returns 12
- **THEN** the version check SHALL pass

#### Scenario: Incompatible KVM API version
- **WHEN** the system queries the KVM API version and it returns a value other than 12
- **THEN** the system SHALL return an error indicating an unsupported KVM API version with the actual version number

### Requirement: Check required KVM extensions
The system SHALL verify that required KVM extensions are available using `KVM_CHECK_EXTENSION` ioctl for: `KVM_CAP_IRQCHIP`, `KVM_CAP_USER_MEMORY`, `KVM_CAP_SET_TSS_ADDR`.

#### Scenario: All required extensions present
- **WHEN** the system checks for required KVM extensions and all are supported
- **THEN** the extension check SHALL pass

#### Scenario: Missing required extension
- **WHEN** the system checks for required KVM extensions and one or more are not supported
- **THEN** the system SHALL return an error listing the missing extensions by name

### Requirement: Detect CPU virtualization features
The system SHALL check CPUID for hardware virtualization support (VT-x on Intel via `CPUID.1:ECX.VMX` or AMD-V on AMD via `CPUID.8000_0001:ECX.SVM`) and extended page tables (EPT on Intel, NPT on AMD).

#### Scenario: CPU supports hardware virtualization with extended page tables
- **WHEN** the system checks CPU features and both virtualization and extended page tables are supported
- **THEN** the CPU feature check SHALL pass

#### Scenario: CPU lacks virtualization support
- **WHEN** the system checks CPU features and hardware virtualization is not detected
- **THEN** the system SHALL return an error indicating the CPU does not support hardware virtualization

#### Scenario: CPU lacks extended page tables
- **WHEN** the system checks CPU features and extended page tables are not supported
- **THEN** the system SHALL return a warning that performance may be degraded without EPT/NPT

### Requirement: Aggregate KVM readiness report
The system SHALL provide a single function that runs all detection checks and returns a structured report containing the pass/fail status and details for each check.

#### Scenario: All checks pass
- **WHEN** the system runs the full KVM readiness check and all sub-checks pass
- **THEN** the system SHALL return a report with an overall status of ready

#### Scenario: One or more checks fail
- **WHEN** the system runs the full KVM readiness check and any sub-check fails
- **THEN** the system SHALL return a report with an overall status of not-ready and include the failure details for each failed check
