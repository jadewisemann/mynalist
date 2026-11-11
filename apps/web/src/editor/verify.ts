// Render checker: compares what the editor's DOM shows with what the model says it must show.
// Used by the render tests (test/render) after every step; independent of how editor.ts renders, so it keeps
// holding while the renderer is optimized (chunked containers, lazily created parts, virtualization).

import { type NodeId, ROOT, type Tree, type View, visibleChildren } from '@outliner/core';
import { getText } from './caret.ts';
import type { Editor, Row } from './editor.ts';
import { renderText } from './render.ts';

/** Elements that group rows without being items (chunks of a long children list). */
export const GROUP_CLASS = 'o-chunk';

/** Blocks whose rows aren't made (yet): empty, standing for their items (lazy rendering). */
export const PENDING_CLASS = 'o-pending';

/** A position in a children list: the item's element, or null where the item stands in a pending chunk. */
export interface Slot {
  id: NodeId;
  el: HTMLElement | null;
}

/** The items of a children container in order, looking through chunks (pending chunks give their ids). */
export function itemSlots(container: Element, pendingIds: (chunk: HTMLElement) => readonly NodeId[] | undefined, bad?: (msg: string) => void): Slot[] {
  const out: Slot[] = [];
  for (const el of container.children as HTMLCollectionOf<HTMLElement>) {
    if (el.classList.contains(GROUP_CLASS) && el.classList.contains(PENDING_CLASS)) {
      const ids = pendingIds(el);
      if (!ids?.length) bad?.('a pending chunk stands for no items');
      if (el.firstElementChild) bad?.('a pending chunk has elements');
      for (const id of ids ?? []) out.push({ id, el: null });
    } else if (el.classList.contains(GROUP_CLASS)) out.push(...itemSlots(el, pendingIds, bad));
    else if (el.classList.contains('o-node')) out.push({ id: el.dataset.id ?? '?', el });
    else bad?.(`unexpected element in a children container: <${el.tagName.toLowerCase()} class="${el.className}">`);
  }
  return out;
}

const scratch = document.createElement('template');
/** HTML as the DOM serializes it back (attribute quoting, entities). */
function normalized(html: string): string {
  scratch.innerHTML = html;
  return scratch.innerHTML;
}

const str = (v: unknown) => (typeof v === 'string' ? v : '');
const hasEnhanced = (src: string) => src.includes('```') || src.includes('$$');

function isShut(t: Tree, v: View, id: NodeId): boolean {
  const n = t.get(id);
  if (!n?.children.length) return false;
  return v.filter ? visibleChildren(t, id, v).length === 0 : n.collapsed;
}

