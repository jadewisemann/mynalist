// The web side of the shared render model (design spec §3): the native fixtures' documents
// (native/crates/outliner-app/tests/snapshots/*.items.jsonl) shown by the real editor and styles in the app's
// shell classes, measured into the same `[model]` text the native structural snapshot writes
// (native/crates/outliner-app/tests/snapshot.rs). Driven by qa/model.mjs: `window.__model(doc, modes, toml)`.

import '../../src/styles.css';
import '@fontsource/fira-sans/400.css';
import '@fontsource/fira-sans/400-italic.css';
import '@fontsource/fira-sans/700.css';
import '@fontsource/fira-sans/700-italic.css';
import '@fontsource/fira-mono/400.css';
import { DocSession, ROOT, Tx, extractTags, type Tree } from '@outliner/core';
import { Editor, type EditorHost } from '../../src/editor/editor.ts';
import { Sidebar, type PaneName, type SidebarHost } from '../../src/ui/sidebar.ts';
import { Workspace } from '../../src/store/workspace.ts';
import type { SyncClient } from '../../src/store/sync.ts';
import { applyAppearance, parseAppearance } from '../../src/design/appearance.ts';
import inline from '../../../../native/crates/outliner-app/tests/snapshots/inline.items.jsonl?raw';
import looks from '../../../../native/crates/outliner-app/tests/snapshots/looks.items.jsonl?raw';

const DOCS: Record<string, string> = { inline, looks };

const host: EditorHost = {
  onZoom() {},
  onTitleEdit() {},
  onAppCommand: () => false,
  openLink() {},
  onTagClick() {},
  backlinkCounts: () => null,
};

const stage = document.getElementById('stage')!;
let ed: Editor | null = null;

function show(doc: string): DocSession {
  const s = new DocSession();
  const tx = new Tx(s.live);
  const last = new Map<string, string>();
  const collapsed: string[] = [];
  for (const line of DOCS[doc]!.split('\n')) {
    if (!line.trim()) continue;
    const it = JSON.parse(line) as { id: string; parent: string | null; props: Record<string, unknown>; collapsed: boolean };
    if (it.id === ROOT) {
      tx.set(ROOT, it.props);
      continue;
    }
    tx.create(it.parent!, last.get(it.parent!) ?? null, it.props, it.id);
    last.set(it.parent!, it.id);
    if (it.collapsed) collapsed.push(it.id);
  }
  for (const id of collapsed) tx.collapse(id, true);
  s.commit(tx, { before: { k: 'none' }, after: { k: 'none' } }, { undoable: false });
  ed?.dispose();
  stage.textContent = '';
  ed = new Editor(host);
  ed.lazy = { ...ed.lazy, auto: false, zone: () => [-1e6, 1e6] };
  ed.applyPrefs({ bulletThreading: true });
  stage.append(ed.el);
  ed.setDoc(s);
  return s;
}

// ---- the sidebar: the app's, over a workspace of the fixture's document and an empty one (as the native fixture) ----

const main = document.querySelector('.main')!;
let side: { el: HTMLElement; splitter: HTMLElement; scrim: HTMLElement } | null = null;

