## ADDED Requirements

### Requirement: Desktop entry file
The project SHALL include a `nux-emulator.desktop` file conforming to the freedesktop Desktop Entry Specification v1.5.

#### Scenario: Desktop entry has required keys
- **WHEN** the desktop file is validated
- **THEN** it contains `Type=Application`, `Name=Nux Emulator`, `Exec=nux-emulator`, `Icon=nux-emulator`, and `Categories` including `Game` and `Emulator`

#### Scenario: Desktop entry is valid
- **WHEN** the desktop file is checked with `desktop-file-validate`
- **THEN** no errors are reported

### Requirement: Application icon in SVG format
The project SHALL include an SVG application icon at a path suitable for installation to `/usr/share/icons/hicolor/scalable/apps/nux-emulator.svg`.

#### Scenario: Icon is valid SVG
- **WHEN** the icon file is inspected
- **THEN** it is a well-formed SVG document

#### Scenario: Icon is square
- **WHEN** the icon SVG viewBox is inspected
- **THEN** it has equal width and height dimensions

### Requirement: AppStream metainfo file
The project SHALL include an AppStream metainfo XML file conforming to the AppStream specification, enabling discovery in software centers like GNOME Software.

#### Scenario: Metainfo has required fields
- **WHEN** the metainfo XML is inspected
- **THEN** it contains `<id>`, `<name>`, `<summary>`, `<description>`, `<launchable>`, `<url type="homepage">`, `<project_license>`, and `<content_rating>`

#### Scenario: Metainfo references correct desktop entry
- **WHEN** the metainfo `<launchable type="desktop-id">` is inspected
- **THEN** it references `nux-emulator.desktop`

#### Scenario: Metainfo passes validation
- **WHEN** the metainfo file is checked with `appstreamcli validate`
- **THEN** no errors are reported

### Requirement: Desktop files are included in all packages
All package formats (.deb, .rpm, tar.gz) SHALL include the desktop entry, icon, and metainfo files installed to their FHS-standard locations.

#### Scenario: Deb package includes desktop integration files
- **WHEN** the `.deb` package contents are listed
- **THEN** it includes `nux-emulator.desktop` in `/usr/share/applications/`, the SVG icon in `/usr/share/icons/hicolor/scalable/apps/`, and the metainfo in `/usr/share/metainfo/`

#### Scenario: RPM package includes desktop integration files
- **WHEN** the `.rpm` package contents are listed
- **THEN** it includes the same desktop integration files at the same paths

#### Scenario: Tarball includes desktop integration files
- **WHEN** the `.tar.gz` archive contents are listed
- **THEN** it includes the desktop entry, icon, and metainfo files
