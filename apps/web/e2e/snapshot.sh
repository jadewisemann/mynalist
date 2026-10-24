#!/bin/sh
# Build a snapshot of the web app and serve it on :5174 (immune to dev-server reloads).
set -e
cd "$(dirname "$0")/.."
npx vite build --outDir /tmp/websnap --emptyOutDir >/dev/null
pkill -f "vite preview --outDir /tmp/websnap" 2>/dev/null && sleep 0.5 || true
nohup npx vite preview --outDir /tmp/websnap --port 5174 --strictPort >/tmp/websnap.log 2>&1 &
# wait until it answers (slow CI machines)
for _ in $(seq 60); do
  curl -sf -o /dev/null http://localhost:5174/ && exit 0
  sleep 0.5
done
echo "snapshot server did not start" >&2; cat /tmp/websnap.log >&2; exit 1