/** App.tagCounts over `trees`: each tag as first written, how often it is used, most used first, then by name. */
function tagCounts(trees: Tree[]): [string, number][] {
  const m = new Map<string, [string, number]>();
  for (const t of trees) {
    for (const n of t.nodes.values()) {
      if (!t.isAttached(n.id)) continue;
      for (const src of [n.props.c, n.props.n]) {
        if (typeof src !== 'string' || !src) continue;
        for (const tag of extractTags(src)) {
          const k = tag.toLowerCase();
          const cur = m.get(k);
          m.set(k, [cur?.[0] ?? tag, (cur?.[1] ?? 0) + 1]);
        }
      }
    }
  }
  return [...m.values()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
}

function showSidebar(doc: DocSession, pane: PaneName | '') {
  if (side) for (const el of Object.values(side)) el.remove();
  const sessions = new Map<string, DocSession>();
  const sync = {
    sessions,
    doc(id: string) {
      if (!sessions.has(id)) sessions.set(id, new DocSession());
      return sessions.get(id)!;
    },
  } as unknown as SyncClient;
  const ws = new Workspace(sync);
  ws.ensureStructure();
  const open = ws.create('doc', null, String(doc.live.root().props.c ?? ''));
  const empty = ws.create('doc', null, 'Untitled');
  const nop = () => {};
  const host: SidebarHost = {
    currentDoc: () => open,
    openDoc: nop, openBookmark: nop, exportDoc: nop, importOpml: nop, searchTag: nop, renameTag: nop, bookmarkCurrent: nop,
    findFile: nop, gotoInbox: nop, copyDoc: nop, versionHistory: nop, share: nop, cleanNewlines: nop, dropItems: nop,
    tags: () => ({ doc: tagCounts([doc.live]), all: tagCounts([doc.live, sync.doc(empty).live]) }),
  };
  const bar = new Sidebar(ws, host);
  bar.show(pane || 'files');
  side = { el: bar.el, splitter: Object.assign(document.createElement('div'), { className: 'splitter' }), scrim: Object.assign(document.createElement('div'), { className: 'scrim' }) };
  // the drawer slides in on phones: measured where it stops
  side.el.style.transition = 'none';
  main.prepend(side.el, side.splitter, side.scrim);
  // shown on desktops from the start; on phones only when asked (the drawer)
  document.querySelector('.app')!.classList.toggle('side-open', pane !== '');
}

// ---- measuring ----

const len = (v: number) => {
  const r = Math.round(v * 2) / 2;
  return Number.isInteger(r) ? String(r === 0 ? 0 : r) : r.toFixed(1);
};
const hex = (css: string): string | null => {
  const m = /rgba?\(([^)]+)\)/.exec(css);
  if (!m) return null;
  const p = m[1]!.split(/[ ,/]+/).filter(Boolean).map(Number);
  const a = p[3] ?? 1;
  if (a === 0) return null;
  const h = (n: number) => Math.round(n).toString(16).padStart(2, '0');
  return `#${h(p[0]!)}${h(p[1]!)}${h(p[2]!)}${a < 1 ? h(a * 255) : ''}`;
};
const px = (v: string) => parseFloat(v) || 0;

/** The style of a text run as the native snapshot writes a span (family size/line weight color [decorations] [bg]):
 * the line is the text's (its paragraph's), as a native text has one line height. */
function spanStyle(el: Element, top: Element): string {
  const s = getComputedStyle(el);
  const line = px(getComputedStyle(top).lineHeight);
  let out = `${s.fontFamily} ${len(px(s.fontSize))}/${len(line)} w${s.fontWeight} ${hex(s.color)}`;
  if (s.fontStyle === 'italic') out += ' italic';
  // decorations and backgrounds of inline ancestors draw over the run too
  const lines = new Set<string>();
  let bg: string | null = null;
  let inside = true;
  for (let e: Element | null = el; e; e = e.parentElement) {
    const c = getComputedStyle(e);
    // decorations reach into inline boxes (a date chip of a checked item is struck), not into inline-blocks
    for (const l of c.textDecorationLine.split(' ')) if (l !== 'none') lines.add(l);
    if (e.classList.contains('o-content') || e.classList.contains('o-note') || c.display === 'inline-block') break;
    // the text's own box (a label, a date chip, a code block) is not a span's background
    if (e === top) inside = false;
    if (inside && !bg && c.display.startsWith('inline') && hex(c.backgroundColor)) bg = `${hex(c.backgroundColor)} r=${len(px(c.borderTopLeftRadius))}`;
  }
  if (lines.has('underline')) out += ' underline';
  if (lines.has('line-through')) out += ' strike';
  if (px(s.letterSpacing)) out += ` spacing=${len(px(s.letterSpacing))}`;
  if (bg) out += ` bg=${bg}`;
  return out;
}

interface Text { x: number; top: number; text: string; spans: [string, string][] }

