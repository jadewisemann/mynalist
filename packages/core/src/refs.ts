// Internal links between items: parsing app URLs, backlinks, and search & replace over a subtree.

import { extractLinks } from './inline.ts';
import { ROOT, type NodeId, type Tree } from './tree.ts';
import type { Tx } from './commands.ts';

/** `{doc, zoom}` of an app-internal URL (`#d=<doc>&z=<item>`, optionally after origin/path). */
export function parseItemUrl(url: string): { doc: string; zoom: NodeId } | null {
  const i = url.indexOf('#');
  if (i < 0) return null;
  const p = new URLSearchParams(url.slice(i + 1));
  const doc = p.get('d');
  return doc ? { doc, zoom: p.get('z') ?? ROOT } : null;
}

export interface Backlink {
  doc: string;
  id: NodeId;
}

/** Attached items (in `trees`) whose text or note links to item `zoom` of `doc`. */
export function findBacklinks(trees: Iterable<{ doc: string; tree: Tree }>, doc: string, zoom: NodeId): Backlink[] {
  const out: Backlink[] = [];
  for (const { doc: d, tree } of trees) {
    const walk = (id: NodeId) => {
      for (const c of tree.get(id)?.children ?? []) {
        const p = tree.get(c)!.props;
        const hit = [p.c, p.n].some((src) => typeof src === 'string' && src.includes('#') &&
          extractLinks(src).some((l) => {
            const u = parseItemUrl(l.url);
            return !!u && u.doc === doc && u.zoom === zoom;
          }));
        if (hit && !(d === doc && c === zoom)) out.push({ doc: d, id: c });
        walk(c);
      }
    };
    walk(ROOT);
  }
  return out;
}

/** Number of backlinks per item of `doc` (for the badge next to bullets). */
export function backlinkCounts(trees: Iterable<{ doc: string; tree: Tree }>, doc: string): Map<NodeId, number> {
  const out = new Map<NodeId, number>();
  for (const { doc: d, tree } of trees) {
    for (const n of tree.nodes.values()) {
      for (const src of [n.props.c, n.props.n]) {
        if (typeof src !== 'string' || !src.includes('#')) continue;
        for (const l of extractLinks(src)) {
          const u = parseItemUrl(l.url);
          if (u && u.doc === doc && !(d === doc && u.zoom === n.id) && tree.isAttached(n.id)) out.set(u.zoom, (out.get(u.zoom) ?? 0) + 1);
        }
      }
    }
  }
  return out;
}

const escapeRe = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** Occurrences of `find` in the text and notes of the items under `scope` (inclusive unless it is the root). */
export function countMatches(tree: Tree, scope: NodeId, find: string, matchCase: boolean): number {
  if (!find) return 0;
  const re = new RegExp(escapeRe(find), matchCase ? 'g' : 'gi');
  let n = 0;
  eachInScope(tree, scope, (id) => {
    for (const f of ['c', 'n'] as const) {
      const v = tree.get(id)!.props[f];
      if (typeof v === 'string') n += v.match(re)?.length ?? 0;
    }
  });
  return n;
}

/** Replace every occurrence in the scope; returns the number replaced. */
export function replaceAll(tx: Tx, scope: NodeId, find: string, replace: string, matchCase: boolean): number {
  if (!find) return 0;
  const re = new RegExp(escapeRe(find), matchCase ? 'g' : 'gi');
  let n = 0;
  eachInScope(tx.tree, scope, (id) => {
    const patch: Record<string, string> = {};
    for (const f of ['c', 'n'] as const) {
      const v = tx.tree.get(id)!.props[f];
      if (typeof v !== 'string') continue;
      const k = v.match(re)?.length ?? 0;
      if (!k) continue;
      n += k;
      patch[f] = v.replace(re, () => replace);
    }
    if (Object.keys(patch).length) tx.set(id, patch);
  });
  return n;
}

function eachInScope(tree: Tree, scope: NodeId, fn: (id: NodeId) => void) {
  if (scope !== ROOT) fn(scope);
  const walk = (id: NodeId) => {
    for (const c of tree.get(id)?.children ?? []) {
      fn(c);
      walk(c);
    }
  };
  walk(scope);
}
