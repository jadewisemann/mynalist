# Architecture

Clean-room Dynalist-style outliner. Behavior specs: `01-data-model-sync.md`, `02-editor-keyboard.md`,
`03-features-ui.md` (derived from observing the original; no original code is used).

```
packages/core     shared TS (no deps): tree + ops, session/undo/sync client, editor commands,
                  inline parser, dates, search, paste/import/export
apps/server       Node 24 (type-stripping, no build) + node:sqlite + ws. Single user.
apps/web          Vite + vanilla TS editor (no framework; keyed DOM patching)
apps/android      Capacitor shell around apps/web build
```

All TS is "erasable syntax only" and imports use explicit `.ts` extensions so Node can run it directly.

## Data model

Everything is a `Tree` (`packages/core/src/tree.ts`): nodes `{id, parent, children, props, collapsed, created, edited}`
with a fixed root id `root`. Deleted nodes are *detached* (`parent: null`) and kept 48 h for undo.

- **Documents**: each document is a Tree. Root `props.c` is the document title. Item props: `c` content,
  `n` note, `cb` checkbox, `ck` checked, `h` heading 1–3, `cl` color 1–6, `num` numbered children.
  Unset = falsy; falsy values are never stored.
- **Workspace**: the file tree is itself a Tree with doc id `workspace`. Node props: `title`, `kind`
  (`'folder' | 'doc'`), `archived`. A workspace node's id **is** the document id. Bookmarks and settings
  live under two special top-level workspace nodes `bookmarks` and `settings`, created on demand
  (bookmark props: `title`, `doc`, `zoom`, `q`, `flat`; settings: one child per pref key, props `{k, v}`).
  Document rename writes both the workspace `title` and the doc root `c`.

## Ops

```
create   {id, parent, after, props, ts}
set      {id, props, ts}            shallow merge, null deletes key
move     {id, parent|null, after, ts}   parent null = delete (detach)
collapse {id, v}                    not undoable
```
`after` = previous sibling anchor (null = first child; stale anchor = append). `Tree.apply` is deterministic
and never throws: invalid ops (missing node, cycle) are dropped. Same function runs on server and clients.

## Sync protocol

Server keeps, per doc: `version` (int), an op log `(doc, version, ops[])` (one row per accepted batch),
and a periodic snapshot `(doc, version, tree)`.

`POST /api/sync` (Bearer token)
```json
{ "docs": [ { "doc": "<id>", "baseVersion": 12, "ops": [ ... ] } ] }
→ { "docs": [ { "doc": "<id>", "version": 15, "remote": [ ...ops ] } |
              { "doc": "<id>", "reset": { "version": 15, "tree": {...} } } ] }
```
Server: load tree at current version V; `remote` = concatenation of logged ops from versions
`(baseVersion, V]`; if `ops` non-empty: apply them through `Tree.apply` (dropping invalid ones is fine —
the client applies the same ops with the same function and reaches the same result), store them as batch
V+1, `version = V+1`. If `baseVersion` is older than the oldest retained log entry, reply `reset` instead.
Unknown doc with `baseVersion: 0` is created implicitly (empty tree).

Client (`DocSession`): `beginSync()` → send → `endSync({version, remote})` (or `failSync()` on network error,
`reset()` on reset). Rebase = `confirmed + remote + sent`, then `live = confirmed + still-pending`.

`GET /api/doc/:id` → `{version, tree}` (initial load). `GET /api/docs` → `[{doc, version}]`.

`WS /api/ws?token=…` → server pushes `{"doc": id, "version": n}` after every accepted batch. Clients sync
that doc if `n > local version`. Client sends `{"ping":1}` every 30 s.

Auth: single user. Server env `OUTLINER_TOKEN` (required); clients store it after a login screen.
`POST /api/login {password}` → `{token}` where password is `OUTLINER_PASSWORD`.

Client scheduling: persist locally (IndexedDB) debounced 300 ms; sync debounced 1 s after edits, every
20 s idle poll as fallback, exponential backoff (2 s → 5 min) on failure.

## Editor performance rules

- One DOM row per *visible* item, keyed by node id, patched only for touched nodes.
- Only the focused item is `contenteditable` (plaintext-only) showing raw markdown; others render HTML.
- Never re-render on keystroke: typing updates the model, and the focused element is the source of truth.
- Large docs: render visible rows lazily (windowing) once item count > ~2000.
