# 01 — Data Model, Persistence, Sync, File/Folder Structure

Behavioral spec derived from static reading of the Dynalist desktop bundle (`www/assets/js/main.min.js`) plus the public API docs (apidocs.dynalist.io). Everything here is a description of observed behavior in our own words, meant for a clean-room reimplementation. Constants are noted where they matter for UX parity.

---

## 1. Overall object hierarchy

```
User account
 └─ Userspace (one per account)
     ├─ Namespaces (sharing units; one "root namespace" = the user's own space)
     │    └─ root file (a folder) → tree of Files
     ├─ Files: Folder | Document | Linkfile (mount point for a shared namespace)
     ├─ Bookmarks (ordered flat list; the "starred"/bookmark sidebar)
     └─ per-document "user data" (view settings, archived flag, ...)
Document
 └─ NodeCollection: map id → Node, one root node, tree via parent/children
```

Two independent sync channels exist:
- **Userspace sync** (file tree, titles, bookmarks, folder collapse, per-document user data).
- **Per-document sync** (node tree inside one document). Each open/loaded document syncs on its own.

---

## 2. Identifiers

- **Node id**: 18 random bytes, base64url-encoded with padding stripped → 24 chars from `[A-Za-z0-9_-]`. Generated client-side. Unique **within a document** only (the same id may exist in another doc; when copying nodes into a doc the original id is reused if free, otherwise a fresh id is generated).
- **Local file / folder / bookmark / namespace id**: `~` + the same 24-char random string. These are *temporary local ids*.
- **Server id**: every file and bookmark also has a `server_id` assigned by the server. Until assigned, "server id" falls back to the local id. Lookups work by either id (two maps: by local id and by server id). URLs and API calls use server ids.
- **Location id (loc_id)**: per-parent slot id, same 24-char format; see §4.3.

---

## 3. Node model

### 3.1 Stored fields of a node

| Field | Meaning |
|---|---|
| `id` | node id (see §2) |
| `meta` | a compact JSON string holding all *content* properties (see 3.2). Empty string means "no properties". |
| `collapsed` | bool; whether children are hidden. Treated as per-user view state (synced separately, see §7.6). |
| `created_ts` | ms epoch; set when created locally. Preserved when nodes are cloned/moved between docs. |
| `edited_ts` | ms epoch; updated whenever `meta` changes. |
| `attached_ts` | ms epoch; updated whenever the node is moved/attached/detached. |
| `children` | ordered list of child node ids |
| `children_loc_ids` | ordered list of slot ids (one per current or former child position, see 4.3) |
| `sync_attempt` | integer; the sync-attempt counter value at which this node was last touched (optimization for diffing) |

The public API exposes the same thing flattened: `id, content, note, checked, checkbox, color (0–6), heading (0–3), created, modified, collapsed, children`.

### 3.2 The `meta` property bag (content properties)

Stored as a JSON object with **short keys**, serialized with keys sorted alphabetically, all falsy values dropped, and `{}` written as the empty string. Booleans are stored as `1` (absent = false). Integers absent = 0. Strings absent = "".

| Key | Property | Type / values |
|---|---|---|
| `c` | content (the bullet text, markdown-ish source) | string |
| `n` | note (multi-line text under the bullet) | string |
| `checkbox` | item shows a checkbox | bool (1/absent) |
| `checked` | item is checked/completed | bool (1/absent) — independent of `checkbox` (an item can be "checked"/completed without displaying a checkbox) |
| `heading_level` | heading | int 0 (none), 1, 2, 3 |
| `cl` | color label | int 0 none, 1 red, 2 orange, 3 yellow, 4 green, 5 blue, 6 purple |
| `l` | list style for the item's children | int 0 = bullets ("dot", default), 1 = numbered ("arabic") |

Color palette used by the client (light theme backgrounds): 1 `#f9d0c4`, 2 `#fed8b1`, 3 `#fef2c0`, 4 `#c8e3c0`, 5 `#c5def5`, 6 `#d4c5f9` (dark-theme variants exist).

