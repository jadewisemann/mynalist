// File tree, settings and bookmarks, stored as the special `workspace` doc.

import { DocSession, ROOT, Tx, newId, type NodeId, type TNode } from '@outliner/core';
import type { SyncClient } from './sync.ts';

export const WS_DOC = 'workspace';
const FILES = 'files';
const SETTINGS = 'settings';
const BOOKMARKS = 'bookmarks';

export type FileKind = 'folder' | 'doc';

export interface FileEntry {
  id: NodeId;
  kind: FileKind;
  title: string;
  archived: boolean;
  collapsed: boolean;
  children: FileEntry[];
}

export interface Bookmark {
  id: NodeId;
  title: string;
  doc: string;
  zoom?: string;
  q?: string;
  /** search results view: flat list of this doc, or every document */
  flat?: boolean;
  global?: boolean;
  sort?: string;
}

export class Workspace {
  readonly session: DocSession;
  private sync: SyncClient;
  constructor(sync: SyncClient) {
    this.sync = sync;
    this.session = sync.doc(WS_DOC);
  }

  /** Create the fixed top-level containers (deterministic ids, safe to race). */
  ensureStructure() {
    const t = this.session.live;
    const tx = new Tx(t);
    if (!t.get(FILES)) tx.create(ROOT, null, { title: 'files' }, FILES);
    if (!t.get(SETTINGS)) tx.create(ROOT, FILES, { title: 'settings' }, SETTINGS);
    if (!t.get(BOOKMARKS)) tx.create(ROOT, SETTINGS, { title: 'bookmarks' }, BOOKMARKS);
    this.session.commit(tx, { before: null, after: null }, { undoable: false });
  }

  onChange(fn: () => void) {
    return this.session.onChange(() => fn());
  }

  private entry(n: TNode): FileEntry {
    const t = this.session.live;
    return {
      id: n.id,
      kind: (n.props.kind as FileKind) ?? 'doc',
      title: (n.props.title as string) || 'Untitled',
      archived: !!n.props.archived,
      collapsed: n.collapsed,
      children: n.children.map((c) => this.entry(t.get(c)!)),
    };
  }

  files(): FileEntry[] {
    const f = this.session.live.get(FILES);
    return f ? f.children.map((c) => this.entry(this.session.live.get(c)!)) : [];
  }

  /** Flat list of all docs with folder path, for finders. */
  allDocs(): { id: string; title: string; path: string; archived: boolean }[] {
    const out: { id: string; title: string; path: string; archived: boolean }[] = [];
    const walk = (es: FileEntry[], path: string) => {
      for (const e of es) {
        if (e.kind === 'doc') out.push({ id: e.id, title: e.title, path, archived: e.archived });
        else walk(e.children, path ? `${path} / ${e.title}` : e.title);
      }
    };
    walk(this.files(), '');
    return out;
  }

  get(id: NodeId): FileEntry | null {
    const n = this.session.live.get(id);
    return n && this.session.live.isAttached(id) ? this.entry(n) : null;
  }

  title(id: NodeId): string {
    return (this.session.live.get(id)?.props.title as string) || 'Untitled';
  }

  private run(fn: (tx: Tx) => void, undoable = false) {
    const tx = new Tx(this.session.live);
    fn(tx);
    this.session.commit(tx, { before: null, after: null }, { undoable });
  }

  create(kind: FileKind, parent: NodeId | null, title = 'Untitled'): NodeId {
    const id = newId();
    const p = parent ?? FILES;
    this.run((tx) => {
      const kids = tx.tree.get(p)?.children ?? [];
      tx.create(p, kids[kids.length - 1] ?? null, { kind, title }, id);
      if (parent) tx.collapse(parent, false);
    });
    if (kind === 'doc') {
      // title goes in via an op: every replica must start from the same empty tree
      const doc = this.sync.doc(id);
      const tx = new Tx(doc.live);
      tx.set(ROOT, { c: title });
      doc.commit(tx, { before: null, after: null }, { undoable: false });
    }
    return id;
  }

