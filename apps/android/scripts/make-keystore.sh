#!/bin/sh
# Create a release keystore + signing properties OUTSIDE the repo (default ~/.config/outliner), random passwords.
# Back both files up somewhere safe: an app update must be signed with the same key.
set -e
DIR="${1:-$HOME/.config/outliner}"
KS="$DIR/outliner-release.jks"
PROPS="$DIR/android-signing.properties"
[ -e "$KS" ] && { echo "$KS already exists" >&2; exit 1; }
mkdir -p "$DIR"
umask 077
PW=$(openssl rand -base64 24 | tr -d '/+=')
keytool -genkeypair -keystore "$KS" -storetype PKCS12 -storepass "$PW" -keypass "$PW" \
  -alias outliner -keyalg RSA -keysize 4096 -validity 10000 -dname "CN=Outliner"
printf 'storeFile=%s\nstorePassword=%s\nkeyAlias=outliner\nkeyPassword=%s\n' "$KS" "$PW" "$PW" > "$PROPS"
chmod 600 "$KS" "$PROPS"
echo "created $KS and $PROPS"
