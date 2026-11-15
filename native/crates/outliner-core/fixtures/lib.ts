// Shared by the fixture generators: a seeded PRNG and random outline trees.

import { ROOT, Tree, type NodeId, type Props } from '../../../../packages/core/src/index.ts';

let seed = 1;
export function reseed(s: number) {
  seed = s;
}
export function rand(): number {
  seed |= 0;
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
export const int = (n: number) => Math.floor(rand() * n);
export const pick = <T>(xs: readonly T[]): T => xs[int(xs.length)]!;

/** Random text from pieces (whole code points, so Rust strings hold it). */
export function textOf(pieces: readonly string[], max: number): string {
  let s = '';
  const n = int(max + 1);
  for (let i = 0; i < n; i++) s += pick(pieces);
  return s;
}

export interface TreeOptions {
  texts: () => string;
  notes?: () => string;
  /** props besides text (cb, ck, h, cl, num) */
  flags?: boolean;
}

/** A random tree of `n` items `n0..`; some collapsed, one maybe detached (deleted). */
export function randomTree(n: number, o: TreeOptions): Tree {
  const t = new Tree({ c: pick(['Title', '', 'Doc 한글', '**Bold** doc']) }, 1000);
  const ids: NodeId[] = [ROOT];
  for (let i = 0; i < n; i++) {
    const parent = rand() < 0.35 ? ROOT : pick(ids);
    const kids = t.get(parent)!.children;
    const after = kids.length && rand() < 0.7 ? pick(kids) : null;
    const p: Props = { c: o.texts() };
    if (o.notes && rand() < 0.3) p.n = o.notes();
    if (o.flags !== false) {
      if (rand() < 0.15) p.ck = true;
      if (rand() < 0.15) p.cb = true;
      if (rand() < 0.1) p.h = 1 + int(3);
      if (rand() < 0.15) p.cl = 1 + int(6);
      if (rand() < 0.1) p.num = true;
    }
    const id = `n${i}`;
    t.apply({ t: 'create', id, parent, after, props: p, ts: 1000 + int(5000) });
    ids.push(id);
  }
  for (const id of ids) if (id !== ROOT && rand() < 0.2) t.apply({ t: 'collapse', id, v: true });
  if (rand() < 0.2) t.apply({ t: 'move', id: pick(ids.slice(1)), parent: null, after: null, ts: 9000 });
  return t;
}

export const ser = (t: Tree) => structuredClone(t.toJSON());
