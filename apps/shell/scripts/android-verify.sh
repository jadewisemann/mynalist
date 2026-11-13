#!/usr/bin/env bash
# Verify an Android APK produced by this project.
#
# Checks that are meaningful without a device (there is no /dev/kvm here, so an
# emulator cannot run):
#   * the APK parses, and package/minSdk/targetSdk/label are what we configured
#   * every bundled ABI has exactly one lib<name>.so
#   * each .so is ELF for the right machine and has JNI entry points
#   * each .so has 16 KB PT_LOAD alignment (Android 15 16 KB-page devices)
#   * the web build is actually embedded (asset marker string present)
#   * debug APK is signed (debug keystore); a release APK is expected unsigned
#     unless you passed a signing key
#
# Usage: scripts/android-verify.sh <path-to.apk>
set -uo pipefail

APK="${1:-}"
[ -f "$APK" ] || { echo "usage: $0 <path-to.apk>"; exit 2; }
: "${ANDROID_HOME:=$HOME/opt/android-sdk}"
: "${ANDROID_NDK_HOME:=$(find "$ANDROID_HOME/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -1)}"
BT="$(find "$ANDROID_HOME/build-tools" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -1)"
TC="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
RC=0
say() { printf '%s\n' "$*"; }
ok()  { say "OK:   $*"; }
bad() { say "FAIL: $*"; RC=1; }

say "apk:  $APK"
say "size: $(du -h "$APK" | cut -f1)"
say

# --- manifest ---------------------------------------------------------------
BADGING="$("$BT/aapt2" dump badging "$APK" 2>/dev/null)"
grep -q "package: name='dev.outliner.app'" <<<"$BADGING" \
  && ok "package dev.outliner.app" || bad "package name is not dev.outliner.app"
grep -q "application-label:'Outliner'" <<<"$BADGING" \
  && ok "label 'Outliner'" || bad "label is not Outliner"
MIN="$(sed -n "s/^minSdkVersion:'\([0-9]*\)'.*/\1/p" <<<"$BADGING")"
TGT="$(sed -n "s/^targetSdkVersion:'\([0-9]*\)'.*/\1/p" <<<"$BADGING")"
say "       minSdk=$MIN targetSdk=$TGT"
grep -q "android.permission.INTERNET" <<<"$BADGING" \
  && ok "INTERNET permission (server sync is optional, but the app must be able to talk to one)" \
  || say "INFO: no INTERNET permission"

# --- ABIs and libraries -----------------------------------------------------
mapfile -t ABIS < <(unzip -Z1 "$APK" | sed -n 's|^lib/\([^/]*\)/.*|\1|p' | sort -u)
[ "${#ABIS[@]}" -gt 0 ] && ok "ABIs: ${ABIS[*]}" || bad "no lib/ entries in APK"

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
for abi in "${ABIS[@]}"; do
  n="$(unzip -Z1 "$APK" | grep -c "^lib/$abi/liboutliner_lib.so$")"
  [ "$n" = 1 ] && ok "$abi: liboutliner_lib.so present" || bad "$abi: expected 1 liboutliner_lib.so, found $n"
  unzip -qo "$APK" "lib/$abi/liboutliner_lib.so" -d "$TMP"
  so="$TMP/lib/$abi/liboutliner_lib.so"

  machine="$("$TC/llvm-readelf" -h "$so" | sed -n 's/^ *Machine: *//p')"
  case "$abi:$machine" in
    "arm64-v8a:AArch64"|"armeabi-v7a:ARM"|"x86:Intel 80386"|"x86_64:Advanced Micro Devices X86-64")
      ok "$abi: ELF machine '$machine'" ;;
    *) bad "$abi: ELF machine '$machine' does not match ABI" ;;
  esac

  if "$TC/llvm-nm" -D --defined-only "$so" > "$TMP/$abi.nm"; grep -q 'Java_dev_outliner_app_Rust_onCreate' "$TMP/$abi.nm"; then
    ok "$abi: JNI entry point Java_dev_outliner_app_Rust_onCreate exported"
  else
    bad "$abi: JNI entry point missing"
  fi

  # Android 15 (API 35)+ ships 16 KB page devices; 4 KB-aligned .so fails there.
  aligns="$("$TC/llvm-readelf" -lW "$so" | sed -n 's/^  LOAD .* \(0x[0-9a-f]*\)$/\1/p' | sort -u)"
  if grep -qvE '^0x(4000|10000)$' <<<"$aligns"; then
    bad "$abi: PT_LOAD alignment not 16 KB: $(tr '\n' ' ' <<<"$aligns")"
  else
    ok "$abi: PT_LOAD alignment 16 KB ($(tr '\n' ' ' <<<"$aligns"))"
  fi

  # The web build is embedded in the binary; this is the only way to see it
  # from outside without running the app. `strings` is dumped to a file first:
  # with `set -o pipefail`, piping into `grep -q` makes grep exit on the first
  # match, SIGPIPE kills strings, and the pipeline reports failure even on a hit.
  STR="$TMP/$abi.strings"
  strings "$so" >"$STR"
  js="$(grep -o 'assets/index-[A-Za-z0-9_-]*\.js' "$STR" | head -1)"
  if [ -n "$js" ]; then
    ok "$abi: web build embedded ($js)"
  else
    bad "$abi: web build not found inside the .so"
  fi
done

# --- signature --------------------------------------------------------------
if [[ "$APK" == *debug* ]]; then
  "$BT/apksigner" verify "$APK" >/dev/null 2>&1 \
    && ok "debug APK is signed (debug key)" || bad "debug APK signature invalid"
else
  if "$BT/apksigner" verify "$APK" >/dev/null 2>&1; then
    ok "APK is signed"
  else
    say "INFO: release APK is unsigned (pass a keystore to tauri android build to sign it)"
  fi
fi

say
[ "$RC" -eq 0 ] && echo "PASS" || echo "FAIL"
exit "$RC"
