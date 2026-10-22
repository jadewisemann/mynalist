import { Tree, type NodeId, type Op, type SerializedTree } from './tree.ts';

/** Editor selection snapshot stored with undo entries (opaque to core). */
export type Selection = unknown;

interface Txn {
  /** inverse ops, in the order they must be applied to undo */
  undo: Op[];
  selBefore: Selection;
  selAfter: Selection;
  ts: number;
  merge?: { id: NodeId; field: string; dir: 1 | -1 };
}

export interface DocSnapshot {
  version: number;
  confirmed: SerializedTree;
  pending: Op[];
}

export interface SyncRequest {
  baseVersion: number;
  ops: Op[];
}

export interface SyncResponse {
  /** server version after our batch (or current version if we sent nothing) */
  version: number;
  /** ops committed by others between baseVersion and our batch, in order */
  remote: Op[];
}

const UNDO_TTL = 48 * 3600 * 1000;
const MERGE_WINDOW = 1000;

export type ChangeKind = 'local' | 'remote' | 'undo';
export type Listener = (kind: ChangeKind, touched: Set<NodeId> | null) => void;

/**
 * One document on a client: live tree = confirmed (server state at `version`)
 * + inflight + pending local ops. Undo history is local and produces normal
 * ops, so undo/redo sync like any other edit.
 */
export class DocSession {
  live: Tree;
  private confirmed: Tree;
  version: number;
  private pending: Op[];
  private inflight: Op[] | null = null;
  private undoStack: Txn[] = [];
  private redoStack: Txn[] = [];
  private listeners = new Set<Listener>();

  constructor(snap?: DocSnapshot, rootProps = {}) {
    this.confirmed = snap ? Tree.fromJSON(snap.confirmed) : new Tree(rootProps, Date.now());
    this.version = snap?.version ?? 0;
    this.pending = snap ? [...snap.pending] : [];
    this.live = this.confirmed.clone();
    for (const op of this.pending) this.live.apply(op);
  }

  snapshot(): DocSnapshot {
    return {
      version: this.version,
      confirmed: this.confirmed.toJSON(),
      pending: [...(this.inflight ?? []), ...this.pending],
    };
  }

  onChange(l: Listener): () => void {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  }

  private emit(kind: ChangeKind, touched: Set<NodeId> | null) {
    for (const l of this.listeners) l(kind, touched);
  }

  get dirty(): boolean {
    return this.pending.length > 0 || this.inflight !== null;
  }

  canUndo() { return this.undoStack.length > 0; }
  canRedo() { return this.redoStack.length > 0; }

  /** Apply ops as one undoable transaction. Returns the ops that took effect. */
  transact(ops: Op[], sel: { before: Selection; after: Selection }, opts: { undoable?: boolean } = {}): Op[] {
    const applied: Op[] = [];
    const inverse: Op[] = [];
    for (const op of ops) {
      const inv = this.live.apply(op);
      if (!inv) continue;
      applied.push(op);
      inverse.unshift(inv);
    }
    this.commit({ ops: applied, inverse }, sel, opts);
    return applied;
  }

  /**
   * Record ops that were already applied to `live` (e.g. by a command Tx),
   * with their inverses in undo order.
   */
  commit(
    tx: { ops: readonly Op[]; inverse: readonly Op[] },
    sel: { before: Selection; after: Selection },
    opts: { undoable?: boolean } = {},
  ) {
    if (tx.ops.length === 0) return;
    const touched = new Set<NodeId>();
    for (const op of tx.ops) {
      this.pending.push(op);
      touched.add(op.id);
      if (op.t === 'create' || op.t === 'move') {
        const parent = this.live.get(op.id)?.parent;
        if (parent) touched.add(parent);
        if (op.t === 'move' && op.parent) touched.add(op.parent);
      }
    }
    for (const inv of tx.inverse) if (inv.t === 'move' && inv.parent) touched.add(inv.parent);
    const inverse = tx.inverse.filter((op) => op.t !== 'collapse');
    if (opts.undoable !== false && inverse.length) {
      const now = Date.now();
      const merge = mergeInfo(tx.ops, inverse);
      const top = this.undoStack[this.undoStack.length - 1];
      if (
        merge && top?.merge && now - top.ts < MERGE_WINDOW &&
        top.merge.id === merge.id && top.merge.field === merge.field && top.merge.dir === merge.dir
      ) {
        // keep the oldest "before" values: the original inverse stays, refresh time
        top.ts = now;
        top.selAfter = sel.after;
      } else {
        this.undoStack.push({ undo: inverse, selBefore: sel.before, selAfter: sel.after, ts: now, merge });
      }
      this.redoStack = [];
    }
    this.emit('local', touched);
  }

