#!/usr/bin/env bash
# act-build.sh — Run the build-linux workflow locally using nektos/act.
#
# Produces .deb, .rpm, and .tar.gz packages in ./build/.
#
# Known limitations vs GitHub Actions:
# - Service containers are not supported by act.
# - actions/cache is skipped by default in act.
# - The Fedora container job may behave slightly differently.
set -euo pipefail

# Check for act
if ! command -v act &>/dev/null; then
  echo "Error: 'act' is not installed."
  echo ""
  echo "Install nektos/act:"
  echo "  curl -s https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash"
  echo "  # or: brew install act"
  echo ""
  echo "See https://github.com/nektos/act for details."
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

BUILD_DIR="${PROJECT_ROOT}/build"
mkdir -p "${BUILD_DIR}"

echo "Running build workflow locally via act..."
echo "Packages will be written to: ${BUILD_DIR}"
echo ""

act push \
  --workflows "${PROJECT_ROOT}/.github/workflows/build-linux.yml" \
  --bind \
  --env ACT=true \
  --directory "${PROJECT_ROOT}" \
  -v "${BUILD_DIR}:/build"

echo ""
echo "Build complete. Packages:"
ls -lh "${BUILD_DIR}"/*.deb "${BUILD_DIR}"/*.rpm "${BUILD_DIR}"/*.tar.gz 2>/dev/null || echo "  (no packages found — check act output above for errors)"