/** A content's texts: its own (dates and code blocks out of it, as the native app draws them), then theirs. */
function texts(root: Element): Text[] {
  const own: Text = { x: 0, top: 0, text: '', spans: [] };
  const rest: Text[] = [];
  const push = (t: Text, s: string, style: string) => {
    t.text += s;
    const last = t.spans[t.spans.length - 1];
    if (last && last[1] === style) last[0] += s;
    else t.spans.push([s, style]);
  };
  const walk = (n: Node, into: Text, top: Element) => {
    if (n.nodeType === Node.TEXT_NODE) {
      if (n.textContent) push(into, n.textContent, spanStyle(n.parentElement!, top));
      return;
    }
    const el = n as Element;
    if (el.classList.contains('o-copy')) return;
    if (el.classList.contains('o-date') || el.classList.contains('o-codeblock')) {
      if (el.classList.contains('o-date')) push(into, '￼', spanStyle(el.parentElement!, top));
      const r = el.getBoundingClientRect();
      const c = getComputedStyle(el);
      const t: Text = { x: r.left + px(c.borderLeftWidth) + px(c.paddingLeft), top: r.top + px(c.borderTopWidth) + px(c.paddingTop), text: '', spans: [] };
      for (const k of el.childNodes) walk(k, t, el);
      rest.push(t);
      return;
    }
    if (el.classList.contains('o-latex')) {
      push(into, (el as HTMLElement).dataset.tex ?? el.textContent ?? '', spanStyle(el, top));
      return;
    }
    for (const k of el.childNodes) walk(k, into, top);
  };
  const r = root.getBoundingClientRect();
  const c = getComputedStyle(root);
  own.x = r.left + px(c.paddingLeft);
  own.top = r.top + px(c.paddingTop);
  for (const k of root.childNodes) walk(k, own, root);
  // a content that is only a code block has no text of its own
  return own.text ? [own, ...rest] : rest;
}

function writeTexts(out: string[], pad: string, ts: Text[]) {
  for (const t of ts) {
    out.push(`${pad}text x=${len(t.x)} top=${len(t.top)} ${JSON.stringify(t.text)}`);
    for (const [s, style] of t.spans) out.push(`${pad}  ${JSON.stringify(s)} ${style}`);
  }
}

interface Piece { x: number; top: number; bottom: number; w: number; color: string; elbow?: { right: number; r: number } }

/** The bullet thread's pieces (styles.css `.o-thread`: pseudo-elements of the marked rows and nodes), straight runs
 * at one x that touch merged, as the native snapshot writes them. `x` is the center of the vertical line. */
function threadPieces(): Piece[] {
  const runs: Piece[] = [];
  // a pseudo-element's box, from its containing block: the row (positioned), or the node or chunk (contained)
  const box = (el: Element, pseudo: string) => {
    const c = getComputedStyle(el, pseudo);
    if (c.content === 'none' || c.display === 'none') return null;
    const r = el.getBoundingClientRect();
    return { c, left: r.left + px(c.left), top: r.top + px(c.top), width: px(c.width), height: px(c.height) };
  };
  const straight = [
    ...[...document.querySelectorAll('.o-path:not(.o-current) > .o-row')].map((el) => box(el, '::after')),
    ...[...document.querySelectorAll('.o-above')].map((el) => box(el, '::before')),
  ];
  for (const b of straight) {
    if (!b || !b.height) continue;
    const [x, top, bottom, color] = [b.left + b.width / 2, b.top, b.top + b.height, hex(b.c.backgroundColor)!];
    const run = runs.find((r) => Math.abs(r.x - x) < 0.25 && Math.abs(r.bottom - top) <= 0.5 && r.color === color);
    if (run) run.bottom = Math.max(run.bottom, bottom);
    else runs.push({ x, top, bottom, w: b.width, color });
  }
  // runs found out of order (a row's segment after its siblings') still join
  runs.sort((a, b) => a.top - b.top);
  for (let i = 0; i < runs.length; i++) {
    const j = runs.findIndex((r, k) => k > i && Math.abs(r.x - runs[i]!.x) < 0.25 && Math.abs(runs[i]!.bottom - r.top) <= 0.5 && r.color === runs[i]!.color);
    if (j >= 0) {
      runs[i]!.bottom = Math.max(runs[i]!.bottom, runs[j]!.bottom);
      runs.splice(j, 1);
      i--;
    }
  }
  for (const el of document.querySelectorAll('.o-kids > .o-path, .o-kids > .o-chunk > .o-path')) {
    const b = box(el, '::before');
    if (!b) continue;
    const w = px(b.c.borderLeftWidth);
    runs.push({ x: b.left + w / 2, top: b.top, bottom: b.top + b.height, w, color: hex(b.c.borderLeftColor)!, elbow: { right: b.left + b.width, r: px(b.c.borderBottomLeftRadius) } });
  }
  return runs.sort((a, b) => a.top - b.top || a.x - b.x);
}

