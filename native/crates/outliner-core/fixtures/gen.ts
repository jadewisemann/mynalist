// Records what the TS core does on random input, so the Rust port can be checked against it
// (crates/outliner-core/tests/parity.rs). Run from the repo root after changing packages/core:
//
//   node native/crates/outliner-core/fixtures/gen.ts
//
// Deterministic: a seeded PRNG drives everything, and Date.now is replaced by a fake clock.

import { writeFileSync } from 'node:fs';
import { DocSession, ROOT, Tree, compact, type Op, type Props } from '../../../../packages/core/src/index.ts';

let seed = 0x9e3779b9;
function rand(): number {
  // mulberry32
  seed |= 0;
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
/** FNV-1a 64 over UTF-8, as hex: per-step checks stay small; the last step keeps the whole string. */
function fnv(str: string): string {
  let h = 0xcbf29ce484222325n;
  for (const b of new TextEncoder().encode(str)) h = BigInt.asUintN(64, (h ^ BigInt(b)) * 0x100000001b3n);
  return h.toString(16).padStart(16, '0');
}
const int = (n: number) => Math.floor(rand() * n);
const pick = <T>(xs: readonly T[]): T => xs[int(xs.length)]!;

let clock = 1_700_000_000_000;
Date.now = () => clock;

const TEXTS = ['', 'a', 'hello', '한글 입력', 'emoji 😀 ok', 'quote " and \\ slash', 'ctl \u0001\u001f\n\t', ' sep', 'x'.repeat(40)];

function randProps(): Props {
  const p: Props = {};
  const n = int(4);
  for (let i = 0; i < n; i++) {
    switch (int(7)) {
      case 0: p.c = pick(TEXTS); break;
      case 1: p.n = pick(TEXTS); break;
      case 2: p.cb = pick([true, false, null]); break;
      case 3: p.ck = pick([true, false, null]); break;
      case 4: p.h = pick([0, 1, 2, 3, null]); break;
      case 5: p.cl = pick([0, 1, 4, 6, null]); break;
      case 6: p.num = pick([true, false]); break;
    }
  }
  return p;
}

function randOp(t: Tree, ids: string[], fresh: () => string): Op {
  const any = () => (ids.length && rand() < 0.9 ? pick(ids) : `ghost${int(3)}`);
  const parentOf = () => (rand() < 0.3 ? ROOT : any());
  const anchor = (parent: string | null) => {
    const r = rand();
    if (r < 0.3) return null;
    const kids = parent ? t.get(parent)?.children ?? [] : [];
    if (r < 0.85 && kids.length) return pick(kids);
    return any();
  };
  const ts = clock;
  switch (int(10)) {
    case 0: case 1: case 2: {
      const parent = parentOf();
      return { t: 'create', id: rand() < 0.95 ? fresh() : any(), parent, after: anchor(parent), props: randProps(), ts };
    }
    case 3: case 4: case 5:
      return { t: 'set', id: any(), props: randProps(), ts };
    case 6: case 7: {
      if (rand() < 0.3) return { t: 'move', id: any(), parent: null, after: null, ts };
      const parent = parentOf();
      return { t: 'move', id: rand() < 0.05 ? ROOT : any(), parent, after: anchor(parent), ts };
    }
    default:
      return { t: 'collapse', id: any(), v: rand() < 0.5 };
  }
}

// ---- 1. Tree.apply ------------------------------------------------------------------------------

interface TreeCase { steps: { op: Op; inverse: Op | null; canonical: string }[]; canonical: string; final: unknown; purged: string }
const treeCases: TreeCase[] = [];
for (let c = 0; c < 30; c++) {
  const t = new Tree();
  const ids: string[] = [];
  let k = 0;
  const fresh = () => `n${c}_${k++}`;
  const steps: TreeCase['steps'] = [];
  for (let i = 0; i < 120; i++) {
    clock += int(5000);
    const op = randOp(t, ids, fresh);
    const inverse = t.apply(op);
    if (inverse && op.t === 'create') ids.push(op.id);
    steps.push({ op, inverse, canonical: fnv(t.canonical()) });
  }
  const final = t.toJSON();
  const canonicalBefore = t.canonical();
  t.purgeDetached(clock - 200_000);
  treeCases.push({ steps, canonical: canonicalBefore, final, purged: JSON.stringify(t.toJSON().nodes.map((n) => n.id).sort()) });
}

// ---- 2. DocSession: two clients through a simulated server --------------------------------------

type Action = { now?: number } & (
  | { a: 'tick'; ms: number }
  | { a: 'transact'; client: number; ops: Op[]; before: number; after: number; undoable: boolean; applied: Op[] }
  | { a: 'undo' | 'redo'; client: number; sel: number | null; undoneFirst: number | null }
  | { a: 'sync'; client: number; request: unknown; response: unknown }
  | { a: 'fail'; client: number; request: unknown }
  | { a: 'check'; client: number; canonical: string; canUndo: boolean; canRedo: boolean; dirty: boolean; snapshot: unknown });

const sessionCases: { actions: Action[]; server: string }[] = [];
for (let c = 0; c < 30; c++) {
  const server = { tree: new Tree(), log: [] as Op[][] };
  const clients = [new DocSession(), new DocSession()];
  const ids: string[] = [];
  let k = 0;
  const fresh = () => `s${c}_${k++}`;
  const actions: Action[] = [];
  let sel = 0;
  for (let i = 0; i < 150; i++) {
    const ci = int(2);
    const s = clients[ci]!;
    const r = rand();
    if (r < 0.15) {
      const ms = pick([10, 300, 900, 1500, 49 * 3600 * 1000]);
      clock += ms;
      actions.push({ now: clock, a: 'tick', ms });
    } else if (r < 0.55) {
      // typing-like single-char edits are common, so coalescing gets exercised
      let ops: Op[];
      const target = ids.length ? pick(ids) : null;
      if (target && rand() < 0.5) {
        const cur = String(s.live.get(target)?.props.c ?? '');
        const next = rand() < 0.7 ? cur + pick(['a', '가', '😀']) : Array.from(cur).slice(0, -1).join(''); // by code point: Rust strings cannot hold a lone surrogate
        ops = [{ t: 'set', id: target, props: { c: next }, ts: clock }];
      } else {
        ops = Array.from({ length: 1 + int(3) }, () => randOp(s.live, ids, fresh));
      }
      const before = sel++;
      const after = sel++;
      const undoable = rand() < 0.9;
      const applied = s.transact(ops, { before, after }, { undoable });
      for (const op of applied) if (op.t === 'create' && !ids.includes(op.id)) ids.push(op.id);
      actions.push({ now: clock, a: 'transact', client: ci, ops, before, after, undoable, applied });
    } else if (r < 0.7) {
      const kind = rand() < 0.6 ? 'undo' : 'redo';
      const got = kind === 'undo' ? s.undo() : s.redo();
      actions.push({ now: clock, a: kind, client: ci, sel: (got as number | undefined) ?? null, undoneFirst: (s.undoneFirst as number | undefined) ?? null });
    } else if (r < 0.95) {
      const req = s.beginSync();
      if (!req) continue;
      if (rand() < 0.1) {
        s.failSync();
        actions.push({ now: clock, a: 'fail', client: ci, request: req });
        continue;
      }
      const remote = server.log.slice(req.baseVersion).flat();
      if (req.ops.length) {
        for (const op of req.ops) server.tree.apply(op);
        server.log.push(req.ops);
      }
      const res = { version: server.log.length, remote };
      s.endSync(res);
      actions.push({ now: clock, a: 'sync', client: ci, request: req, response: res });
    } else {
      actions.push({ now: clock, a: 'check', client: ci, canonical: fnv(s.live.canonical()), canUndo: s.canUndo(), canRedo: s.canRedo(), dirty: s.dirty, snapshot: null });
    }
  }
  for (let ci = 0; ci < 2; ci++) {
    const s = clients[ci]!;
    actions.push({ now: clock,
      a: 'check', client: ci, canonical: s.live.canonical(), canUndo: s.canUndo(), canRedo: s.canRedo(), dirty: s.dirty,
      snapshot: s.snapshot(),
    });
  }
  sessionCases.push({ actions, server: server.tree.canonical() });
}

// Scripted: typing coalescing right at the edges of the merge window (1000 ms) and the undo TTL (48 h).
{
  const s = new DocSession();
  const actions: Action[] = [];
  const check = () => actions.push({
    now: clock, a: 'check', client: 0, canonical: s.live.canonical(), canUndo: s.canUndo(), canRedo: s.canRedo(),
    dirty: s.dirty, snapshot: s.snapshot(),
  });
  const transact = (ops: Op[]) => {
    const applied = s.transact(ops, { before: 0, after: 1 });
    actions.push({ now: clock, a: 'transact', client: 0, ops, before: 0, after: 1, undoable: true, applied });
  };
  const type = (text: string, gap: number) => {
    clock += gap;
    transact([{ t: 'set', id: 'w', props: { c: text }, ts: clock }]);
  };
  const undo = () => {
    const sel = s.undo();
    actions.push({ now: clock, a: 'undo', client: 0, sel: (sel as number | undefined) ?? null, undoneFirst: (s.undoneFirst as number | undefined) ?? null });
    check();
  };
  transact([{ t: 'create', id: 'w', parent: ROOT, after: null, props: {}, ts: clock }]);
  for (const [text, gap] of [['a', 10], ['ab', 999], ['abc', 1000], ['abcd', 1500], ['abcde', 1], ['abcd', 1], ['abc', 1]] as const) {
    type(text, gap);
  }
  for (let i = 0; i < 4; i++) undo();
  type('z', 49 * 3600 * 1000);
  clock += 49 * 3600 * 1000;
  undo(); // expired: the stack is dropped
  sessionCases.push({ actions, server: '' });
}

// ---- 3. compact -----------------------------------------------------------------------------------

const compactCases = Array.from({ length: 20 }, () => {
  const ops: Op[] = Array.from({ length: 10 }, () =>
    rand() < 0.7
      ? { t: 'set', id: pick(['a', 'b']), props: randProps(), ts: int(100) }
      : { t: 'collapse', id: 'a', v: rand() < 0.5 });
  return { ops, out: compact(ops) };
});

const out = new URL('./core.json', import.meta.url);
writeFileSync(out, JSON.stringify({ tree: treeCases, session: sessionCases, compact: compactCases }));
console.log(`wrote ${out.pathname}`);
