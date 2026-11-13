#!/usr/bin/env bash
# Checks that this machine can build each target, and says exactly what is
# missing when it cannot. Run this first when a build fails.
#
# Usage: scripts/doctor.sh
set -uo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
RC=0
ok()   { printf '  ok    %s\n' "$*"; }
miss() { printf '  MISS  %s\n' "$*"; RC=1; }
info() { printf '  --    %s\n' "$*"; }

echo "host"
info "$(uname -srm)"

echo
echo "node / rust"
node --version  >/dev/null 2>&1 && ok "node $(node --version)" || miss "node"
rustc --version >/dev/null 2>&1 && ok "rustc $(rustc --version | cut -d' ' -f2)" || miss "rustc"
for t in aarch64-unknown-linux-gnu aarch64-apple-darwin aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android; do
  if rustup target list --installed 2>/dev/null | grep -qx "$t"; then ok "target $t"; else info "target $t not installed (rustup target add $t)"; fi
done

echo
echo "linux desktop (Tauri 2 / WebKitGTK)"
for p in webkit2gtk-4.1 javascriptcoregtk-4.1 libsoup-3.0 gtk+-3.0; do
  v="$(pkg-config --modversion "$p" 2>/dev/null)" && ok "$p $v" || miss "$p (apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev)"
done
command -v rsvg-convert >/dev/null && ok "rsvg-convert (icons)" || info "rsvg-convert not found (only needed to regenerate icons)"
command -v dbus-run-session >/dev/null && ok "dbus-run-session (WebKitGTK needs a session bus)" || info "dbus-run-session not found; headless smoke tests will hang"
command -v xdotool >/dev/null && ok "xdotool (window checks)" || info "xdotool not found (smoke tests only)"

echo
echo "macOS"
if command -v xcodebuild >/dev/null 2>&1; then
  ok "xcodebuild $(xcodebuild -version 2>/dev/null | head -1)"
else
  info "no xcodebuild: macOS builds must run on a Mac (expected on Linux)"
fi

echo
echo "android"
: "${ANDROID_HOME:=$HOME/opt/android-sdk}"
: "${JAVA_HOME:=}"
if [ -d "$ANDROID_HOME" ]; then ok "ANDROID_HOME=$ANDROID_HOME"; else miss "ANDROID_HOME not found at $ANDROID_HOME"; fi
[ -n "$JAVA_HOME" ] && [ -x "$JAVA_HOME/bin/java" ] && ok "JAVA_HOME=$JAVA_HOME" \
  || info "JAVA_HOME unset (source scripts/android-env.sh)"

NDK="$(find "$ANDROID_HOME/ndk" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -1)"
if [ -n "$NDK" ]; then
  ok "NDK $(basename "$NDK")"
  NDKCLANG="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/clang"
  if [ -x "$NDKCLANG" ] && "$NDKCLANG" --version >/dev/null 2>&1; then
    ok "NDK clang is executable"
  else
    # aarch64 host + x86-64 NDK: needs qemu-user binfmt and an x86-64 glibc
    miss "NDK clang cannot execute (this host is $(uname -m); the NDK ships linux-x86_64 tools)"
    info "  fix: sudo apt install qemu-user-static, then install an x86-64 libc under /lib64"
    info "  (this repo's build was validated with qemu-x86_64 binfmt + x86-64 libc6/libstdc++6/libgcc-s1/zlib1g)"
  fi
else
  miss "no NDK under $ANDROID_HOME/ndk"
fi

echo "  installed platforms: $(ls "$ANDROID_HOME/platforms" 2>/dev/null | tr '\n' ' ')"
echo "  installed build-tools: $(ls "$ANDROID_HOME/build-tools" 2>/dev/null | tr '\n' ' ')"
[ -d "$ANDROID_HOME/platforms/android-37.0" ] && ok "platform android-37.0 (compileSdk 37)" \
  || info "platform android-37.0 missing (sdkmanager 'platforms;android-37.0')"
[ -d "$ANDROID_HOME/build-tools/37.0.0" ] && ok "build-tools 37.0.0" \
  || info "build-tools 37.0.0 missing (sdkmanager 'build-tools;37.0.0')"
[ -e /dev/kvm ] && ok "/dev/kvm present (emulator possible)" \
  || info "no /dev/kvm: an emulator cannot run here, so APK execution is untested"

echo
echo "vars set by scripts/android-env.sh"
for v in ANDROID_HOME ANDROID_NDK_HOME NDK_HOME ANDROID_API_LEVEL JAVA_HOME; do
  printf '  %-18s %s\n' "$v" "${!v:-<unset>}"
done

echo
[ "$RC" -eq 0 ] && echo "PASS: required pieces are present" || echo "INCOMPLETE: see MISS lines above"
exit "$RC"
