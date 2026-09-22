#!/usr/bin/env bash
# El entorno Android de la casa: PATH limpio, NDK 27 y el shim de fdk-aac.
# OJO: el shim vive en ~/.config/yappy-android/shim (SIN espacios): cc-rs
# tokeniza CFLAGS por espacios y el path del repo los tiene.
# Uso: source scripts/android-env.sh
export PATH="/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:$HOME/.cargo/bin:$HOME/Library/Android/sdk/platform-tools"
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/27.1.12297006"
export JAVA_HOME="/Library/Java/JavaVirtualMachines/zulu-17.jdk/Contents/Home"
mkdir -p "$HOME/.config/yappy-android/shim/log"
cp -f "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/yappy-app/src-tauri/android-shim/log/log.h" \
      "$HOME/.config/yappy-android/shim/log/log.h" 2>/dev/null || true
export CFLAGS_aarch64_linux_android="-I$HOME/.config/yappy-android/shim"
export CXXFLAGS_aarch64_linux_android="-I$HOME/.config/yappy-android/shim"
_TC="$NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin"
export CC_aarch64_linux_android="$_TC/aarch64-linux-android24-clang"
export CXX_aarch64_linux_android="$_TC/aarch64-linux-android24-clang++"
export AR_aarch64_linux_android="$_TC/llvm-ar"
export RANLIB_aarch64_linux_android="$_TC/llvm-ranlib"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$_TC/aarch64-linux-android24-clang"
export ORT_LIB_LOCATION="$HOME/.config/yappy-android/ort-1.22.0/lib"
# Páginas de 16 KB (Android 15+ lo exige a las apps nuevas de Play): el .so de
# Rust se enlaza con esa alineación; onnxruntime-android 1.22 ya la trae.
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384"
