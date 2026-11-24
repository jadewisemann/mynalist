# @outliner/server

Single-user sync server for the outliner. Node 24 runs the TypeScript directly (type stripping, no
build step); storage is `node:sqlite`; the only runtime dependency is `ws`. Protocol: see
[`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md#sync-protocol).

## Run

```sh
npm install                      # from the repo root
OUTLINER_TOKEN=$(openssl rand -hex 24) OUTLINER_PASSWORD=<your-password> npm start -w apps/server
```

## Environment

| var | default | |
|---|---|---|
| `OUTLINER_TOKEN` | — (required) | master bearer token for scripts / admin; never returned by login (devices get their own) |
| `OUTLINER_PASSWORD` | unset | password for `POST /api/login`; unset = only pairing codes. Changing it signs out every device |
| `OUTLINER_TRUST_PROXY` | on | take the client IP from `X-Forwarded-For` when the TCP peer is loopback (Tailscale Funnel/serve, Caddy on the same host); `0` to disable |
| `SERVER_PORT` / `PORT` / `OUTLINER_PORT` | `8080` | listen port |
| `OUTLINER_HOST` | `0.0.0.0` | listen address |
| `OUTLINER_DB` | `outliner.db` | sqlite file (WAL mode; back up the `-wal` file too, or use `sqlite3 .backup`) |
| `OUTLINER_STATIC` | unset | directory to serve at `/` (the web build, e.g. `apps/web/dist`), with SPA fallback |
| `OUTLINER_CORS_ORIGINS` | `*` | comma-separated allowed origins, e.g. `https://localhost,capacitor://localhost,http://localhost:5173` |
| `OUTLINER_MAX_BODY_MB` | `20` | request size limit |
| `OUTLINER_SNAPSHOT_EVERY` | `200` | write a tree snapshot every N versions |
| `OUTLINER_RETAIN_DAYS` / `OUTLINER_RETAIN_VERSIONS` | `30` / `10000` | op log is pruned only past *both* limits (and only up to the latest snapshot); older `baseVersion` gets a `reset` |
| `OUTLINER_PURGE_DETACHED_DAYS` | `30` | deleted subtrees untouched this long are dropped when snapshotting (`0` = never) |
| `OUTLINER_CACHE_DOCS` | `64` | loaded Trees kept in memory (LRU) |

## API

All under `/api`, JSON, `Authorization: Bearer <token>` except login/health.

- `POST /api/login {password, name?}` or `{code, name?}` → `{token, device}` (see [Authentication](#authentication))
- `POST /api/pair` → `{code, expires}`: one-time pairing code, valid 5 minutes
- `GET /api/devices` → `{current, devices:[{id, name, created, lastSeen, lastIp}]}`
- `DELETE /api/devices/:id` revokes one device; `DELETE /api/devices` revokes all but the caller; `POST /api/logout` revokes the caller
- `POST /api/devices {name?}` (master token only) → `{token, device}`
- `POST /api/sync {docs:[{doc, baseVersion, ops}]}` → `{docs:[{doc, version, remote} | {doc, reset:{version, tree}}]}`
- `GET /api/doc/:id` → `{version, tree}` (unknown doc → `{version: 0, tree: <empty>}`)
- `GET /api/docs` → `[{doc, version}]`
- `GET /api/health` → `{ok: true}` (no auth)
- `WS /api/ws` with subprotocols `mynalist, bearer.<base64url(token)>` (or an `Authorization` header) → server pushes `{doc, version}`; `{"ping":1}` → `{"pong":1}`

Notes on semantics:
- A whole `/api/sync` request is one sqlite transaction. Ops are validated strictly (malformed → 400,
  nothing applied); ops that `Tree.apply` drops are not logged, and a batch whose ops were all dropped
  creates no new version.
- On `reset` the batch is **not** applied; the client calls `DocSession.reset()` (which moves the
  in-flight batch back to pending) and re-sends against the new base. `reset` is returned when
  `baseVersion` predates the retained log, is ahead of the server, or names an unknown doc with
  `baseVersion > 0`.

## Authentication

- **One token per device.** Login returns a fresh random token (only its SHA-256 is stored, table
  `devices`). Settings → Sync & data lists the devices with their last use and IP; any of them can be
  signed out, which also closes its live WebSocket.
- **Pairing.** A signed-in device can show a one-time code + QR code (Settings → Sync & data → Add a
  device). The new device enters the code instead of the password; the QR code is a
  `<server>/?pair=<code>` link that signs in the web app served by this server. Codes are 50-bit,
  single use, valid 5 minutes, kept in memory only.
- **Password change.** A scrypt hash of `OUTLINER_PASSWORD` is kept in `auth_meta`; when the server
  starts with a different password, every device token is revoked.
- **WebSocket.** The token is sent as a subprotocol, never in the URL (query strings end up in proxy
  logs). The old `?token=` form is no longer accepted.
- **Rate limit.** Wrong passwords / codes: 10 per 15 min per client IP, and 100 per 15 min in total
  as a backstop. It only blocks *login*: signed-in devices keep working. Behind a proxy on the same
  host the client IP is the last `X-Forwarded-For` entry (see `OUTLINER_TRUST_PROXY`).
- **Master token.** `OUTLINER_TOKEN` still works for scripts. Clients that stored it before per-device
  tokens swap it for a device token on their next start; after updating every device, change
  `OUTLINER_TOKEN` once so old copies stop working (this also changes the calendar feed URL).
- The server URL in a build (`VITE_SYNC_SERVER`) is not a secret; the token is.

## Storage

Tables `docs(id, version, floor)`, `ops(doc, version, ts, ops)` (one row per accepted batch),
`snapshots(doc, version, tree)` (latest two kept). Loading a doc = latest snapshot + replay of later
batches. The log is pruned hourly.

## Deploy

```sh
docker build -f apps/server/Dockerfile -t outliner .      # from the repo root; also builds apps/web
docker run -d --name outliner -p 8080:8080 -v outliner-data:/data \
  -e OUTLINER_TOKEN=... -e OUTLINER_PASSWORD=... outliner
```

Put it behind a TLS-terminating reverse proxy (Caddy/nginx) that forwards WebSocket upgrades on
`/api/ws`. The Android app needs an `https://` URL.

## Test

```sh
npx vitest run apps/server       # from the repo root
```
