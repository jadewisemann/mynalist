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
OUTLINER_HOST=${OUTLINER_HOST:-0.0.0.0}
OUTLINER_DB=$HOME/.local/share/outliner/outliner.db
OUTLINER_STATIC=$ROOT/apps/web/dist
ENV
  chmod 600 ~/.config/outliner/env
  echo "Generated password: $PASS (stored in ~/.config/outliner/env)"
fi
sed "s#%h/code/outliner#$ROOT#" deploy/outliner.service > ~/.config/systemd/user/outliner.service
systemctl --user daemon-reload
systemctl --user enable --now outliner.service
loginctl enable-linger "$USER" 2>/dev/null || true
systemctl --user --no-pager status outliner.service | head -5