function writePiece(out: string[], p: Piece) {
  const head = `x=${len(p.x)} top=${len(p.top)} bottom=${len(p.bottom)}`;
  out.push(p.elbow ? `  elbow ${head} right=${len(p.elbow.right)} w=${len(p.w)} r=${len(p.elbow.r)} ${p.color}` : `  thread ${head} w=${len(p.w)} ${p.color}`);
}

/** The sidebar's part of the model, as the native snapshot writes it: its box, its tabs, section heads, file rows
 * and tag rows with their texts (a text's box: a flex item's, or a button's content box), and the phone scrim. */
function sidebarModel(out: string[]) {
  const el = document.querySelector('.sidebar');
  if (!el) return;
  const r = el.getBoundingClientRect();
  if (!r.width || r.right <= 0) return;
  const bgOf = (e: Element) => hex(getComputedStyle(e).backgroundColor);
  let head = `sidebar x=${len(r.left)} top=${len(r.top)} width=${len(r.width)} height=${len(r.height)}`;
  if (bgOf(el)) head += ` bg=${bgOf(el)}`;
  out.push(head);
  const kinds: [string, string][] = [['tab', 'tab'], ['pane-head', 'head'], ['file-row', 'file'], ['tag-row', 'tag']];
  for (const item of el.querySelectorAll('.tab, .pane-head, .file-row, .tag-row')) {
    const kind = kinds.find(([c]) => item.classList.contains(c))![1];
    const ir = item.getBoundingClientRect();
    let line = `  ${kind} top=${len(ir.top)} height=${len(ir.height)}`;
    if (kind === 'tab' && item.classList.contains('active')) line += ' active';
    if (bgOf(item)) line += ` bg=${bgOf(item)}`;
    out.push(line);
    const texts = kind === 'tab' ? [item] : kind === 'file' ? [item.querySelector('.name')!] : [...item.children].filter((c) => c.textContent);
    for (const t of texts) {
      const c = getComputedStyle(t);
      const tr = t.getBoundingClientRect();
      let s = t.textContent ?? '';
      if (c.textTransform === 'uppercase') s = s.toUpperCase();
      const x = tr.left + px(c.borderLeftWidth) + px(c.paddingLeft);
      const top = tr.top + px(c.borderTopWidth) + px(c.paddingTop);
      out.push(`    text x=${len(x)} top=${len(top)} ${JSON.stringify(s)}`);
      out.push(`      ${JSON.stringify(s)} ${spanStyle(t, t)}`);
    }
  }
  // the scrim shows on phones only (it has no look elsewhere)
  const scrim = document.querySelector('.app.side-open .scrim');
  if (scrim && bgOf(scrim)) {
    const sr = scrim.getBoundingClientRect();
    out.push(`scrim width=${len(sr.width)} height=${len(sr.height)} bg=${bgOf(scrim)}`);
  }
}

