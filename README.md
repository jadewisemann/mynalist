# Outliner

A self-hosted, keyboard-first outliner modeled on Dynalist's behavior (clean-room: behavior specs in
`docs/01-03`, no original code or assets). Web, desktop browser (PWA) and Android, syncing through your own
server.

```
packages/core   shared logic: tree + ops, undo, sync client, editor commands, inline syntax, dates, search,
                paste/import/export (no runtime deps; unit-tested)
apps/server     Node 24 + node:sqlite + ws — op-log sync server (single user)
apps/web        Vite + vanilla TS editor
apps/android    Capacitor shell around apps/web
deploy/         systemd user service + install script
```

## Run locally

```sh
npm install
OUTLINER_TOKEN=$(openssl rand -hex 24) OUTLINER_PASSWORD=<your-password> PORT=8787 npm start -w apps/server
npm run dev -w apps/web          # http://localhost:5173 (proxies /api → :8787)
```
Open Settings → Sync server → URL `http://localhost:5173`, password → Connect. Without a server the app
works fully offline ("Local only").

## Deploy (this machine, over Tailscale)

```sh
OUTLINER_PASSWORD=<your-password> deploy/install.sh     # builds web, installs + starts outliner.service on :8080
```
Then browse to `http://<tailscale-ip>:8080` and connect to the same URL. For PWA offline support the page
must be served over HTTPS (e.g. `tailscale serve --bg 8080`).

## Android

See `apps/android/README.md`. `npm run build -w @outliner/android` → `apps/android/android/app/build/outputs/apk/debug/app-debug.apk`.
Install it, then Settings → server URL (`http://<tailscale-ip>:8080`) + password.

## Tests

```sh
npx vitest run                       # core + server unit/integration tests
npx tsc -p tsconfig.json             # typecheck
apps/web/e2e/snapshot.sh             # serve a build on :5174, then:
node apps/web/e2e/keys.mjs           # editor keyboard behavior (also drag, mobile, sync, perf, popups)
```

## Sync model

Every replica applies the same deterministic ops (`create/set/move/collapse`) with `Tree.apply`. The server
orders batches into versions; clients rebase pending local ops on top of remote ones. Offline edits queue
in IndexedDB. Details: `docs/ARCHITECTURE.md`.
