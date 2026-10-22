# @outliner/android

Capacitor 8 shell around the `apps/web` build (`webDir: ../web/dist`). App id `dev.outliner.app`, name "Outliner".

## Build

```sh
export JAVA_HOME=$HOME/opt/jdk-21
export ANDROID_HOME=$HOME/opt/android-sdk
export PATH=$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$PATH

npm install                          # from the repo root
npm run build -w @outliner/android   # web build + cap sync + ./gradlew assembleDebug
```

Output: `apps/android/android/app/build/outputs/apk/debug/app-debug.apk` (debug-signed).

Other scripts: `sync` (web build + `cap sync android`), `icons` (regenerate launcher icon/splash via
`scripts/gen-icons.py`, stdlib Python only), `open` (Android Studio).

## Toolchain (as installed on the dev box)

| What | Where |
| --- | --- |
| Temurin JDK 21 (aarch64) | `~/opt/jdk-21` -> `~/opt/jdk-21.0.12.1+1` |
| Android cmdline-tools 20.0 | `~/opt/android-sdk/cmdline-tools/latest` |
| platform-tools, `platforms;android-36`, `build-tools;35.0.0` | `~/opt/android-sdk` (licenses accepted) |

Use cmdline-tools <= 20.0: newer releases replace `sdkmanager` with a native x86_64 `android` binary.

### aarch64 Linux hosts

Google ships Android build tools for Linux x86_64 only. On an ARM64 Linux host the x86_64 `aapt2`
is run under qemu-user, without changing the system:

- `~/opt/qemu/qemu-x86_64-static`: extracted from Ubuntu's `qemu-user-static` .deb (no binfmt registration)
- `~/opt/x86_64-sysroot`: Ubuntu noble amd64 `libc6` + `libgcc-s1` extracted (with `lib64 -> usr/lib64`)
- `~/opt/android-x86-tools/aapt2-8.13.0-13719691/aapt2`: the aapt2 that AGP 8.13.0 would download from Google Maven
- `~/opt/android-x86-tools/bin/{aapt2,aapt,zipalign}`: shell shims that run these under qemu
- `~/.gradle/gradle.properties`: `android.aapt2FromMavenOverride=/home/ubuntu/opt/android-x86-tools/bin/aapt2`

On x86_64 hosts, skip all of this and leave out the override.

Check an APK: `~/opt/android-x86-tools/bin/aapt2 dump badging app-debug.apk`.

## Runtime notes

- The page origin is `https://localhost` (`server.androidScheme: 'https'`, so it's a secure context).
  The sync server can still be plain `http://` (LAN or Tailscale). That works because of
  `android.allowMixedContent`, `usesCleartextTraffic="true"`, and `res/xml/network_security_config.xml`.
  The config allows cleartext everywhere and trusts user-installed CAs, so private-CA HTTPS works too.
  Requests use plain `fetch`/`WebSocket` (`CapacitorHttp` is disabled), so the server must send CORS
  headers for origin `https://localhost`. The default `OUTLINER_CORS_ORIGINS=*` already does this; if you
  restrict that list, add `https://localhost` to it.
- Keyboard: `windowSoftInputMode="adjustResize"`. In Capacitor 8, the core `SystemBars` plugin
  (`insetsHandling: 'css'`) pads the WebView by the IME inset, so `100dvh` shrinks. This keeps the
  `.mobile-bar` toolbar above the keyboard. Edge-to-edge safe areas reach CSS through `env(safe-area-inset-*)`
  and `--safe-area-inset-*`.
- `apps/web/src/platform.ts` runs only when `Capacitor.isNativePlatform()` is true. It handles the
  hardware back button: close a menu, dialog, finder or mobile sidebar first; otherwise `history.back()`;
  otherwise exit. It also sets the status/navigation bar icon style to match the theme.

## Install on a phone

1. Copy `app-debug.apk` to the phone, or use `adb install -r app-debug.apk` with USB debugging on.
2. Allow "Install unknown apps" for the file manager or browser you open it with.
3. Open Outliner > Settings > Sync server URL. The field is prefilled with `https://localhost`.
   Replace it with your server, e.g. `http://192.168.1.10:8080` or `http://outliner.tailnet.ts.net:8080`,
   then enter the password.
