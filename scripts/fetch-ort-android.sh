#!/usr/bin/env bash
# Baja el ONNX Runtime OFICIAL para Android (el AAR de Maven Central) y
# deja libonnxruntime.so + headers donde ort-sys y el APK los esperan.
# Espejo del fetch-ort-ios.sh: misma versión (rc.10 pide OrtApi 22).
set -euo pipefail
ORT_VERSION="${ORT_VERSION:-1.22.0}"
VENDOR="$HOME/.config/yappy-android/ort-${ORT_VERSION}"
if [ -f "$VENDOR/lib/libonnxruntime.so" ]; then
  echo "→ ya está: $VENDOR"
  exit 0
fi
mkdir -p "$VENDOR"
AAR="onnxruntime-android-${ORT_VERSION}.aar"
URL="https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android/${ORT_VERSION}/${AAR}"
echo "→ bajando $URL"
curl -fL "$URL" -o "$VENDOR/$AAR"
cd "$VENDOR"
/usr/bin/unzip -o -q "$AAR" "jni/arm64-v8a/*" "headers/*"
mkdir -p lib
cp jni/arm64-v8a/libonnxruntime.so lib/
echo "→ listo: $VENDOR/lib/libonnxruntime.so"
# La .so también al APK (jniLibs no va al repo: se regenera aquí).
JNI="$(cd "$(dirname "$0")/.." && pwd)/yappy-app/src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a"
mkdir -p "$JNI"
cp -f "$VENDOR/lib/libonnxruntime.so" "$JNI/"
