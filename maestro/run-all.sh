#!/usr/bin/env bash
# Run all Maestro flows against a clean install of Yappy on the simulator.
# Maestro's `clearState: true` doesn't reliably wipe iOS UserDefaults
# (first_launch_done persists across `simctl install`), so we do a real
# uninstall+install dance up front.

set -euo pipefail

SIM="${SIM_UDID:-E08E324E-5C95-433C-88D6-F63B652DE9E8}"  # iPhone 17 Pro by default
APP="${YAPPY_APP:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/yappy-app/src-tauri/gen/apple/build/arm64-sim/Yappy.app}"

if [ ! -d "$APP" ]; then
  echo "ERROR: build first — no app at $APP"
  echo "       cd yappy-app && cargo tauri ios build --debug --target aarch64-sim"
  exit 1
fi

echo "→ uninstall com.yappy.app from $SIM"
xcrun simctl uninstall "$SIM" com.yappy.app 2>&1 | tail -1 || true
echo "→ install fresh build"
xcrun simctl install "$SIM" "$APP"
echo

FLOWS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/flows"
echo "→ running flows from $FLOWS"
maestro --device "$SIM" test "$FLOWS/"
