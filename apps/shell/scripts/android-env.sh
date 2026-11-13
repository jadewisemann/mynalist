#!/usr/bin/env bash
# Environment for Android builds on this machine. `source` this, do not run it.
#
# This box is aarch64; the Android SDK/NDK ship x86-64 tools. qemu-user binfmt
# (registered in /proc/sys/fs/binfmt_misc/qemu-x86_64) plus an x86-64 glibc under
# /lib64 make those tools runnable unmodified. Verify with:
#   scripts/doctor.sh
#
# Everything is overridable so the same script works on a normal x86-64 machine
# or on a Mac (where it is not needed at all).

export ANDROID_HOME="${ANDROID_HOME:-$HOME/opt/android-sdk}"
export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"

# Only one NDK is installed here; pick the newest if that changes.
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  _ndk_root="$ANDROID_HOME/ndk"
  if [ -d "$_ndk_root" ]; then
    ANDROID_NDK_HOME="$(find "$_ndk_root" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -1)"
  fi
  export ANDROID_NDK_HOME
  unset _ndk_root
fi
export NDK_HOME="${NDK_HOME:-$ANDROID_NDK_HOME}"

# minSdk of the generated Gradle project. Keep in sync with
# src-tauri/gen/android/app/build.gradle.kts (and the linker names in
# src-tauri/.cargo/config.toml).
export ANDROID_API_LEVEL="${ANDROID_API_LEVEL:-24}"

# The NDK toolchain directory: contains the per-target clang wrappers and llvm-ar.
ANDROID_NDK_BIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
if [ -d "$ANDROID_NDK_BIN" ]; then
  case ":$PATH:" in
    *":$ANDROID_NDK_BIN:"*) ;;
    *) export PATH="$ANDROID_NDK_BIN:$PATH" ;;
  esac
fi

export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk-arm64}"
case ":$PATH:" in
  *":$JAVA_HOME/bin:"*) ;;
  *) export PATH="$JAVA_HOME/bin:$PATH" ;;
esac

# Point build tools at the x86-64 glibc under qemu. On a normal x86-64 host
# these are already the system paths and setting them is harmless.
if [ -d /usr/lib/x86_64-linux-gnu ]; then
  export QEMU_LD_PREFIX="${QEMU_LD_PREFIX:-}"
fi

# Gradle's cache location; CI overrides this implicitly via the runner home.
export GRADLE_USER_HOME="${GRADLE_USER_HOME:-$HOME/.gradle}"
