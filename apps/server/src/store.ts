// Document storage: per-doc version + op log (one row per accepted batch) +
// periodic snapshots. Loading a doc = latest snapshot + replay of later batches.
// Loaded Trees are kept in a small in-memory LRU.

import { DatabaseSync, type StatementSync } from 'node:sqlite';
import { mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { Tree, type Op, type SerializedTree } from '@outliner/core';
import type { SyncEntry } from './validate.ts';

export interface StoreOptions {
  /** sqlite file path, or ':memory:' */
  path: string;
  /** write a snapshot every N versions */
  snapshotEvery: number;
  /** keep at least this many most recent versions of op log per doc */
  retainVersions: number;
  /** keep op log entries at least this long (ms) */
  retainMs: number;
  /** drop detached (deleted) subtrees untouched for this long when snapshotting; 0 = never */
  purgeDetachedMs: number;
  /** max number of Trees kept in memory */
  cacheSize: number;
}

export type SyncResult =
  | { doc: string; version: number; remote: Op[] }
  | { doc: string; reset: { version: number; tree: SerializedTree } };

export interface DocVersion {
  doc: string;
  version: number;
}

interface Loaded {
  id: string;
  tree: Tree;
  version: number;
  /** op log rows with version <= floor have been pruned */
  floor: number;
  /** version of the latest snapshot (0 = none) */
  snap: number;
}

interface DocRow {
  id: string;
  version: number;
  floor: number;
}

export class Store {
  readonly db: DatabaseSync;
  private readonly opts: StoreOptions;
  private readonly cache = new Map<string, Loaded>();
  private readonly q: Record<string, StatementSync>;

  constructor(opts: StoreOptions) {
    this.opts = opts;
    if (opts.path !== ':memory:') mkdirSync(dirname(opts.path), { recursive: true });
    this.db = new DatabaseSync(opts.path);
    this.db.exec(`
      PRAGMA journal_mode = WAL;
      PRAGMA synchronous = NORMAL;
      PRAGMA busy_timeout = 5000;
      CREATE TABLE IF NOT EXISTS docs (
        id      TEXT PRIMARY KEY,
        version INTEGER NOT NULL,
        floor   INTEGER NOT NULL DEFAULT 0,
        created INTEGER NOT NULL,
        updated INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS ops (
        doc     TEXT NOT NULL,
        version INTEGER NOT NULL,
        ts      INTEGER NOT NULL,
        ops     TEXT NOT NULL,
        PRIMARY KEY (doc, version)
      ) WITHOUT ROWID;
      CREATE TABLE IF NOT EXISTS snapshots (
        doc     TEXT NOT NULL,
        version INTEGER NOT NULL,
        ts      INTEGER NOT NULL,
        tree    TEXT NOT NULL,
        PRIMARY KEY (doc, version)
      ) WITHOUT ROWID;
    `);
    const p = (sql: string) => this.db.prepare(sql);
    this.q = {
      docGet: p('SELECT id, version, floor FROM docs WHERE id = ?'),
      docList: p('SELECT id AS doc, version FROM docs ORDER BY id'),
      docAll: p('SELECT id, version, floor FROM docs'),
      docInsert: p('INSERT INTO docs (id, version, floor, created, updated) VALUES (?, 0, 0, ?, ?)'),
      docUpdate: p('UPDATE docs SET version = ?, updated = ? WHERE id = ?'),
      docFloor: p('UPDATE docs SET floor = ? WHERE id = ?'),
      opsRange: p('SELECT ops FROM ops WHERE doc = ? AND version > ? AND version <= ? ORDER BY version'),
      opsInsert: p('INSERT INTO ops (doc, version, ts, ops) VALUES (?, ?, ?, ?)'),
      opsFirstRecent: p('SELECT MIN(version) AS v FROM ops WHERE doc = ? AND ts > ?'),
      opsPrune: p('DELETE FROM ops WHERE doc = ? AND version <= ?'),
      snapLatest: p('SELECT version, tree FROM snapshots WHERE doc = ? ORDER BY version DESC LIMIT 1'),
      snapInsert: p('INSERT OR REPLACE INTO snapshots (doc, version, ts, tree) VALUES (?, ?, ?, ?)'),
      snapPrune: p('DELETE FROM snapshots WHERE doc = ? AND version < ?'),
      snapAt: p('SELECT version, tree FROM snapshots WHERE doc = ? AND version <= ? ORDER BY version DESC LIMIT 1'),
      snapFloor: p('SELECT MAX(version) AS v FROM snapshots WHERE doc = ? AND version <= ?'),
      opsTimes: p('SELECT version, ts FROM ops WHERE doc = ? AND version > ? ORDER BY version'),
    };
  }

  close() {
    this.cache.clear();
    if (this.db.isOpen) this.db.close();
  }

  /** Load (or fetch from cache) a doc's tree at its current version. */
  private load(id: string): Loaded | null {
    const hit = this.cache.get(id);
    if (hit) {
      this.cache.delete(id);
      this.cache.set(id, hit);
      return hit;
    }
    const row = this.q.docGet!.get(id) as DocRow | undefined;
    if (!row) return null;
    const snapRow = this.q.snapLatest!.get(id) as { version: number; tree: string } | undefined;
    const tree = snapRow ? Tree.fromJSON(JSON.parse(snapRow.tree) as SerializedTree) : new Tree();
    const snap = snapRow?.version ?? 0;
    for (const op of this.readOps(id, snap, row.version)) tree.apply(op);
    const doc: Loaded = { id, tree, version: row.version, floor: row.floor, snap };
    this.remember(doc);
    return doc;
  }

  private remember(doc: Loaded) {
    this.cache.set(doc.id, doc);
    while (this.cache.size > Math.max(1, this.opts.cacheSize)) {
      this.cache.delete(this.cache.keys().next().value!);
    }
  }

  /** Concatenated ops of batches (from, to]. */
  private readOps(id: string, from: number, to: number): Op[] {
    if (to <= from) return [];
    const out: Op[] = [];
    for (const r of this.q.opsRange!.all(id, from, to) as { ops: string }[]) {
      for (const op of JSON.parse(r.ops) as Op[]) out.push(op);
    }
    return out;
  }

  getDoc(id: string): { version: number; tree: SerializedTree } {
    const d = this.load(id);
    if (!d) return { version: 0, tree: new Tree().toJSON() };
    return { version: d.version, tree: d.tree.toJSON() };
  }

  listDocs(): DocVersion[] {
    return this.q.docList!.all() as unknown as DocVersion[];
  }

  /**
   * Points in the retained history of a doc, newest first: the last version of each burst of edits
   * (batches less than `gapMs` apart are one burst).
   */
  history(id: string, gapMs = 5 * 60 * 1000, max = 500): { version: number; ts: number; edits: number }[] {
    const row = this.q.docGet!.get(id) as DocRow | undefined;
    if (!row) return [];
    const rows = this.q.opsTimes!.all(id, row.floor) as { version: number; ts: number }[];
    const out: { version: number; ts: number; edits: number }[] = [];
    for (const r of rows) {
      const last = out[out.length - 1];
      if (last && r.ts - last.ts < gapMs) Object.assign(last, { version: r.version, ts: r.ts, edits: last.edits + 1 });
      else out.push({ version: r.version, ts: r.ts, edits: 1 });
    }
    return out.reverse().slice(0, max);
  }

  /** The doc as it was at `version`, or null when that version is outside the retained history. */
  treeAt(id: string, version: number): SerializedTree | null {
    const row = this.q.docGet!.get(id) as DocRow | undefined;
    if (!row || version < row.floor || version > row.version) return null;
    const snapRow = this.q.snapAt!.get(id, version) as { version: number; tree: string } | undefined;
    if (!snapRow && row.floor > 0) return null;
    const tree = snapRow ? Tree.fromJSON(JSON.parse(snapRow.tree) as SerializedTree) : new Tree();
    for (const op of this.readOps(id, snapRow?.version ?? 0, version)) tree.apply(op);
    return tree.toJSON();
  }

  /** Cached versions (for tests / diagnostics). */
  cachedDocs(): string[] {
    return [...this.cache.keys()];
  }

  /**
   * Process a sync batch atomically: all DB writes happen in one transaction.
   * Returns per-doc results plus the docs whose version advanced.
   */
  sync(entries: SyncEntry[]): { results: SyncResult[]; changed: DocVersion[] } {
    const now = Date.now();
    const results: SyncResult[] = [];
    const changed: DocVersion[] = [];
    // Docs whose in-memory state may have changed; evicted if the transaction fails.
    const touched: string[] = [];
    this.db.exec('BEGIN IMMEDIATE');
    try {
      for (const e of entries) {
        touched.push(e.doc);
        let d = this.load(e.doc);
        if (!d) {
          if (e.baseVersion !== 0) {
            // Client believes in history we don't have: start it over from empty.
            results.push({ doc: e.doc, reset: { version: 0, tree: new Tree().toJSON() } });
            continue;
          }
          if (e.ops.length === 0) {
            results.push({ doc: e.doc, version: 0, remote: [] });
            continue;
          }
          this.q.docInsert!.run(e.doc, now, now);
          d = { id: e.doc, tree: new Tree(), version: 0, floor: 0, snap: 0 };
          this.remember(d);
        }
        if (e.baseVersion > d.version || e.baseVersion < d.floor) {
          // Ahead of us (server restored from backup) or older than retained log.
          // The batch is NOT applied; the client re-sends it against the new base.
          results.push({ doc: e.doc, reset: { version: d.version, tree: d.tree.toJSON() } });
          continue;
        }
        const remote = this.readOps(e.doc, e.baseVersion, d.version);
        if (e.ops.length) {
          // Ops dropped by Tree.apply had no effect here and will have none on any
          // other replica (same state, same function), so only effective ops are logged.
          const applied = e.ops.filter((op) => d.tree.apply(op) !== null);
          if (applied.length) {
            const version = ++d.version;
            this.q.opsInsert!.run(e.doc, version, now, JSON.stringify(applied));
            this.q.docUpdate!.run(version, now, e.doc);
            if (version - d.snap >= this.opts.snapshotEvery) {
              if (this.opts.purgeDetachedMs > 0) d.tree.purgeDetached(now - this.opts.purgeDetachedMs);
              // older snapshots stay until prune(): they are the bases for version history
              this.q.snapInsert!.run(e.doc, version, now, JSON.stringify(d.tree.toJSON()));
              d.snap = version;
            }
            changed.push({ doc: e.doc, version });
          }
        }
        results.push({ doc: e.doc, version: d.version, remote });
      }
      this.db.exec('COMMIT');
    } catch (err) {
      if (this.db.isTransaction) this.db.exec('ROLLBACK');
      for (const id of touched) this.cache.delete(id);
      throw err;
    }
    return { results, changed };
  }

  /**
   * Drop op log rows that are older than the retention window in BOTH age and
   * version count, and already covered by a snapshot. Returns rows removed.
   */
  prune(now = Date.now()): number {
    let removed = 0;
    const cutoff = now - this.opts.retainMs;
    for (const row of this.q.docAll!.all() as unknown as DocRow[]) {
      const snapRow = this.q.snapLatest!.get(row.id) as { version: number } | undefined;
      let limit = Math.min(snapRow?.version ?? 0, row.version - this.opts.retainVersions);
      if (limit <= row.floor) continue;
      const recent = (this.q.opsFirstRecent!.get(row.id, cutoff) as { v: number | null }).v;
      if (recent !== null) limit = Math.min(limit, recent - 1);
      // the floor always sits on a snapshot, so every retained version can be rebuilt (version history)
      limit = (this.q.snapFloor!.get(row.id, limit) as { v: number | null }).v ?? 0;
      if (limit <= row.floor) continue;
      this.db.exec('BEGIN IMMEDIATE');
      try {
        removed += Number(this.q.opsPrune!.run(row.id, limit).changes);
        this.q.snapPrune!.run(row.id, limit);
        this.q.docFloor!.run(limit, row.id);
        this.db.exec('COMMIT');
      } catch (err) {
        this.db.exec('ROLLBACK');
        throw err;
      }
      const c = this.cache.get(row.id);
      if (c) c.floor = limit;
    }
    return removed;
  }
}
