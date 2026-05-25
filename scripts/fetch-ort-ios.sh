#!/usr/bin/env bash
# Fetch the ONNX Runtime iOS XCFramework from Microsoft's public CDN and
# stage its static archives where ort-sys' build script can find them.
#
# Microsoft ships iOS via CocoaPods + a public CDN download (the same URL
# the `onnxruntime-c` podspec points at). We pull straight from the CDN to
# avoid round-tripping through a synthetic Xcode project just to drive
# `pod install`. Once `tauri ios init` generates a real iOS project we can
# also wire onnxruntime-c as a proper Pod dependency at the app layer — but
# the Rust build needs the static libs ahead of that.
#
# Output layout (read by `.cargo/config.toml`):
#   vendor/ort-ios/
#     ios-arm64/libonnxruntime.a                 (real device)
#     ios-arm64_x86_64-simulator/libonnxruntime.a (simulator universal)
#
# Pin to ORT 1.20.x: matches `ort 2.0.0-rc.10`'s ORT_API_VERSION == 22.

set -euo pipefail

# 1.20.1 is not on the CDN as of writing (404), 1.20.0 is. ORT_API_VERSION is
# 22 in both, so ort 2.0.0-rc.10's bindings are ABI-compatible.
ORT_VERSION="${ORT_VERSION:-1.20.0}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENDOR="$ROOT/vendor/ort-ios"
WORK="$ROOT/.cache/ort-ios"

mkdir -p "$VENDOR" "$WORK"
cd "$WORK"

ZIP="pod-archive-onnxruntime-c-${ORT_VERSION}.zip"
URL="https://download.onnxruntime.ai/${ZIP}"

if [ ! -f "$ZIP" ]; then
  echo "→ downloading $URL"
  curl -fL --progress-bar -o "$ZIP" "$URL"
else
  echo "→ using cached $WORK/$ZIP"
fi

EXTRACT="$WORK/extract-$ORT_VERSION"
rm -rf "$EXTRACT"
mkdir -p "$EXTRACT"
echo "→ unzipping into $EXTRACT"
unzip -q "$ZIP" -d "$EXTRACT"

XCFW=$(find "$EXTRACT" -type d -name "onnxruntime.xcframework" | head -1)
if [ -z "$XCFW" ]; then
  echo "ERROR: onnxruntime.xcframework not found in $EXTRACT"
  exit 1
fi
echo "→ xcframework: $XCFW"

# Each slice's framework contains a static archive named `onnxruntime`
# (no `lib` prefix, no `.a` suffix). ort-sys looks for `libonnxruntime.a`.
for SLICE in ios-arm64 ios-arm64_x86_64-simulator; do
  SRC="$XCFW/$SLICE/onnxruntime.framework/onnxruntime"
  if [ ! -f "$SRC" ]; then
    echo "  WARN: slice $SLICE missing, skipping"
    continue
  fi
  DST="$VENDOR/$SLICE"
  rm -rf "$DST"
  mkdir -p "$DST"
  cp -f "$SRC" "$DST/libonnxruntime.a"
  # Sanity check the architecture(s) actually present.
  ARCHS=$(lipo -archs "$DST/libonnxruntime.a" 2>/dev/null || echo "unknown")
  echo "  staged $SLICE  ($(ls -lh "$DST/libonnxruntime.a" | awk '{print $5}'), archs: $ARCHS)"
done

# Stash the full xcframework too — it'll be linked into the iOS app bundle
# by Xcode once `tauri ios init` has generated the project.
rm -rf "$VENDOR/onnxruntime.xcframework"
cp -R "$XCFW" "$VENDOR/onnxruntime.xcframework"
echo "→ full xcframework copied to $VENDOR/onnxruntime.xcframework"
echo
echo "✓ ORT iOS vendored under $VENDOR"
