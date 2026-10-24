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
Open that URL on any device and connect to the same URL in Settings → Sync & data. Secrets live only in
`~/.config/outliner/env`; the database in `~/.local/share/outliner/outliner.db`.

### Backups

- `outliner-backup.timer` (installed by `deploy/install.sh`) copies the database every night with
  `VACUUM INTO` to `~/.local/share/outliner/backups/` and keeps the newest 14
  (`OUTLINER_BACKUP_DIR`, `OUTLINER_BACKUP_KEEP`). Run one now: `systemctl --user start outliner-backup`.
- Restore: `systemctl --user stop outliner`, copy a backup over `outliner.db` (delete `outliner.db-wal` and
  `-shm`), start it again. Clients that are ahead of the restored state get a reset and re-send their edits.
- In the app: main menu → Download backup (zip of every document as text + OPML), or `GET /api/backup`
  (JSON of every document, with the bearer token).
- Version history (document menu): the server keeps every change for `OUTLINER_RETAIN_DAYS` (30) days /
  `OUTLINER_RETAIN_VERSIONS` (10 000) versions; any of them can be viewed and restored (as an undoable edit).
- Uploaded files (`OUTLINER_FILES_DIR`, default `files/` next to the database) never change, so the timer copies
  each new one once into `backups/files/`; restore by copying them back.

### Files, sharing, calendar

- **Attachments:** paste or drop files into a document; they go to `POST /api/upload` (max
  `OUTLINER_MAX_UPLOAD_MB`, 50) and the item gets `![name](url)` (images) or `[name](url)`. Settings → Sync & data
  lists and deletes them.
- **Share…** (document or item menu) creates a public read-only link `/s/<random>`: a live, server-rendered page
  (no script, collapsible items; add `?collapsed` to start collapsed). Revoke it in the same dialog or in Settings.
- **Calendar feed:** Settings → Sync & data shows a secret `/cal/<key>.ics` URL; subscribe to it in Google Calendar
  (Other calendars → From URL), Apple Calendar or Outlook. Every unchecked dated item of a non-archived document
  becomes an event (recurrence included). Item links use the request's host, or `OUTLINER_PUBLIC_URL`.

### Security notes

- One user: a password (`POST /api/login`, 10 failures per 15 minutes per client address, then 429) hands out
  the bearer token; every other API call and the WebSocket need it. Behind Tailscale Funnel all requests come
  from 127.0.0.1, so the lockout is global: brute force is capped, at worst you wait 15 minutes to log in
  (already logged-in devices keep working).
- The token is compared in constant time; request bodies are capped (`OUTLINER_MAX_BODY_MB`, 20) and
  validated; static files are served with `nosniff`, `no-referrer` and `frame-ancestors 'none'`.
- CORS defaults to `*`, which is safe here because auth is a bearer token, not a cookie (another site can't
  use it without knowing it). Set `OUTLINER_CORS_ORIGINS` to restrict it anyway.
- Use a long random password (the installer generates one) and keep `~/.config/outliner/env` at mode 600.
- Three kinds of URL work without the token, each guarded by an unguessable random part (capability URLs):
  uploaded files `/files/<128-bit key>/…`, share pages `/s/<128-bit key>`, and the calendar feed
  `/cal/<HMAC of the token>.ics` (changes when the token changes). Anyone you give one to can read that thing only.
  Files are served with `nosniff` and a `sandbox` CSP (HTML or SVG uploads can't run script in the app's origin),
  and only images, PDF, plain text, audio and video open inline; share pages have a no-script CSP.

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