function model(): string {
  const out = ['[model]'];
  const thread = threadPieces();
  const bar = document.querySelector('.topbar')!.getBoundingClientRect();
  out.push(`topbar top=${len(bar.top)} height=${len(bar.height)}`);
  const title = document.querySelector('.o-title')!;
  const tr = title.getBoundingClientRect();
  out.push(`title top=${len(tr.top)} height=${len(tr.height)}`);
  writeTexts(out, '  ', texts(title.querySelector('.o-content')!));
  const nodes = [...document.querySelectorAll('.o-items .o-node')].filter((n) => n.getClientRects().length);
  for (const node of nodes) {
    const row = node.querySelector(':scope > .o-row')!;
    const rr = row.getBoundingClientRect();
    let depth = 0;
    for (let p = node.parentElement; p && !p.classList.contains('o-items'); p = p.parentElement) if (p.classList.contains('o-node')) depth++;
    let line = `row depth=${depth} top=${len(rr.top)} height=${len(rr.height)}`;
    const bullet = row.querySelector(':scope > .o-bullet')!;
    const br = bullet.getBoundingClientRect();
    line += ` bullet=${len(br.left + br.width / 2)}`;
    if (bullet.textContent) {
      const range = document.createRange();
      range.selectNodeContents(bullet);
      const t = range.getBoundingClientRect();
      line += ` number=${JSON.stringify(bullet.textContent)} number_right=${len(t.right)} number_color=${hex(getComputedStyle(bullet).color)}`;
    } else {
      const d = getComputedStyle(bullet, '::before');
      const ring = /0px 0px 0px ([\d.]+)px/.exec(d.boxShadow);
      const ringW = ring ? px(ring[1]!) : 0;
      const size = px(d.width) + 2 * ringW;
      line += ` dot=${len(size)} bg=${hex(d.backgroundColor)} r=${len(size / 2)}`;
      if (ringW) line += ` border=${len(ringW)} ${hex(/rgba?\([^)]+\)/.exec(d.boxShadow)![0])}`;
    }
    const check = row.querySelector(':scope > .o-check:not([hidden])');
    if (check) line += ` checkbox=${len(check.getBoundingClientRect().left)}`;
    const content = row.querySelector('.o-content')!;
    const cc = getComputedStyle(content);
    line += ` text=${len(content.getBoundingClientRect().left + px(cc.paddingLeft))}`;
    if (hex(cc.backgroundColor)) line += ` label=${hex(cc.backgroundColor)}`;
    if (node.classList.contains('o-current')) line += ' current';
    out.push(line);
    // the item being edited shows its source, as the native app's input does
    if (content.classList.contains('o-editing')) out.push(`  input x=${len(content.getBoundingClientRect().left + px(cc.paddingLeft))} ${JSON.stringify(content.textContent)}`);
    else writeTexts(out, '  ', texts(content));
    const note = row.querySelector('.o-note');
    if (note && note.textContent) {
      out.push(`  note top=${len(note.getBoundingClientRect().top)}`);
      writeTexts(out, '    ', texts(note));
    }
    // the thread's pieces that start in this row (its node's row and the note in it)
    for (const p of thread) if (p.top >= rr.top - 0.25 && p.top < rr.bottom - 0.25) writePiece(out, p);
  }
  sidebarModel(out);
  return out.join('\n') + '\n';
}

/** Shows `doc` in `modes` (`{ theme: 'dark', size: 'large' }`) with a user's appearance TOML, editing the item
 * `editing` (its caret at the end) when given, the sidebar on pane `side` (or as the device starts it: shown on
 * desktops, closed on phones); its model. */
(window as unknown as Record<string, unknown>).__model = async (doc: string, modes: Record<string, string>, toml: string, editing?: string, side: PaneName | '' = '') => {
  const root = document.documentElement;
  for (const k of ['theme', 'size', 'density', 'font', 'eafont']) delete root.dataset[k];
  Object.assign(root.dataset, modes);
  const look = parseAppearance(toml);
  if ('error' in look) throw new Error(`line ${look.error.line}: ${look.error.message}`);
  applyAppearance(root, look.values);
  showSidebar(show(doc), side);
  if (editing) ed!.startEdit(editing, 'c', Number.MAX_SAFE_INTEGER);
  await document.fonts.ready;
  await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  return model();
};
