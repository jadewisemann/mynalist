#!/bin/sh
# Build a snapshot of the web app and serve it on :5174 (immune to dev-server reloads).
set -e
cd "$(dirname "$0")/.."
npx vite build --outDir /tmp/websnap --emptyOutDir >/dev/null
pkill -f "vite preview --outDir /tmp/websnap" 2>/dev/null || true
nohup npx vite preview --outDir /tmp/websnap --port 5174 --strictPort >/tmp/websnap.log 2>&1 &
sleep 2