/** Problems found (empty: the DOM matches the model). */
export function verifyRender(ed: Editor, limit = 40): string[] {
  const errs: string[] = [];
  const bad = (m: string) => {
    if (errs.length < limit) errs.push(m);
  };
  const x = ed.debugInternals();
  const t = ed.tree;
  const v = ed.view;
  const ed$ = x.editing;
  const name = (id: NodeId) => `${id}(${JSON.stringify(str(t.get(id)?.props.c).slice(0, 24))})`;
  const hl = (id: NodeId) => (x.searchHl?.ids.has(id) ? { highlight: x.searchHl.terms } : undefined);

  const field = (r: Readonly<Row>, id: NodeId, f: 'c' | 'n', el: HTMLElement, cached: string | null) => {
    const want = str(t.get(id)?.props[f]);
    if (ed$ && ed$.el === el) {
      if (getText(el) !== want) bad(`${name(id)} ${f}: edited text ${JSON.stringify(getText(el))} != model ${JSON.stringify(want)}`);
      return;
    }
    if (el.isContentEditable) bad(`${name(id)} ${f}: editable but not the edited field`);
    if (cached !== want) bad(`${name(id)} ${f}: cache ${JSON.stringify(cached)} != model ${JSON.stringify(want)}`);
    if (!hasEnhanced(want) && el.innerHTML !== normalized(renderText(want, hl(id)))) bad(`${name(id)} ${f}: rendered HTML is stale`);
    void r;
  };

  const checkRow = (r: Readonly<Row>, id: NodeId, num: number, isTitle: boolean) => {
    const n = t.get(id)!;
    const p = n.props;
    const cls = r.el.classList;
    if (r.el.dataset.id !== id) bad(`${name(id)}: data-id ${r.el.dataset.id}`);
    if (r.el.firstElementChild !== r.row) bad(`${name(id)}: row is not the first child`);
    const expect = (c: string, on: boolean, el: Element = r.el) => {
      if (el.classList.contains(c) !== on) bad(`${name(id)}: class ${c} should be ${on ? 'on' : 'off'}`);
    };
    expect('o-checked', !!p.ck);
    expect('o-has-kids', n.children.length > 0);
    expect('o-collapsed', !isTitle && isShut(t, v, id));
    expect('o-numbered', num > 0);
    for (const h of [1, 2, 3]) expect(`o-h${h}`, p.h === h);
    for (const c of [1, 2, 3, 4, 5, 6]) expect(`o-cl${c}`, p.cl === c, r.content);
    expect('o-current', !!ed$ && ed$.id === id);
    if (!isTitle) expect('o-selected', !!x.block?.ids.includes(id));
    if (!cls.contains(isTitle ? 'o-title' : 'o-node')) bad(`${name(id)}: missing ${isTitle ? 'o-title' : 'o-node'}`);

    // row parts, in order: [menu, zoom,] badge?, bullet, check?, body
    const parts = [...r.row.children].filter((e) => !e.matches('.o-menu-btn, .o-zoom-btn'));
    const want = [r.badge, r.bullet, r.check, r.body].filter(Boolean);
    if (parts.length !== want.length || parts.some((e, i) => e !== want[i])) {
      bad(`${name(id)}: row parts [${parts.map((e) => e.className).join(' | ')}]`);
    }
    if (r.bullet.textContent !== (num > 0 ? `${num}.` : '')) bad(`${name(id)}: bullet "${r.bullet.textContent}", want number ${num}`);
    if (!r.bullet.getAttribute('href')?.endsWith(`#z=${id}`)) bad(`${name(id)}: bullet href ${r.bullet.getAttribute('href')}`);
    const ck = r.check;
    if (p.cb) {
      if (!ck || ck.hidden) bad(`${name(id)}: checkbox not shown`);
      else if (ck.classList.contains('o-on') !== !!p.ck) bad(`${name(id)}: checkbox state`);
    } else if (ck && !ck.hidden) bad(`${name(id)}: checkbox shown without cb`);
    const refs = x.backlinks?.get(id) ?? 0;
    if (refs) {
      if (!r.badge || r.badge.hidden || r.badge.textContent !== String(refs)) bad(`${name(id)}: backlink badge, want ${refs}`);
    } else if (r.badge && !r.badge.hidden) bad(`${name(id)}: stale backlink badge`);

    // body: content, note?
    const body = [...r.body.children];
    if (body[0] !== r.content || body.length > 2 || (body.length === 2 && body[1] !== r.note)) bad(`${name(id)}: body parts`);
    field(r, id, 'c', r.content, r.c);
    const nt = str(p.n);
    // a note takes a line when it has text (spaces alone don't) or while the note is the edited field
    const noteShown = nt.trim() !== '' || (!!ed$ && ed$.id === id && ed$.f === 'n');
    if (noteShown) {
      if (!r.note) bad(`${name(id)}: note missing`);
      else {
        if (r.note.hidden) bad(`${name(id)}: note hidden`);
        field(r, id, 'n', r.note, r.n);
      }
    } else if (r.note && !r.note.hidden) {
      bad(`${name(id)}: empty note shown`);
    }

    // node children: row, kids?, collapse-animation copy?
    if (!isTitle) {
      for (const c of r.el.children) {
        if (c !== r.row && c !== r.kids && !c.classList.contains('o-kids-ghost')) bad(`${name(id)}: stray child ${c.className}`);
      }
      if (r.kids && r.kids.parentElement !== r.el) bad(`${name(id)}: children block detached`);
    }
  };

  // title (the zoom root)
  const zoom = v.zoom;
  if (x.title.id !== zoom || x.rows.get(zoom) !== x.title) bad(`title row is not the zoom root ${zoom}`);
  checkRow(x.title, zoom, 0, true);

  // chunking: a list is grouped iff it is longer than chunkSize; chunks hold [min, max + min) rows, not mixed with rows
  const max = Math.max(1, x.chunkSize);
  const min = Math.max(1, max >> 2);
  const chunks = (parent: NodeId, container: HTMLElement, n: number) => {
    const kids = [...container.children] as HTMLElement[];
    const grouped = kids.filter((c) => c.classList.contains(GROUP_CLASS));
    if (grouped.length && grouped.length !== kids.length) return bad(`children of ${name(parent)}: rows and chunks mixed`);
    if (!!grouped.length !== n > max) return bad(`children of ${name(parent)}: ${n} rows ${grouped.length ? '' : 'not '}chunked (chunk size ${max})`);
    grouped.forEach((c, i) => {
      const k = c.classList.contains(PENDING_CLASS) ? (x.pendingIds(c)?.length ?? 0) : c.children.length;
      if (!k || k >= max + min || (k < min && grouped.length > 1)) bad(`children of ${name(parent)}: chunk ${i} has ${k} rows (size ${max})`);
      if (c.querySelector(`:scope > :not(.o-node)`)) bad(`children of ${name(parent)}: chunk ${i} holds a non-row`);
    });
  };

  // bullet threading marks: the current item and its ancestors (o-path); in each children block on the path, the
  // rows of the active branch's chunk before it and the chunks before that one (o-above)
  const cur = ed.prefs.bulletThreading && ed$ ? ed$.id : null;
  const onPath = new Set(cur ? [cur, ...t.ancestors(cur)] : []);
  const wantPath = new Set<Element>();
  const wantAbove = new Set<Element>();
  if (cur && x.rows.get(cur)) wantPath.add(x.rows.get(cur)!.el);

  const seen = new Set<NodeId>();
  // an item standing in a pending block: neither it nor what it shows below has a row
  const unmade = (id: NodeId) => {
    if (x.rows.has(id)) bad(`${name(id)}: has a row but stands in a pending block`);
    for (const c of visibleChildren(t, id, v)) unmade(c);
  };
  const walk = (parent: NodeId, container: HTMLElement | null) => {
    const want = visibleChildren(t, parent, v);
    if (container?.classList.contains(PENDING_CLASS)) {
      if (container === x.items) bad('the top list is pending');
      if (!want.length) bad(`children of ${name(parent)}: pending with nothing to show`);
      if (container.firstElementChild) bad(`children of ${name(parent)}: pending but has elements`);
      if (!container.style.height) bad(`children of ${name(parent)}: pending without a height`);
      want.forEach(unmade);
      return;
    }
    if (container?.style.height) bad(`children of ${name(parent)}: a filled block keeps a fixed height`);
    const els = container ? itemSlots(container, x.pendingIds, bad) : [];
    if (container) chunks(parent, container, els.length);
    const got = els.map((e) => e.id);
    if (got.join() !== want.join()) {
      bad(`children of ${name(parent)}: DOM [${got.join()}] != model [${want.join()}]`);
      return;
    }
    const numbered = !!t.get(parent)?.props.num;
    const ci = cur && parent !== cur && onPath.has(parent) ? want.findIndex((id) => onPath.has(id)) : -1;
    const branch = ci >= 0 ? els[ci]!.el : null;
    if (ci >= 0 && !branch) bad(`children of ${name(parent)}: the branch to the edited item has no row`);
    if (branch && parent !== zoom) {
      const box = branch.parentElement!;
      for (let i = 0; i < ci; i++) if (els[i]!.el?.parentElement === box) wantAbove.add(els[i]!.el!);
      if (box.classList.contains(GROUP_CLASS)) for (let c = box.previousElementSibling; c; c = c.previousElementSibling) wantAbove.add(c);
    }
    want.forEach((id, i) => {
      if (!els[i]!.el) return unmade(id);
      if (onPath.has(id)) wantPath.add(els[i]!.el!);
      const r = x.rows.get(id);
      if (!r) return bad(`no row for visible item ${name(id)}`);
      if (r.el !== els[i]!.el) bad(`${name(id)}: row map points to another element`);
      if (seen.has(id)) bad(`${name(id)}: rendered twice`);
      seen.add(id);
      checkRow(r, id, numbered ? i + 1 : 0, false);
      walk(id, r.kids);
    });
  };
  walk(zoom, x.items);

  // nothing else is rendered or remembered
  const real = [...x.items.querySelectorAll<HTMLElement>('.o-node')].filter((e) => !e.closest('.o-kids-ghost'));
  if (real.length !== seen.size) bad(`${real.length} item elements for ${seen.size} visible items`);
  for (const [id, r] of x.rows) {
    if (r !== x.title && !seen.has(id)) bad(`row map keeps ${name(id)} (${r.el.isConnected ? 'connected' : 'detached'})`);
  }
  if (ed.el.classList.contains('o-empty') !== (visibleChildren(t, zoom, v).length === 0)) bad('o-empty class');
  if (ed.el.querySelectorAll('.o-current').length > (ed$ ? 1 : 0)) bad('more than one current item');
  const editable = [...ed.el.querySelectorAll<HTMLElement>('[contenteditable]')];
  if (editable.length > 1 || (editable[0] && editable[0] !== ed$?.el)) bad(`editable elements: ${editable.length}`);
  if (ed$) {
    const r = x.rows.get(ed$.id);
    if (!r || (ed$.el !== r.content && ed$.el !== r.note) || !ed$.el.isConnected) bad(`edited field of ${name(ed$.id)} is not rendered`);
  }
  for (const g of x.items.querySelectorAll<HTMLElement>('.o-kids-ghost')) {
    if (!g.inert || g.querySelector('[data-id], [contenteditable]')) bad('collapse-animation copy is addressable');
  }
  if (!x.animating && x.items.querySelector('.o-kids-ghost, .o-kids-anim')) bad('animation leftovers without a running animation');
  if (ed.el.querySelectorAll('.o-menu-btn').length > 1 || ed.el.querySelectorAll('.o-zoom-btn').length > 1) bad('hover buttons duplicated');
  if (zoom !== ROOT && !t.isAttached(zoom)) bad('zoomed into a detached item');
  const marked = (c: string) => [...ed.el.querySelectorAll(`.${c}`)].filter((e) => !e.closest('.o-kids-ghost'));
  for (const [c, want] of [['o-path', wantPath], ['o-above', wantAbove]] as const) {
    const got = marked(c);
    const label = (e: Element) => (e as HTMLElement).dataset.id ?? e.className;
    for (const e of got) if (!want.has(e)) bad(`threading: ${label(e)} marked ${c}`);
    for (const e of want) if (!got.includes(e)) bad(`threading: ${label(e)} not marked ${c}`);
  }
  // pending blocks: all tracked, all chunks or children blocks, holding a height
  for (const el of x.items.querySelectorAll<HTMLElement>(`.${PENDING_CLASS}`)) {
    if (el.closest('.o-kids-ghost')) continue;
    if (!x.pending.has(el)) bad('a pending block is not tracked');
    if (!el.matches(`.${GROUP_CLASS}, .o-kids`)) bad(`pending element ${el.className}`);
    if (el.matches(`.${GROUP_CLASS}`) && !el.style.height) bad('a pending chunk without a height');
  }
  for (const el of x.pending) if (el.isConnected && !el.classList.contains(PENDING_CLASS)) bad('a tracked block is not pending');
  return errs;
}

