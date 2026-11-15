// Records what the TS link helpers (packages/core/src/refs.ts) do, for crates/outliner-core/tests/refs.rs:
// parseItemUrl, backlinks across documents, and find/replace over a subtree. Run from the repo root:
//
//   node native/crates/outliner-core/fixtures/gen-refs.ts

import { writeFileSync } from 'node:fs';
import { backlinkCounts, countMatches, findBacklinks, parseItemUrl, replaceAll, ROOT, Tx, type Tree } from '../../../../packages/core/src/index.ts';
import { int, pick, rand, randomTree, reseed, ser, textOf } from './lib.ts';

reseed(0x7ef5);

const DOCS = ['docA', 'docB', 'doc C'];
const zoomRef = () => (rand() < 0.15 ? '' : `&z=n${int(12)}`);
function url(): string {
  const doc = pick(DOCS);
  const enc = doc.replace(' ', pick(['%20', '+', ' ']));
  return pick([
    `#d=${enc}${zoomRef()}`, `https://app.example/#d=${enc}${zoomRef()}`, `/x?y=1#z=n1&d=${enc}`, `#d=`, `#z=n1`,
    `#d=${enc}&d=docB${zoomRef()}`, `#?d=${enc}${zoomRef()}`, `#d=doc%41&z=n%31`, `#d=%E3%85%8B%ZZ&z=n2`, `#&&d=${enc}&z`,
    `https://e.org/page`, `#tag`, `#d=${enc}&z=root`,
  ]);
}

const WORDS = ['aaaa', 'aAa', 'apple', 'Apple', 'APPLE', 'pie', 'a.b', 'a*b', '(x)', '[y]', 'ß', 'İ', 'i', '한글', '😀', 'aa', 'aaa', 'Ab'];
function text(): string {
  const parts: string[] = [];
  const n = int(5);
  for (let i = 0; i < n; i++) {
    const r = rand();
    if (r < 0.3) parts.push(`[${pick(['link', '', 'x'])}](${url()})`);
    else if (r < 0.4) parts.push(url().startsWith('https') ? url() : `https://h.example/${url()}`);
    else parts.push(pick(WORDS));
  }
  return parts.join(' ');
}

const urls = Array.from({ length: 600 }, () => {
  const u = rand() < 0.2 ? textOf(['#', '&', '=', 'd', 'z', '%', '2', '0', '+', 'x', '?', '한', 'A'], 10) : url();
  return [u, parseItemUrl(u)];
});

const cases = Array.from({ length: 120 }, () => {
  const trees: { doc: string; tree: Tree }[] = DOCS.filter(() => rand() < 0.8).map((doc) => ({
    doc, tree: randomTree(4 + int(10), { texts: text, notes: text, flags: false }),
  }));
  const docs = trees.map(({ doc, tree }) => ({ doc, tree: ser(tree) }));
  const back = Array.from({ length: 6 }, () => {
    const doc = pick(DOCS);
    const zoom = rand() < 0.2 ? ROOT : `n${int(12)}`;
    return [doc, zoom, findBacklinks(trees, doc, zoom)];
  });
  const counts = DOCS.map((doc) => [doc, Object.fromEntries(backlinkCounts(trees, doc))]);
  const finds = trees.length ? Array.from({ length: 4 }, () => {
    const k = int(trees.length);
    const t = trees[k]!.tree;
    const ids = [...t.nodes.keys()];
    const scope = rand() < 0.4 ? ROOT : pick(ids);
    const find = rand() < 0.2 ? pick(['aa', 'AA', 'aaa']) : pick([...WORDS, '', 'a', 'p', 'n', '.', '*', '#', 'd=']);
    const replace = pick(['', 'X', '$&', '$1', 'ß', '😀']);
    const matchCase = rand() < 0.5;
    const count = countMatches(t, scope, find, matchCase);
    const copy = t.clone();
    const tx = new Tx(copy, 7000);
    const n = replaceAll(tx, scope, find, replace, matchCase);
    return { k, scope, find, replace, matchCase, count, n, ops: tx.ops };
  }) : [];
  return { docs, back, counts, finds };
});

const out = new URL('./refs.json', import.meta.url);
writeFileSync(out, JSON.stringify({ urls, cases }));
console.log(`wrote ${urls.length} URL cases and ${cases.length} document sets to ${out.pathname}`);
