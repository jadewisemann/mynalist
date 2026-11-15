// Records what the TS exporters (packages/core/src/export.ts) and the inline HTML renderer (renderInline in
// inline.ts) write, for crates/outliner-core/tests/export.rs. Run from the repo root:
//
//   node native/crates/outliner-core/fixtures/gen-export.ts
//
// Rendered date labels depend on the local time zone: the render cases are recorded in several zones.

import { writeFileSync } from 'node:fs';
import {
  escapeXmlAttr, exportHtml, exportOpml, exportText, opmlToPortable, portableFromTree, portableToHtml, portableToOpml,
  portableToText, renderInline, ROOT, type ExportScope, type NodeId,
} from '../../../../packages/core/src/index.ts';
import { int, pick, rand, randomTree, reseed, ser, textOf } from './lib.ts';

reseed(0xe490);

const PIECES = [
  'a', 'text', ' ', '\n', '한글', '😀', '**', '__', '~~', '==', '`', '```js\n', '```', '$$', '\\', '\\*', '[', ']', '(', ')',
  '#tag', '@me', 'https://x.org/a?b=1&c=<d>', 'mailto:me@x.org', '"q"', "'s'", '<b>', '&amp;', '!(2026-10-22)',
  '!(2026-10-22 10:00 | ~2w135)', '!(2026-10-20 - 2026-10-23)', '[t](https://e.org)', '[](#d=doc&z=n1)', '[https://u.org](https://u.org)',
  '[x](javascript:alert(1))', '![alt](img.png)', '![](data:image/png;base64,AA)', '![a](javascript:x)', '`c`', '$$x^2$$', '\t',
];
const markup = () => textOf(PIECES, 10);

const include = (id: NodeId) => Number(id.slice(1)) % 3 !== 0;

const trees = Array.from({ length: 150 }, () => {
  const tree = randomTree(3 + int(12), { texts: markup, notes: () => pick(['', 'note', 'two\nlines', markup()]) });
  const attached = [...tree.nodes.keys()].filter((id) => id !== ROOT && tree.isAttached(id));
  const sel: NodeId[] = rand() < 0.3 ? tree.get(ROOT)!.children.slice() : attached.filter(() => rand() < 0.4);
  if (rand() < 0.1) sel.push('missing');
  const scope: ExportScope = {};
  const sc: Record<string, unknown> = {};
  if (rand() < 0.6) {
    scope.visibleOnly = sc.visibleOnly = true;
    if (rand() < 0.5) scope.showChecked = sc.showChecked = rand() < 0.5;
    if (rand() < 0.6) scope.notes = sc.notes = pick(['show', 'first', 'hide'] as const);
    if (rand() < 0.3) { scope.include = include; sc.include = true; }
  }
  const text = { indent: pick([undefined, 'spaces', 'asterisks', 'dashes', 'none'] as const), includeNotes: pick([undefined, true, false]), topNumbered: pick([undefined, undefined, true, false]) };
  const html = { includeNotes: pick([undefined, true, false]), topNumbered: pick([undefined, undefined, true, false]), document: rand() < 0.3, title: pick([undefined, 'T <&> "x"', '한글']) };
  const opml = { title: pick([undefined, 'Doc & <co>']), source: pick([undefined, '', 'https://x.org/?a=1&b']), ownerName: pick([undefined, 'Me <me>']), ownerEmail: pick([undefined, 'a@b.c']) };
  const items = portableFromTree(tree, sel, scope);
  const xml = exportOpml(tree, sel, { ...scope, ...opml });
  return {
    tree: ser(tree), sel, scope: sc, text, html, opml,
    portable: items,
    exportText: exportText(tree, sel, { ...scope, ...text }),
    exportHtml: exportHtml(tree, sel, { ...scope, ...html }),
    exportOpml: xml,
    toText: portableToText(items, text),
    toHtml: portableToHtml(items, html),
    toOpml: portableToOpml(items, opml),
    roundTrip: opmlToPortable(xml),
  };
});

const attrs = Array.from({ length: 300 }, () => {
  const s = textOf(['a', '&', '<', '>', '"', "'", '\n', '\r', '\t', '한', '😀'], 8);
  return [s, escapeXmlAttr(s)];
});

const linkTitle = (url: string) => (url.length % 2 ? `Title of ${url}` : undefined);
const isInternal = (url: string) => url.startsWith('#');
const PATTERNS = [undefined, 'YYYY-MM-DD', 'relative', 'MMM D, HH:mm'];

const ZONES = ['UTC', 'America/New_York', 'Asia/Seoul'];
const zones: Record<string, unknown[]> = {};
for (const z of ZONES) {
  process.env.TZ = z;
  zones[z] = Array.from({ length: 1500 }, () => {
    const src = markup();
    const o = {
      now: Date.UTC(2026, 9, 22, 2, 0) + Math.floor((rand() - 0.5) * 20 * 86400000),
      hour12: pick([undefined, true, false]),
      dateFormat: pick(PATTERNS),
      dateFormatNoTime: pick(PATTERNS),
      markOverdue: rand() < 0.5,
      inlineImages: rand() < 0.5,
      highlight: rand() < 0.5 ? [] : [pick(['a', 'x', 'org', '', 'tag', '한', 'T', 'e.o'])],
      clean: rand() < 0.25,
      callbacks: rand() < 0.5,
    };
    const html = renderInline(src, { ...o, isInternal: o.callbacks ? isInternal : undefined, linkTitle: o.callbacks ? linkTitle : undefined });
    return { src, o, html };
  });
}

const out = new URL('./export.json', import.meta.url);
writeFileSync(out, JSON.stringify({ trees, attrs, zones }));
console.log(`wrote ${trees.length} export cases and ${ZONES.length} zones of render cases to ${out.pathname}`);