/** Serialized in place of an item whose row isn't made (in a pending block): matches any line of that item. */
export const UNMADE = '~';

/** Do two serialized lines show the same (`a` may be an item whose row isn't made)? */
export function sameLine(a: string, b: string): boolean {
  return a === b || (a.endsWith(` ${UNMADE}`) && b.startsWith(a.slice(0, -UNMADE.length)));
}

/**
 * The editor's rendering as text, independent of how rows are grouped or when parts were created: one line per
 * item with its depth, classes and rendered text. Two editors showing the same state serialize equally.
 */
export function serializeRender(ed: Editor): string {
  const x = ed.debugInternals();
  const t = ed.tree;
  const hl = (id: NodeId) => (x.searchHl?.ids.has(id) ? { highlight: x.searchHl.terms } : undefined);
  const out: string[] = [];
  const TRANSIENT = new Set(['o-current', 'o-selected', 'o-path', 'o-above', 'o-node', 'o-title']);
  const line = (r: Readonly<Row>, d: number) => {
    const id = r.el.dataset.id!;
    const n = t.get(id);
    const cls = [...r.el.classList].filter((c) => !TRANSIENT.has(c)).sort().join(' ');
    // the edited field shows source text: serialize what it would render to
    const html = (f: 'c' | 'n', el: HTMLElement | null) => (x.editing?.el === el || !el ? normalized(renderText(str(n?.props[f]), hl(id))) : el.innerHTML);
    const ck = r.check && !r.check.hidden ? (r.check.classList.contains('o-on') ? '[x]' : '[ ]') : '';
    const badge = r.badge && !r.badge.hidden ? `<${r.badge.textContent}>` : '';
    const note = str(n?.props.n) ? ` // ${html('n', r.note)}` : '';
    out.push(`${'  '.repeat(d)}${id} {${cls}} ${r.bullet.textContent}${ck}${badge} ${html('c', r.content)}${note} ${[...r.content.classList].filter((c) => c.startsWith('o-cl')).join(',')}`);
  };
  line(x.title, 0);
  const unmade = (id: NodeId, d: number) => {
    out.push(`${'  '.repeat(d)}${id} ${UNMADE}`);
    for (const c of visibleChildren(t, id, ed.view)) unmade(c, d + 1);
  };
  const walk = (owner: NodeId, container: HTMLElement | null, d: number) => {
    if (!container) return;
    if (container.classList.contains(PENDING_CLASS)) return visibleChildren(t, owner, ed.view).forEach((id) => unmade(id, d));
    for (const { id, el } of itemSlots(container, x.pendingIds)) {
      const r = el && x.rows.get(id);
      if (!el) unmade(id, d);
      else if (!r) out.push(`${'  '.repeat(d)}?? ${id}`);
      else {
        line(r, d);
        walk(id, r.kids, d + 1);
      }
    }
  };
  walk(ed.view.zoom, x.items, 1);
  return out.join('\n');
}