Example meta for a checked green H2 with a note:
`{"c":"Ship v1","checked":1,"cl":4,"heading_level":2,"n":"by Friday"}`

Because the whole bag is one string, an edit of any property is a single "meta changed old→new" operation (last-writer-wins per node on the whole bag; there is no per-field merge on the wire).

Merge helper: combining two metas = shallow key union with the second winning.

### 3.3 Root node

- Every document has exactly one root node (created with the literal id `root` in a fresh local collection; the server provides its own root id via `root_node_server_id`).
- **The root node's content is the document title.** Editing the root's content renames the document (debounced ~500 ms, ignored if the new text is empty), and renaming the document from the file pane writes the new title into the root content. Collapse cannot be applied to the root.

### 3.4 Delete semantics

- Deleting a node in the editor = **detaching** it from its parent (it's not destroyed). Detached nodes remain in the local collection so undo can reattach them.
- A detached (parentless, non-root) node is purged from memory after it has been detached/unedited for **48 h** (same constant as undo expiry), checked after each successful sync.
- On the wire, a detached node simply disappears from the tree (see "removed" in §7.3).
- There is no trash for nodes; history is available via Version History (§8).

---

## 4. Tree operations (document level)

### 4.1 Primitive mutations (all go through the document controller)

1. **create_node(optional id)** — new node with created/edited/attached = now; ignored for read-only docs.
2. **edit_meta(node, newMeta)** — no-op if unchanged; sets edited_ts = now; marks doc dirty; if node is root, queue a document rename.
3. **attach_node(node, newParent|null, loc_id)** — move/attach (or detach when parent is null); sets attached_ts = now; marks old parent, new parent and node as touched; marks doc dirty.
4. **set_node_collapse(node, bool)** — view-state change; does **not** mark content dirty ("unsynced") but schedules a sync.

Everything is refused when the document's namespace permission is ≤ 1 (read only).

### 4.2 Undoable actions

Undo stores two action types:
- **EditMeta**(node, oldMeta, newMeta) — apply sets new, revert sets old.
- **Move**(node, oldParent, oldLocId, newParent, newLocId) — apply attaches at new slot, revert attaches back at old slot. Detach = Move with newParent null. Attach-at-index first generates a new slot id in the target parent at that index.

Collapse/expand is **not** in undo history.

### 4.3 Location slots ("loc ids") — position stability

Each parent keeps an ordered list of slot ids, parallel to (and a superset of) its children. When a child is detached, its slot stays in the list as an empty placeholder. Reattaching "at old slot" puts the child back between the same neighbors even if other siblings moved meanwhile: index is computed as "just after the nearest occupied slot before mine". New slots are inserted at the slot position of the child currently at the requested index (or at end). This is what makes undo of delete/move restore the right position robustly. Slots are local-only (server reports empty slot ids; they get regenerated).

### 4.4 Cloning / import

- **Clone subtree into another doc/location** (used by "move to document", "convert document to item", duplicate): depth-first copy creating nodes (reusing ids where free), copying meta, created_ts, and collapse state; returns old→new id map.
- **Import "portable nodes"** (paste / import formats): each portable item has `content, note, checkbox, checked, heading, color_label, list_style, collapsed, children[]` and is materialized under the target.

---

## 5. Undo / redo model

- One history per **document** (not global).
- A **transaction** groups any number of primitive actions plus `oldSelection` (cursor/selection before) and `newSelection` (after; defaults to current selection after execution) and a creation timestamp.
- Executing a transaction: clear redo stack, push onto undo stack, apply actions in order, restore selection.
- **Typing coalescing**: if the new transaction and the top of the undo stack are each a single meta edit on the **same node**, created within **1 s** of the previous one, and both were single-character changes in the same direction (both inserts or both deletes) and in the same field (content vs note), they merge: the top entry is replaced by a combined edit keeping the earliest old meta and earliest old selection.
- Undo: pop, revert actions in reverse order, restore old selection, push to redo. Redo: pop redo, re-apply, restore new selection, push to undo. Merge info is cleared when a transaction is undone (so a redone item won't merge further).
- **Expiry**: entries older than **48 h** are dropped; undoing an expired entry clears the undo stack; redoing an expired one clears both stacks. Expired entries are pruned after every successful sync.
- Remote changes are applied outside the undo system (they don't enter the user's history).
- UI is notified (history_change event with canUndo/canRedo).

---

## 6. File / folder / sharing model (userspace)

### 6.1 File types
- **Folder**: title, `collapsed` (folder expanded state in the file pane), ordered children (files).
- **Document**: title, per-document `user_data` (see 6.3), and a node collection.
- **Linkfile**: a placeholder in the user's tree that "mounts" a shared namespace (points at `target_namespace_id`; its effective file is that namespace's root file). Shown as the shared folder/doc.

Common file fields: `id` (local), `server_id`, `type`, `namespace_id`, `title` (empty title becomes "Untitled").

The user's tree has a single hidden **root folder**; the file pane shows its children. Root namespace's root file is this folder.

### 6.2 Namespaces (sharing)
- A namespace is a unit of sharing: `namespace_id`, `version` (integer, increases on each server change), `permission` (0 none, 1 view only, 2 can edit, 3 manage, 4 owner; default 2), `root_file`.
- Sharing a file creates a namespace whose root is that file; other users see it through a linkfile.
- All files inside a namespace subtree carry that namespace. Moving a file into another namespace re-tags it recursively.
- Namespace integrity check: client computes a SHA-256 over a BFS listing of `[server_id, comma-joined child server ids]` for each file and compares to the server's `hash`; mismatch is logged as "corrupted state".

### 6.3 Per-document user data (per-user, short keys)
| Key | Meaning |
|---|---|
| `sc` | sync dated items to Google Calendar |
| `la` | last access ts |
| `lo` | layout |
| `c` | checked-items visibility (show/hide) |
| `n` | notes visibility |
| `a` | archived (archived docs are not preloaded/kept in memory) |
| `td` | text direction |
When a document is deleted, calendar sync is turned off for every document in the deleted subtree.

### 6.4 File operations (userspace actions, each undo-able locally until synced)
- **create document / folder**: created with a local id, queued as a "new file"; optionally attached under parent at index and renamed. New docs trigger an immediate sync.
- **rename** (action `edit`): title old→new.
- **move** (action `move`): parent old/index old → parent new/index new. Refused if target is self/descendant, or if file is a namespace root. Index is zero-based.
- **delete** = move to no parent (detach). No trash.
- **folder collapse**: separate per-user change list.
- **edit document user data**: separate change list; on conflict the client merges only the keys it changed.

### 6.5 Bookmarks ("starred")
- Flat ordered list of bookmarks under an invisible root.
- Bookmark: `id`, `server_id`, `ts` (edited), `data` — a meta-style JSON with short keys:
  `t` title, `d` document server id, `z` zoomed node id, `f` focused node id, `q` search query, `flat` flat-search flag, `s` sort. I.e. a bookmark is a saved **view state** (doc + zoom + search), not just a doc.
- Operations: edit (data old→new), move (index old→new); remove = move to index −1.

### 6.6 Inbox
- Preference `inbox_location` = `"<document server id>"` or `"<document server id>/<node id>"` (doc root if no node).
- Preference `inbox_move_position` = `bottom` (default) | `top`: where captured items are inserted.
- Quick-capture (global shortcut on desktop, Pro) adds an item to that location; invalid/detached location ⇒ no inbox.
- Related pref `move_item_sent_to_position` (bottom/top) for "move to another document".

### 6.7 User preferences
Server-stored key/value (`/user/prefs/get` with `ts`, `/user/prefs/set` with `pref_key`, `data`), also cached locally. Keys include: doc_border, show_checked, tag_bg, center_doc, export_visible_only, shortcut_override, finished_onboarding, use_bullet_to_zoom, highlight_overdue, keyboard_layout, upload_permission, show_timezone, display_images_inline, move_item_sent_to_position, sync_calendar_id/name, only_use_wifi_to_sync, custom_date_display_format(_no_time), global_tag_autocomplete, unindent_in_place, auto_focus_first_item, inbox_location, inbox_move_position, show_search_tips, use_true_black, show_version_history_after, show_cleanup_newlines_option, custom_css, workflowy_shortcuts, spellchecker_language, recurrence_check_behavior (default new_instance), mobile_toolbar_options_order, email_strip_footer, right_to_left, word_count, show_backlinks.

---

## 7. Sync protocol

### 7.1 Transport
- All calls are `POST /api/<path>` with a JSON body (sent as text/plain), cookies included. A session token may be echoed back in a `DYS` response header and re-sent as a request header.
- Responses carry `_code` (`Ok` success; `NotSignedIn` → logged-out flow; `Deprecated` → client too old, sync disabled), optional `_msg`, `_b` (server build), `_mv` (minimum client version).
- Client errors mapped to `NetError` (timeout/offline/abort) or `BadResponse` (parse error). While disconnected the client pings `/ping` every 30 s.
- **Request bundling**: low-priority requests are coalesced (debounced, or immediately when 8 queued) into one `/api/bundle_binary` request whose body is `{bundle:[{path,data}]}` JSON, LZ-string compressed to bytes (`application/octet-stream`). Payloads > 100 KB use `/api/bundle_binary_large`. Responses come back as a parallel `bundle` array. The currently viewed document's requests are high priority and go unbundled.
- Default request timeout 60 s. Requests are serialized (one in flight at a time).

### 7.2 Endpoints used by the app
| Endpoint | Purpose |
|---|---|
| `/user/session`, `/user/login`, `/user/signup`, `/user/logout`, `/user/forgotpass`, `/user/resetpass`, `/user/google_login`, `/user/apple_login`, `/user/google_link`, `/loginrequest/create`, `/loginrequest/login` | auth |
| `/user/get_info`, `/user/prefs/get`, `/user/prefs/set`, `/user/cal/get` | account/prefs |
| `/file/load` | load full userspace (namespaces, files, bookmarks) |
| `/file/create` | allocate server ids for new folders/documents |
| `/file/update` | push file-tree / bookmark / collapse / user-data changes, receive remote ones |
| `/file/share/get|new|set|del` | sharing management |
| `/doc/load` | load full document |
| `/doc/update` | push document diff, receive rebase info |
| `/doc/history` | version history pages |
| `/uploads/upload_multipart`, `/uploads/get`, `/uploads/delete/` | attachments |
| `/import/workflowy`, `/notifications/updatelastread`, `/upgrade/*`, `/dropbox/start`, `/gdrive/start`, `/gcal/start`, `/auths/remove`, `/feedback`, `/ping` | misc |

Public token API (different surface, useful as a simpler model): `/api/v1/file/list`, `/file/edit` (edit|move|create), `/doc/read`, `/doc/check_for_updates` (returns version per doc), `/doc/edit` (insert|edit|move|delete with parent_id/index, index −1 = end), `/inbox/add`, `/upload`, `/pref/get|set`.

### 7.3 Document sync: snapshot-diff model

The client does **not** send an operation log. It keeps:
- `lastSync`: a snapshot (per node: id, meta, collapsed, children, slot ids, timestamps) of the state the server last acknowledged.
- the live node collection.
- `version` (server's document version, monotonically increasing integer) and `user_version` (version of the per-user state, e.g. collapse).
- sync attempt counters (`lastSyncAttempt`, `nextSyncAttempt`).

A **diff** between two snapshots contains:
- `added`: node ids present only in the new snapshot (reachable from root).
- `removed`: ids present only in the old one (detached nodes count as removed).
- `meta`: `{id, meta_old, meta_new}` for nodes whose meta string differs.
- `location`: `{id, parent_old, index_old, loc_id_old, parent_new, index_new, loc_id_new}` for nodes whose parent changed or whose relative order among the siblings common to both snapshots changed (missing parent = "" ⇒ detached).
- `collapse`: `{id, collapse_old, collapse_new}`.
- `created`: id → created_ts for added/removed nodes.
- `ts`: when the diff was made.
Optimization: when diffing for sync, nodes whose `sync_attempt` ≤ last successful attempt are assumed unchanged and skipped.

Applying a diff: ensure all referenced nodes exist (creating them with their created_ts), set metas, detach every moved node, sort moves by (parent, index) ascending, then reattach each at its index. Reverting applies the "old" side with moves sorted by old position. Diffs can be merged (union of adds/removes; for meta/location keep first old and last new) — used for history grouping.

**Wire format of a diff (`diff2`)**: build a sorted list `nodes` of every id referenced (added, removed, meta-changed, moved, and moved-to parents); everything else references nodes by their **index into that list**:
- `added: [idx…]`, `removed: [idx…]`
- `meta: [{i, meta}]` (new meta only)
- `location: [{i, parent: idx or -1, index}]` (new position only)
- `timestamps: [{i, c: created_ts}]`
Collapse changes go separately as `[{node_id, collapsed}]`.

**`/doc/update` request**: `{file_id, diff2 (or null), collapse_changes (or null), event_changes (calendar items: {id, start_ts, end_ts, allday} or {id, delete:true}), version, user_version}`.

**Responses**:
- Success: new `version`, `user_version`, optional `hash`, optional `collapse_changes` from other sessions. Client sets `lastSync` = the snapshot it sent, marks synced, prunes undo history and purges old detached nodes. If edits happened during the request, the doc is re-marked dirty.
- `try_again` with `diffs[]` + `diffs_ts[]`: the server rejected because the client was behind. **Rebase**: revert the local pending diff, apply each server diff in order (stamping edited/attached ts from the diff ts), take the result as new `lastSync` (keeping local collapse values), re-apply the local pending diff on top, then sync again immediately. Effectively: server order wins, local changes are replayed afterwards; for the same node meta, the local (later) write wins; moves of nodes that no longer exist or whose new parent is gone are dropped.
- `full_reload`: discard and reload the document from server on next tick.
- `hash`: SHA-256 hex over a JSON BFS listing `[id, "child1,child2,…", meta]` (root's meta and `{}` encoded as ""); mismatch is logged as corruption.

**`/doc/load`**: returns `nodes[]` (`id, meta, collapsed, children, created_ts, edited_ts, attached_ts`), `root_node_server_id`, `version`, `user_version`. On load with pending local edits: revert local diff, apply diff(lastSync → server state), reset lastSync, re-apply local diff (same rebase idea), and copy server timestamps.

Pushing changes from server: WebSocket push (§7.5) only signals "version N available"; the client then runs a normal `/doc/update` round (with no diff) to receive `try_again` diffs.

### 7.4 Userspace sync
- New files first: if there are unsynced new docs/folders, call `/file/create` with `{folder_count, document_count}`; server returns lists of new ids which are assigned to the pending local files (popped from the end of each list). Then continue.
- `/file/update` request: `{pv:1, o: currently open doc server id, v: userspace version, n: {namespace_id: version}, fc: file changes, b: bookmark changes, cc: folder collapse changes, dudc: doc user-data changes}`.
  - file change wire form: `{action: "edit"|"move", type, file_id, title}` or `{…, parent_id, index}` ("" parent = delete).
  - bookmark change: `{a:"e", id, d:data}` or `{a:"m", id, i:index}`.
  - collapse: `{file_id, collapsed}`; user data: `{file_id, user_data}`.
- Response: `v` (new userspace version), `fc` server-side file changes (`add|remove|move|edit` with type, file_id, namespace_id, parent_id, index, title, target_namespace_id), `nc` namespace changes (root, version, permission, hash), `nn` new namespaces (full content), `nr` removed namespaces, `cc`, `dudc`, `nb` (map local bookmark id → server id), `b` (authoritative bookmark list), `try_again`, `us_try_again`.
- Client merge strategy: revert all pending local file changes, apply server changes, then: if not `try_again`, drop the acknowledged local changes; re-apply whatever is still pending. Bookmarks: revert pending, replace with server list, re-apply pending. Same "rebase local pending ops on top of server" pattern as documents.
- `/file/load` response: `v`, `root_namespace_id`, `namespaces[]` (each with `folders`, `documents`, `linkfiles`, `root_file_server_id`, `version`, `permission`, `hash`), `b` bookmarks, `last_file_server_id` (doc to open on start).

### 7.5 Scheduling, push, backoff
- Global sync timer ticks every **20 s**. Per component: if it has pending changes → sync now. Otherwise it polls: active & focused → every tick; after 10 idle polls → every 5 ticks; unfocused/passive → every 5 ticks; passive sleeping → every 15 ticks.
- Local edit: save to local DB debounced **1 s**; remote sync debounced **5 s** (leading edge).
- Exponential backoff on failures: base 2 s, doubling, ±50% jitter, cap **5 min**.
- **Push WebSocket** (`wss://push.dynalist.io`): client sends `{watch: {"u-<user identity>":1, "n-<namespace id>":1, "d-<doc server id>":1}}`; server sends `{enabled}`, `{push:"d-…", version}` (sync if version > local), `{flush[, timeout]}` (poke all subscriptions). Client heartbeat `{ping:1}` every 30 s; reconnect if silent > 5 min. When push is connected and the component is fully synced, timer-driven polls are skipped.
- Background loading: on web, up to 5 non-archived documents are preloaded per 5 s tick once the app is synced; desktop loads all non-archived documents. Archived docs are unloaded when not current and fully synced.

### 7.6 Sync status shown to the user
States (aggregated as the worst across userspace + all documents): **Loading**, **Not saved yet**, **Saved locally**, **Synced with server**. Leaving the page with any "Not saved yet" component prompts "You have unsaved changes." Collapse/user-data changes don't downgrade status (they sync silently).

Sync is skipped in demo view, on mobile with "wifi only" on cellular, or on desktop when the env flag `disable-sync` is set.

---

## 8. Version history
- Server keeps one diff per document version, with metadata `{ts, version, user_identity}`.
- `/doc/history {file_id, version}` returns a page of older diffs (`diffs`, `diffs_meta`, `users{identity:{name,email}}`, `has_more`), walking backwards from the given version.
- Client: loads current doc, then pages backward; to view version N it applies/reverts diffs step by step. Consecutive versions by the **same user within 5 min** are grouped into one entry ("Versions a – b"), labelled with time and author. For a group it renders before/after trees, highlighting changed/added/removed nodes (unchanged branches collapsed). Viewing only — restoring is done by exporting/copying content out (export from a historical version is supported).

---

## 9. Offline / local persistence

- Offline storage is used on desktop and mobile always; on web only if enabled (`offline` flag in localStorage, or server config).
- **Web**: localforage (IndexedDB) database named `dynalist-<db user id>`; values are JSON strings compressed with LZ-string (UTF-16 variant).
- **Desktop (Electron)**: one file per key at `<appdata>/data/<user id>/<key>.data`; env settings in `<appdata>/env.json`.
- Keys:
  - `userspace` → `{userspace: {userspace_version, available, root_namespace_id, namespaces[], files[], bookmarks[]}, changes: {actions[], new_files[]}, bookmark_changes, collapse_changes, document_user_data_changes}` (pending changes are persisted so they survive restarts).
  - `document-<local document id>` → `{nodes: {version, user_version, available, nodes:[{id, meta, collapsed, created_ts, edited_ts, attached_ts, sync_attempt, children_loc_ids, children_by_loc_ids}]}, last_sync: <snapshot>, last_sync_attempt, next_sync_attempt}`. Pending changes are implicit (= diff between `last_sync` and `nodes`).
  - `userdata` → misc client state (cached preferences + `pref_ts`, last opened document id, etc.).
- On load: read local first (UI usable immediately, status "Saved locally" if pending diff), then remote.
- `localStorage["dynalist"]` (JSON object) holds: `env` (session etc. on web), `offline`, `document-zoom-levels` (per-doc zoom level).

---

## 10. Suggested clean-room simplifications
- Keep: short-key meta bag (or explicit fields), per-doc snapshot+diff sync with server version numbers and "try again / rebase" semantics, slot-based reattach for undo, 1 s typing coalescing, root-content = title, bookmarks as saved view states, inbox as `doc/node` pref.
- Can simplify: request bundling/compression, namespace hashing, calendar event_changes.