  rename(id: NodeId, title: string) {
    this.run((tx) => tx.set(id, { title }));
    const doc = this.sync.sessions.get(id);
    if (doc && (doc.live.root().props.c ?? '') !== title) {
      const tx = new Tx(doc.live);
      tx.set(ROOT, { c: title });
      doc.commit(tx, { before: null, after: null }, { undoable: false });
    }
  }

  /** Called when a document's root title is edited in the editor. */
  syncTitleFromDoc(id: NodeId, title: string) {
    if (this.title(id) !== (title || 'Untitled')) this.run((tx) => tx.set(id, { title: title || 'Untitled' }));
  }

  move(id: NodeId, parent: NodeId | null, index: number) {
    const p = parent ?? FILES;
    this.run((tx) => {
      const kids = (tx.tree.get(p)?.children ?? []).filter((c) => c !== id);
      tx.move(id, p, index <= 0 ? null : kids[Math.min(index, kids.length) - 1]!);
    });
  }

  remove(id: NodeId) {
    this.run((tx) => tx.move(id, null, null));
  }

  setArchived(id: NodeId, archived: boolean) {
    this.run((tx) => tx.set(id, { archived: archived || null }));
  }

  /** Per-document user data (view options), stored on the file entry. */
  docPref<T>(doc: NodeId, key: string, fallback: T): T {
    const v = this.session.live.get(doc)?.props[`v_${key}`];
    if (typeof v !== 'string') return fallback;
    try {
      return JSON.parse(v) as T;
    } catch {
      return fallback;
    }
  }

  setDocPref(doc: NodeId, key: string, value: unknown) {
    this.run((tx) => tx.set(doc, { [`v_${key}`]: value === undefined || value === null ? null : JSON.stringify(value) }));
  }

  /** Folder of a file entry (null = top level). */
  parentOf(id: NodeId): NodeId | null {
    const p = this.session.live.get(id)?.parent;
    return p && p !== FILES ? p : null;
  }

  /** Position among its siblings. */
  indexOf(id: NodeId): number {
    return this.session.live.index(id);
  }

  setCollapsed(id: NodeId, v: boolean) {
    this.run((tx) => tx.collapse(id, v));
  }

  // ---- settings (synced) -----------------------------------------------------

  // Values are JSON-encoded: falsy props are dropped by the tree, but a pref
  // set to `false` must still override a `true` default.
  pref<T>(key: string, fallback: T): T {
    const v = this.session.live.get(SETTINGS)?.props[key];
    if (typeof v !== 'string') return fallback;
    try {
      return JSON.parse(v) as T;
    } catch {
      return fallback;
    }
  }

  setPref(key: string, value: unknown) {
    this.run((tx) => tx.set(SETTINGS, { [key]: value === undefined ? null : JSON.stringify(value) }));
  }

  // ---- bookmarks -------------------------------------------------------------

  bookmarks(): Bookmark[] {
    const t = this.session.live;
    return (t.get(BOOKMARKS)?.children ?? []).map((id) => {
      const p = t.get(id)!.props;
      return {
        id,
        title: (p.title as string) || 'Untitled',
        doc: p.doc as string,
        zoom: p.zoom as string | undefined,
        q: p.q as string | undefined,
        flat: !!p.flat,
        global: !!p.global,
        sort: p.sort as string | undefined,
      };
    });
  }

  addBookmark(b: Omit<Bookmark, 'id'>) {
    this.run((tx) => {
      const kids = tx.tree.get(BOOKMARKS)?.children ?? [];
      tx.create(BOOKMARKS, kids[kids.length - 1] ?? null, { ...b });
    });
  }

  moveBookmark(id: NodeId, index: number) {
    this.run((tx) => {
      const kids = (tx.tree.get(BOOKMARKS)?.children ?? []).filter((c) => c !== id);
      tx.move(id, BOOKMARKS, index <= 0 ? null : kids[Math.min(index, kids.length) - 1]!);
    });
  }

  renameBookmark(id: NodeId, title: string) {
    this.run((tx) => tx.set(id, { title }));
  }

  removeBookmark(id: NodeId) {
    this.run((tx) => tx.move(id, null, null));
  }
}