  /** Undo the last transaction; returns the selection to restore, or undefined. */
  undo(): Selection | undefined {
    return this.replay(this.undoStack, this.redoStack, 'before');
  }

  redo(): Selection | undefined {
    return this.replay(this.redoStack, this.undoStack, 'after');
  }

  private replay(from: Txn[], to: Txn[], restore: 'before' | 'after'): Selection | undefined {
    const now = Date.now();
    while (from.length && now - from[from.length - 1]!.ts > UNDO_TTL) from.length = 0;
    const txn = from.pop();
    if (!txn) return undefined;
    const inverse: Op[] = [];
    const touched = new Set<NodeId>();
    for (const op of txn.undo) {
      const stamped = op.t === 'collapse' ? op : { ...op, ts: now };
      const inv = this.live.apply(stamped);
      if (!inv) continue;
      this.pending.push(stamped);
      touchOp(this.live, stamped, inv, touched);
      inverse.unshift(inv);
    }
    to.push({ undo: inverse, selBefore: txn.selBefore, selAfter: txn.selAfter, ts: txn.ts });
    this.emit('undo', touched);
    return restore === 'before' ? txn.selBefore : txn.selAfter;
  }

  // ---- sync -------------------------------------------------------------

  /** Take pending ops for sending. Returns null if a sync is already in flight. */
  beginSync(): SyncRequest | null {
    if (this.inflight) return null;
    this.inflight = compact(this.pending);
    this.pending = [];
    return { baseVersion: this.version, ops: this.inflight };
  }

  /** Server accepted our inflight batch (after the given remote ops). */
  endSync(res: SyncResponse) {
    const sent = this.inflight ?? [];
    this.inflight = null;
    for (const op of res.remote) this.confirmed.apply(op);
    for (const op of sent) this.confirmed.apply(op);
    this.version = res.version;
    if (res.remote.length === 0) return;
    // Rebase: live = confirmed + still-pending local ops.
    this.live = this.confirmed.clone();
    for (const op of this.pending) this.live.apply(op);
    this.emit('remote', null);
  }

  /** Network failure: put the batch back in front of newer pending ops. */
  failSync() {
    if (!this.inflight) return;
    this.pending = [...this.inflight, ...this.pending];
    this.inflight = null;
  }

  /** Server says our state is unusable (e.g. history compacted); reload from snapshot. */
  reset(confirmed: SerializedTree, version: number) {
    this.confirmed = Tree.fromJSON(confirmed);
    this.version = version;
    // The server did not apply the inflight batch: it becomes pending again and
    // is re-sent against the new base (otherwise beginSync() would stay blocked).
    this.pending = [...(this.inflight ?? []), ...this.pending];
    this.inflight = null;
    this.live = this.confirmed.clone();
    for (const op of this.pending) this.live.apply(op);
    this.emit('remote', null);
  }

  purge(now = Date.now()) {
    this.confirmed.purgeDetached(now - UNDO_TTL);
  }
}

function touchOp(t: Tree, op: Op, inv: Op, touched: Set<NodeId>) {
  touched.add(op.id);
  if (op.t === 'create' || op.t === 'move') {
    const parent = t.get(op.id)?.parent;
    if (parent) touched.add(parent);
    if (inv.t === 'move' && inv.parent) touched.add(inv.parent);
  }
}

/** Single-character edit of one text field → candidate for typing coalescing. */
function mergeInfo(applied: readonly Op[], inverse: readonly Op[]): Txn['merge'] {
  if (applied.length !== 1 || inverse.length !== 1) return undefined;
  const op = applied[0]!;
  const inv = inverse[0]!;
  if (op.t !== 'set' || inv.t !== 'set') return undefined;
  const keys = Object.keys(op.props);
  if (keys.length !== 1) return undefined;
  const field = keys[0]!;
  if (field !== 'c' && field !== 'n') return undefined;
  const now = String(op.props[field] ?? '');
  const was = String(inv.props[field] ?? '');
  const d = now.length - was.length;
  if (d !== 1 && d !== -1) return undefined;
  return { id: op.id, field, dir: d as 1 | -1 };
}

/** Collapse runs of `set` ops on the same node into one (keystrokes). */
export function compact(ops: Op[]): Op[] {
  const out: Op[] = [];
  for (const op of ops) {
    const last = out[out.length - 1];
    if (op.t === 'set' && last?.t === 'set' && last.id === op.id) {
      out[out.length - 1] = { ...last, props: { ...last.props, ...op.props }, ts: op.ts };
    } else {
      out.push(op);
    }
  }
  return out;
}
