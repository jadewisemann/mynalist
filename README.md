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

## Deploy (this machine, public HTTPS via Tailscale Funnel)

```sh
OUTLINER_PASSWORD=<your-password> deploy/install.sh     # builds web, installs + starts outliner.service on 127.0.0.1:8080
sudo tailscale funnel --bg 8080                          # https://<machine>.<tailnet>.ts.net (public)
```
Open that URL on any device and connect to the same URL in Settings → Sync server. Secrets live only in
`~/.config/outliner/env`; the database in `~/.local/share/outliner/outliner.db`.

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

### Dynalist parity

`qa/` runs the same scenarios on the public Dynalist demo and on this build and compares the results side
by side. It covers about 340 editing scenarios, a random-key fuzzer, inline rendering and layout metrics.
Dynalist's results are recorded in `qa/golden/`, so the default run is offline and works as a
regression test (CI runs it):

```sh
npm run qa:serve && npm run qa       # replay against the recording; exits 1 on a regression
npm run qa:live                      # both apps live, side by side (needs dynalist.io)
npm run qa:record                    # refresh the recording from Dynalist
```

Reports are written to `qa/out/<suite>/report.html`. See [qa/PLAN.md](qa/PLAN.md) for the method,
tolerances and known differences.

## Sync model

Every replica applies the same deterministic ops (`create/set/move/collapse`) with `Tree.apply`. The server
orders batches into versions; clients rebase pending local ops on top of remote ones. Offline edits queue
in IndexedDB. Details: `docs/ARCHITECTURE.md`.
