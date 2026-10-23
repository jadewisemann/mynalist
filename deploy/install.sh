#!/bin/sh
# Build the web app and install the server as a systemd user service.
set -e
cd "$(dirname "$0")/.."
ROOT=$(pwd)
npm run build -w apps/web
mkdir -p ~/.config/outliner ~/.local/share/outliner ~/.config/systemd/user
if [ ! -f ~/.config/outliner/env ]; then
  PASS=${OUTLINER_PASSWORD:-$(openssl rand -base64 12)}
  cat > ~/.config/outliner/env <<ENV
OUTLINER_TOKEN=$(openssl rand -hex 24)
OUTLINER_PASSWORD=$PASS
OUTLINER_PORT=${OUTLINER_PORT:-8080}
OUTLINER_HOST=${OUTLINER_HOST:-127.0.0.1}
OUTLINER_DB=$HOME/.local/share/outliner/outliner.db
OUTLINER_STATIC=$ROOT/apps/web/dist
ENV
  chmod 600 ~/.config/outliner/env
  echo "Generated password: $PASS (stored in ~/.config/outliner/env)"
fi
# systemd's PATH lacks version-manager shims: pin the node binary
NODE=$(mise which node 2>/dev/null || command -v node)
sed -e "s#%h/code/outliner#$ROOT#" -e "s#/usr/bin/env node#$NODE#" deploy/outliner.service > ~/.config/systemd/user/outliner.service
systemctl --user daemon-reload
systemctl --user enable outliner.service
systemctl --user restart outliner.service
loginctl enable-linger "$USER" 2>/dev/null || true
systemctl --user --no-pager status outliner.service | head -5
