#!/bin/bash
# Cross-compilation env shared by the build scripts and the check-* recipes.
#
#   source bin/toolchain.sh <android|ios>

TOOLCHAIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_API=26

export OPUS_NO_PKG=1
export OPUS_STATIC=1

case "${1:-}" in
android)
    for _v in ANDROID_HOME NDK_HOME; do
        if [ ! -d "${!_v:-}" ]; then
            echo "error: \$$_v is not a directory: ${!_v:-<unset>}" >&2
            return 1 2>/dev/null || exit 1
        fi
    done

    ANDROID_NDK_HOME="$NDK_HOME"
    export ANDROID_HOME ANDROID_NDK_HOME NDK_HOME

    case "$(uname -s)" in
    Darwin) NDK_HOST_TAG=darwin-x86_64 ;;
    *) NDK_HOST_TAG=linux-x86_64 ;;
    esac
    NDK_BIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$NDK_HOST_TAG/bin"

    for _abi in \
        aarch64-linux-android:aarch64-linux-android \
        armv7-linux-androideabi:armv7a-linux-androideabi \
        i686-linux-android:i686-linux-android \
        x86_64-linux-android:x86_64-linux-android; do
        _triple="${_abi%%:*}"
        _clang="$NDK_BIN/${_abi##*:}$ANDROID_API-clang"
        _var="${_triple//-/_}"
        export "CC_$_var=$_clang"
        export "CXX_$_var=$_clang++"
        export "AR_$_var=$NDK_BIN/llvm-ar"
        export "CARGO_TARGET_$(echo "$_var" | tr '[:lower:]' '[:upper:]')_LINKER=$_clang"
    done
    unset _abi _triple _clang _var _v

    opus_lib_dir_for() {
        case "$1" in
        aarch64* | arm64*) echo "$TOOLCHAIN_ROOT/bin/deps/android-arm64" ;;
        armv7* | arm) echo "$TOOLCHAIN_ROOT/bin/deps/android-armv7" ;;
        i686* | x86) echo "$TOOLCHAIN_ROOT/bin/deps/android-x86" ;;
        x86_64*) echo "$TOOLCHAIN_ROOT/bin/deps/android-x86_64" ;;
        *)
            echo "error: no libopus for android target '$1'" >&2
            return 1
            ;;
        esac
    }

    export OPUS_LIB_DIR="$TOOLCHAIN_ROOT/bin/deps/android-arm64"
    ;;
ios)
    export OPUS_LIB_DIR="$TOOLCHAIN_ROOT/bin/deps/ios-arm64"
    ;;
*)
    echo "usage: source bin/toolchain.sh <android|ios>" >&2
    return 1 2>/dev/null || exit 1
    ;;
esac
