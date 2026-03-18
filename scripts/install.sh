#!/usr/bin/env bash
# install.sh — Install Nux Emulator from an extracted tarball.
#
# Usage:
#   tar xzf nux-emulator-*-linux-x86_64.tar.gz
#   cd nux-emulator-*-linux-x86_64
#   sudo ./install.sh
#
# To install to a custom prefix (e.g., for testing):
#   DESTDIR=/tmp/test-install ./install.sh
#
# Runtime dependencies: GTK4 (>= 4.12), libadwaita (>= 1.4)
set -euo pipefail

DESTDIR="${DESTDIR:-}"
PREFIX="${PREFIX:-/usr}"

BIN_DIR="${DESTDIR}${PREFIX}/bin"
APP_DIR="${DESTDIR}${PREFIX}/share/applications"
ICON_DIR="${DESTDIR}${PREFIX}/share/icons/hicolor/scalable/apps"
META_DIR="${DESTDIR}${PREFIX}/share/metainfo"
KEYMAP_DIR="${DESTDIR}${PREFIX}/share/nux-emulator/keymaps"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Verify we're in the right directory
if [ ! -f "${SCRIPT_DIR}/nux-emulator" ]; then
  echo "Error: nux-emulator binary not found in ${SCRIPT_DIR}"
  echo "Run this script from inside the extracted tarball directory."
  exit 1
fi

echo "Installing Nux Emulator..."
echo "  DESTDIR=${DESTDIR:-(none)}"
echo "  PREFIX=${PREFIX}"
echo ""

install -Dm755 "${SCRIPT_DIR}/nux-emulator"              "${BIN_DIR}/nux-emulator"
install -Dm644 "${SCRIPT_DIR}/nux-emulator.desktop"       "${APP_DIR}/nux-emulator.desktop"
install -Dm644 "${SCRIPT_DIR}/nux-emulator.svg"           "${ICON_DIR}/nux-emulator.svg"
install -Dm644 "${SCRIPT_DIR}/nux-emulator.metainfo.xml"  "${META_DIR}/nux-emulator.metainfo.xml"

if [ -d "${SCRIPT_DIR}/keymaps" ] && [ "$(ls -A "${SCRIPT_DIR}/keymaps" 2>/dev/null)" ]; then
  mkdir -p "${KEYMAP_DIR}"
  cp -r "${SCRIPT_DIR}/keymaps/"* "${KEYMAP_DIR}/"
  echo "  Keymaps -> ${KEYMAP_DIR}/"
fi

echo "  Binary  -> ${BIN_DIR}/nux-emulator"
echo "  Desktop -> ${APP_DIR}/nux-emulator.desktop"
echo "  Icon    -> ${ICON_DIR}/nux-emulator.svg"
echo "  Metainfo-> ${META_DIR}/nux-emulator.metainfo.xml"
echo ""
echo "Installation complete."
