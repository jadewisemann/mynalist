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
import { DocSession, ROOT, Tx } from '@outliner/core';
import { Editor, type EditorHost } from '../../src/editor/editor.ts';
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

function show(doc: string) {
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

function model(): string {
  const out = ['[model]'];
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
      line += ` number=${JSON.stringify(bullet.textContent)} number_right=${len(t.right)}`;
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
    out.push(line);
    writeTexts(out, '  ', texts(content));
    const note = row.querySelector('.o-note');
    if (note && note.textContent) {
      out.push(`  note top=${len(note.getBoundingClientRect().top)}`);
      writeTexts(out, '    ', texts(note));
    }
  }
  return out.join('\n') + '\n';
}

/** Shows `doc` in `modes` (`{ theme: 'dark', size: 'large' }`) with a user's appearance TOML; its model. */
(window as unknown as Record<string, unknown>).__model = async (doc: string, modes: Record<string, string>, toml: string) => {
  const root = document.documentElement;
  for (const k of ['theme', 'size', 'density', 'font', 'eafont']) delete root.dataset[k];
  Object.assign(root.dataset, modes);
  const look = parseAppearance(toml);
  if ('error' in look) throw new Error(`line ${look.error.line}: ${look.error.message}`);
  applyAppearance(root, look.values);
  show(doc);
  await document.fonts.ready;
  await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  return model();
};
