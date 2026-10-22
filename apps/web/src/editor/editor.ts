// The outline editor: keyed DOM rendering + keyboard/mouse/clipboard handling.
//
// Rules for speed: rows are patched only for touched nodes; the item being
// edited is never re-rendered while typing (the DOM is the source of truth and
// the model is updated from it on every input event).

import {
  type CmdResult, type DocSession, type NodeId, type Portable, ROOT, type Sel, Tx, type View,
  backspaceAtStart, caret, cycleLevel, defaultView, deleteAtEnd, deleteItems, duplicate, enter,
  expandToLevel, flatten, indent, insertPortable, moveDown, moveUp, nextVisible, nextVisibleNonDescendant,
  nextVisibleSibling, normalizeBlock, outdent, pasteItems, prevVisible, prevVisibleSibling, replaceWithEmpty,
  selectBetween, setChildrenCheckbox, setLevel, toPortable, toggleChecked, toggleCollapse, toggleProp,
  toggleSiblings, toggleWrap, toggleWrapItems, uncheckDescendants, visibleChildren, wrapLink,
} from '@outliner/core';
import { getSel, getText, onFirstLine, onLastLine, placeAtX, caretRect, setSel, setText } from './caret.ts';
import { Keymap, comboOf } from './keymap.ts';
import { renderText, sourceOffsetAt } from './render.ts';
import { parseClipboardData, portableToPlain, portableToHtml } from './clipboard.ts';

export interface EditorHost {
  /** zoom changed by the editor (update URL / breadcrumbs owner) */
  onZoom(zoom: NodeId): void;
  /** document root title edited */
  onTitleEdit(title: string): void;
  /** app-level commands the editor doesn't own (settings, finders, panes...) */
  onAppCommand(id: string): boolean;
  /** clicked a link in rendered text */
  openLink(url: string): void;
  onTagClick(tag: string, alt: boolean): void;
  /** after every text input while editing (autocomplete triggers) */
  onTextInput?(ctx: TextContext): void;
  /** clicked a rendered date chip; [s, e) is the token's source range */
  onDateClick?(id: NodeId, s: number, e: number, anchor: HTMLElement): void;
  /** right-click / long-press on a bullet, or the item menu */
  onItemMenu?(id: NodeId, x: number, y: number): void;
}

export interface TextContext {
  id: NodeId;
  f: 'c' | 'n';
  el: HTMLElement;
  text: string;
  /** caret offset (collapsed selection) or null when a range is selected */
  caret: number | null;
}

/** A popup (autocomplete, date picker) that wants first look at keys. */
export interface KeyInterceptor {
  /** return true if the key was consumed */
  onKey(e: KeyboardEvent): boolean;
}

export interface EditorPrefs {
  bulletZoom: boolean;
  showNotes: 'show' | 'teaser' | 'hide';
  showChecked: boolean;
  unindentInPlace: boolean;
}

interface Row {
  id: NodeId;
  el: HTMLElement;
  row: HTMLElement;
  bullet: HTMLAnchorElement;
  check: HTMLElement;
  content: HTMLElement;
  note: HTMLElement;
  kids: HTMLElement;
  c: string | null;
  n: string | null;
  /** children/collapse/numbering signature at last reconcile */
  kidsKey: string;
  num: number;
}

interface Editing {
  id: NodeId;
  f: 'c' | 'n';
  el: HTMLElement;
}

const PLAINTEXT_ONLY = (() => {
  const d = document.createElement('div');
  try {
    d.contentEditable = 'plaintext-only';
  } catch {
    return false;
  }
  return d.contentEditable === 'plaintext-only';
})();

export class Editor {
  readonly el: HTMLElement;
  private crumbs: HTMLElement;
  private title: Row;
  private items: HTMLElement;
  private rows = new Map<NodeId, Row>();
  private session!: DocSession;
  private unsub: (() => void) | null = null;
  view: View = defaultView();
  private editing: Editing | null = null;
  private block: { ids: NodeId[]; anchor: NodeId; focus: NodeId } | null = null;
  private lastSel: Sel = { k: 'none' };
  /** suppress blur handling while the DOM is being rearranged */
  private mutating = 0;
  /** horizontal caret position kept across vertical moves */
  private goalX: number | null = null;
  private clip: { text: string; items: Portable[] } | null = null;
  keymap = new Keymap();
  /** set by an open popup; gets keys before the editor */
  interceptor: KeyInterceptor | null = null;
  prefs: EditorPrefs = { bulletZoom: false, showNotes: 'show', showChecked: true, unindentInPlace: false };
  readOnly = false;

  private host: EditorHost;

  constructor(host: EditorHost) {
    this.host = host;
    this.el = div('o-editor');
    this.el.tabIndex = -1;
    this.crumbs = div('o-crumbs');
    this.title = this.makeRow(ROOT, true);
    this.items = div('o-items');
    this.el.append(this.crumbs, this.title.el, this.items);

    this.el.addEventListener('keydown', (e) => this.onKeyDown(e));
    this.el.addEventListener('beforeinput', (e) => this.onBeforeInput(e));
    this.el.addEventListener('input', () => this.onInput());
    this.el.addEventListener('mousedown', (e) => this.onMouseDown(e));
    this.el.addEventListener('click', (e) => this.onClick(e));
    this.el.addEventListener('focusout', (e) => this.onFocusOut(e));
    this.el.addEventListener('copy', (e) => this.onCopy(e, false));
    this.el.addEventListener('cut', (e) => this.onCopy(e, true));
    this.el.addEventListener('paste', (e) => this.onPaste(e));
    this.el.addEventListener('contextmenu', (e) => {
      const b = (e.target as HTMLElement).closest('.o-bullet, .o-title > .o-row');
      const r = this.rowFromEvent(e);
      if (!b || !r || !this.host.onItemMenu || (e.target as HTMLElement).closest('.o-editing')) return;
      e.preventDefault();
      if (!this.block?.ids.includes(r.id) && r.id !== this.view.zoom) this.applySel({ k: 'block', ids: [r.id], anchor: r.id, focus: r.id });
      this.host.onItemMenu(r.id, e.clientX, e.clientY);
    });
    document.addEventListener('selectionchange', () => this.trackSel());
  }

  get tree() {
    return this.session.live;
  }

  get docSession() {
    return this.session;
  }

  // ---- document / view -------------------------------------------------------

  setDoc(session: DocSession, zoom: NodeId = ROOT) {
    this.unsub?.();
    this.session = session;
    this.unsub = session.onChange((kind, touched) => {
      if (kind === 'remote' || !touched) this.render();
      else this.patch(touched);
      if (kind === 'remote') this.revalidateSel();
    });
    this.block = null;
    this.editing = null;
    this.view = { ...this.view, zoom: session.live.get(zoom) && session.live.isAttached(zoom) ? zoom : ROOT };
    this.render();
  }

  setZoom(zoom: NodeId, focus?: Sel) {
    if (!this.tree.get(zoom) || !this.tree.isAttached(zoom)) zoom = ROOT;
    const changed = zoom !== this.view.zoom;
    this.stopEdit();
    this.block = null;
    this.view = { ...this.view, zoom };
    this.render();
    if (changed) this.host.onZoom(zoom);
    if (focus) this.applySel(focus);
  }

  setFilter(filter: Set<NodeId> | null) {
    this.view = { ...this.view, filter };
    this.render();
  }

  /** Re-render all text (e.g. after render options like search highlight changed). */
  refreshText() {
    for (const r of this.rows.values()) r.c = r.n = null;
    this.render();
  }

  setShowChecked(v: boolean) {
    this.view = { ...this.view, showChecked: v };
    this.render();
  }

  applyPrefs(p: Partial<EditorPrefs>) {
    this.prefs = { ...this.prefs, ...p };
    this.el.classList.toggle('o-bullet-zoom', this.prefs.bulletZoom);
    this.el.dataset.notes = this.prefs.showNotes;
    if (p.showChecked !== undefined) this.setShowChecked(p.showChecked);
  }

  // ---- rendering ---------------------------------------------------------------

  private makeRow(id: NodeId, isTitle = false): Row {
    const el = div(isTitle ? 'o-title' : 'o-node');
    const row = div('o-row');
    const bullet = document.createElement('a');
    bullet.className = 'o-bullet';
    bullet.draggable = false;
    const check = div('o-check');
    const body = div('o-body');
    const content = div('o-content');
    const note = div('o-note');
    const kids = div('o-kids');
    body.append(content, note);
    row.append(bullet, check, body);
    el.append(row);
    if (!isTitle) el.append(kids);
    el.dataset.id = id;
    return { id, el, row, bullet, check, content, note, kids, c: null, n: null, kidsKey: '', num: -1 };
  }

  /** Full render (reuses existing rows). */
  render() {
    if (!this.session) return;
    this.mutating++;
    try {
      const t = this.tree;
      const zoom = this.view.zoom;
      // the title row is keyed by the zoom id
      if (this.title.id !== zoom) {
        this.rows.delete(this.title.id);
        if (this.editing?.id === this.title.id) this.editing = null;
        this.title.id = zoom;
        this.title.el.dataset.id = zoom;
        this.title.c = this.title.n = null;
      }
      this.rows.set(zoom, this.title);
      this.renderCrumbs();
      this.updateRow(this.title, 0);
      this.reconcile(zoom, this.items, true);
      this.el.classList.toggle('o-empty', visibleChildren(t, zoom, this.view).length === 0);
      for (const [id, r] of this.rows) if (!r.el.isConnected) this.rows.delete(id);
      this.paintBlock();
    } finally {
      this.mutating--;
    }
  }

  /** Patch rows for touched nodes only. */
  private patch(touched: Set<NodeId>) {
    this.mutating++;
    try {
      const t = this.tree;
      const parents = new Set<NodeId>();
      let removed = false;
      for (const id of touched) {
        const n = t.get(id);
        if (!n) continue;
        const r = this.rows.get(id);
        if (r) {
          // children list may have changed (collapse, check filter, moves)
          if (r.kidsKey !== kidsKey(n.children, n.collapsed, n.props.num)) parents.add(id);
          else this.updateRow(r, this.numberOf(id));
        }
        const p = n.parent;
        if (p) {
          const pr = this.rows.get(p);
          if (!pr || !r || r.el.parentElement !== (p === this.view.zoom ? this.items : pr.kids) || pr.kidsKey !== kidsKey(t.get(p)!.children, t.get(p)!.collapsed, t.get(p)!.props.num)) {
            parents.add(p);
          }
          // checked state affects visibility in the parent when hiding checked items
          if (!this.view.showChecked) parents.add(p);
        } else if (r) {
          removed = true;
        }
      }
      for (const p of parents) {
        const r = this.rows.get(p);
        if (!r) continue;
        this.reconcile(p, p === this.view.zoom ? this.items : r.kids, false);
        this.updateRow(r, this.numberOf(p));
        removed = true;
      }
      if (removed) for (const [id, r] of this.rows) if (!r.el.isConnected) this.rows.delete(id);
      if (touched.has(this.view.zoom) || (this.view.zoom !== ROOT && [...touched].some((id) => t.ancestors(this.view.zoom).includes(id)))) {
        this.renderCrumbs();
      }
      this.el.classList.toggle('o-empty', visibleChildren(t, this.view.zoom, this.view).length === 0);
      if (this.block) this.paintBlock();
    } finally {
      this.mutating--;
    }
  }

  private numberOf(id: NodeId): number {
    const t = this.tree;
    const p = t.get(id)?.parent;
    if (!p || id === this.view.zoom || !t.get(p)?.props.num) return 0;
    return visibleChildren(t, p, this.view).indexOf(id) + 1;
  }

  private reconcile(parent: NodeId, container: HTMLElement, deep: boolean) {
    const t = this.tree;
    const want = visibleChildren(t, parent, this.view);
    const pn = t.get(parent);
    const numbered = !!pn?.props.num;
    const pr = this.rows.get(parent);
    if (pr && pn) pr.kidsKey = kidsKey(pn.children, pn.collapsed, pn.props.num);
    // index-based: recursing may pull rows out of this container (e.g. after zoom out)
    want.forEach((id, i) => {
      let r = this.rows.get(id);
      const fresh = !r || r === this.title;
      if (fresh) {
        r = this.makeRow(id);
        this.rows.set(id, r);
      }
      const at = container.children[i] ?? null;
      if (r!.el !== at) container.insertBefore(r!.el, at);
      const num = numbered ? i + 1 : 0;
      // touched rows are updated by patch(); others only when their number moved
      if (fresh || deep || r!.num !== num) this.updateRow(r!, num);
      if (fresh || deep) this.reconcile(id, r!.kids, deep);
    });
    while (container.children.length > want.length) container.lastElementChild!.remove();
  }

  private updateRow(r: Row, num: number) {
    const t = this.tree;
    const n = t.get(r.id);
    if (!n) return;
    r.num = num;
    const p = n.props;
    const isTitle = r === this.title;
    const kids = n.children.length > 0;
    const cls = r.el.classList;
    cls.toggle('o-checked', !!p.ck);
    cls.toggle('o-has-kids', kids);
    cls.toggle('o-collapsed', kids && n.collapsed && !isTitle);
    cls.toggle('o-numbered', num > 0);
    for (const h of [1, 2, 3]) cls.toggle(`o-h${h}`, p.h === h);
    for (const c of [1, 2, 3, 4, 5, 6]) r.content.classList.toggle(`o-cl${c}`, p.cl === c);
    r.check.hidden = !p.cb;
    r.check.classList.toggle('o-on', !!p.ck);
    r.bullet.textContent = num > 0 ? `${num}.` : '';
    r.bullet.href = `#z=${r.id}`;

    const c = (p.c as string | undefined) ?? '';
    const nt = (p.n as string | undefined) ?? '';
    this.syncField(r, 'c', c);
    this.syncField(r, 'n', nt);
    r.note.hidden = !nt && !(this.editing?.id === r.id && this.editing.f === 'n');
  }

  private syncField(r: Row, f: 'c' | 'n', value: string) {
    const el = f === 'c' ? r.content : r.note;
    if (this.editing && this.editing.el === el) {
      if (getText(el) !== value) {
        const s = getSel(el);
        setText(el, value);
        if (s && document.activeElement === el) setSel(el, Math.min(s.s, value.length), Math.min(s.e, value.length));
      }
      r[f] = value;
      return;
    }
    if (r[f] === value) return;
    r[f] = value;
    el.innerHTML = renderText(value);
  }

  private renderCrumbs() {
    const t = this.tree;
    const zoom = this.view.zoom;
    this.crumbs.textContent = '';
    if (zoom === ROOT) {
      this.crumbs.hidden = true;
      return;
    }
    this.crumbs.hidden = false;
    for (const a of t.ancestors(zoom)) {
      const link = document.createElement('a');
      link.className = 'o-crumb';
      link.dataset.id = a;
      link.href = `#z=${a}`;
      const text = (t.get(a)?.props.c as string | undefined) ?? '';
      link.innerHTML = a === ROOT && !text ? 'Home' : renderText(text) || '&nbsp;';
      this.crumbs.append(link);
      const sep = document.createElement('span');
      sep.className = 'o-crumb-sep';
      sep.textContent = '›';
      this.crumbs.append(sep);
    }
  }

  // ---- selection ---------------------------------------------------------------

  currentSel(): Sel {
    if (this.block) return { k: 'block', ...this.block, ids: [...this.block.ids] };
    if (this.editing) {
      const s = getSel(this.editing.el);
      if (s) return { k: 'text', id: this.editing.id, f: this.editing.f, s: s.s, e: s.e };
      return this.lastSel;
    }
    return { k: 'none' };
  }

  private trackSel() {
    if (!this.editing) return;
    const s = getSel(this.editing.el);
    if (s) this.lastSel = { k: 'text', id: this.editing.id, f: this.editing.f, s: s.s, e: s.e };
  }

  applySel(sel: Sel | undefined) {
    if (!sel) return;
    if (sel.k === 'none') {
      this.block = null;
      this.paintBlock();
      this.stopEdit();
      return;
    }
    if (sel.k === 'block') {
      const ids = sel.ids.filter((id) => this.rows.has(id));
      if (!ids.length) return;
      this.stopEdit();
      this.block = { ids, anchor: sel.anchor, focus: sel.focus };
      this.paintBlock();
      this.el.focus({ preventScroll: true });
      this.rows.get(sel.focus)?.row.scrollIntoView({ block: 'nearest' });
      return;
    }
    this.block = null;
    this.paintBlock();
    this.ensureVisible(sel.id);
    this.startEdit(sel.id, sel.f, sel.s, sel.e);
  }

  /** Expand collapsed ancestors so `id` has a row (without undo). */
  private ensureVisible(id: NodeId) {
    if (this.rows.has(id)) return;
    const t = this.tree;
    if (!t.isAttached(id)) return;
    const anc = t.ancestors(id);
    if (!anc.includes(this.view.zoom) && id !== this.view.zoom) {
      this.setZoom(ROOT);
    }
    const tx = new Tx(t);
    for (const a of t.ancestors(id)) if (t.get(a)!.collapsed) tx.collapse(a, false);
    if (tx.ops.length) this.session.commit(tx, { before: null, after: null }, { undoable: false });
  }

  private revalidateSel() {
    if (this.editing && !this.rows.has(this.editing.id)) this.stopEdit();
    if (this.block) {
      this.block.ids = this.block.ids.filter((id) => this.rows.has(id));
      if (!this.block.ids.length) this.block = null;
      this.paintBlock();
    }
  }

  private painted: HTMLElement[] = [];

  private paintBlock() {
    for (const el of this.painted) el.classList.remove('o-selected');
    this.painted = [];
    if (!this.block) return;
    for (const id of this.block.ids) {
      const el = this.rows.get(id)?.el;
      if (el) {
        el.classList.add('o-selected');
        this.painted.push(el);
      }
    }
  }

  // ---- editing -----------------------------------------------------------------

  startEdit(id: NodeId, f: 'c' | 'n', s: number, e = s) {
    const r = this.rows.get(id);
    if (!r) return;
    const el = f === 'c' ? r.content : r.note;
    if (this.editing?.el !== el) {
      this.stopEdit();
      this.editing = { id, f, el };
      r.note.hidden = false;
      el.contentEditable = PLAINTEXT_ONLY ? 'plaintext-only' : 'true';
      el.spellcheck = false;
      setText(el, (this.tree.get(id)?.props[f] as string | undefined) ?? '');
      el.classList.add('o-editing');
      r.el.classList.add('o-current');
    }
    this.mutating++;
    el.focus({ preventScroll: true });
    this.mutating--;
    const len = getText(el).length;
    setSel(el, Math.min(s, len), Math.min(e, len));
    this.lastSel = { k: 'text', id, f, s, e };
    scrollCaretIntoView(el);
  }

  stopEdit() {
    const ed = this.editing;
    if (!ed) return;
    this.editing = null;
    ed.el.removeAttribute('contenteditable');
    ed.el.classList.remove('o-editing');
    const r = this.rows.get(ed.id);
    r?.el.classList.remove('o-current');
    const value = (this.tree.get(ed.id)?.props[ed.f] as string | undefined) ?? '';
    ed.el.innerHTML = renderText(value);
    if (r) {
      r[ed.f] = value;
      r.note.hidden = !((this.tree.get(ed.id)?.props.n as string | undefined) ?? '');
    }
  }

  private onFocusOut(e: FocusEvent) {
    if (this.mutating || !this.editing) return;
    if (e.target !== this.editing.el) return;
    const to = e.relatedTarget as Node | null;
    if (to && this.el.contains(to)) return;
    // window lost focus (alt-tab): keep the edit state so focus returns there
    if (!document.hasFocus()) return;
    this.stopEdit();
  }

  private onInput() {
    const ed = this.editing;
    if (!ed) return;
    const text = getText(ed.el);
    const cur = (this.tree.get(ed.id)?.props[ed.f] as string | undefined) ?? '';
    if (text === cur) return;
    const s = getSel(ed.el);
    const before = this.lastSel;
    const after: Sel = { k: 'text', id: ed.id, f: ed.f, s: s?.s ?? text.length, e: s?.e ?? text.length };
    const tx = new Tx(this.tree);
    tx.set(ed.id, { [ed.f]: text });
    this.session.commit(tx, { before, after });
    this.lastSel = after;
    if (ed.id === ROOT && ed.f === 'c') this.host.onTitleEdit(text);
    this.host.onTextInput?.({ id: ed.id, f: ed.f, el: ed.el, text, caret: s && s.s === s.e ? s.s : null });
  }

  /** Current edit context (for popups). */
  textContext(): TextContext | null {
    const ed = this.editing;
    if (!ed) return null;
    const s = getSel(ed.el);
    return { id: ed.id, f: ed.f, el: ed.el, text: getText(ed.el), caret: s && s.s === s.e ? s.s : null };
  }

  /**
   * Replace [s, e) of a node's field with `text` as one undo step and put the
   * caret at `caretAt` (selection [caretAt, caretEnd]). Used by autocomplete
   * and the date picker; works whether or not the item is being edited.
   */
  replaceRange(id: NodeId, f: 'c' | 'n', s: number, e: number, text: string, caretAt?: number, caretEnd?: number) {
    const ed = this.editing;
    const cur = ed && ed.id === id && ed.f === f ? getText(ed.el) : ((this.tree.get(id)?.props[f] as string | undefined) ?? '');
    const next = cur.slice(0, s) + text + cur.slice(e);
    const at = caretAt ?? s + text.length;
    this.run((tx) => {
      tx.set(id, { [f]: next });
      return { sel: { k: 'text', id, f, s: at, e: caretEnd ?? at } };
    });
  }

  // ---- command execution -------------------------------------------------------

  /** Run a structural command as one undoable transaction. */
  run(fn: (tx: Tx) => CmdResult | null | void): boolean {
    if (this.readOnly) return false;
    const before = this.currentSel();
    const tx = new Tx(this.tree);
    this.mutating++;
    try {
      const r = fn(tx);
      if (!r) {
        if (tx.ops.length) {
          // commands like collapse return nothing but still did work
          this.session.commit(tx, { before, after: before });
          this.applySel(before);
          return true;
        }
        return false;
      }
      this.session.commit(tx, { before, after: r.sel });
      if (r.zoom !== undefined) this.setZoom(r.zoom);
      this.applySel(r.sel);
      return true;
    } finally {
      this.mutating--;
    }
  }

  /** ids targeted by item commands: block selection or the item being edited. */
  targets(): NodeId[] {
    if (this.block) return normalizeBlock(this.tree, this.block.ids);
    if (this.editing) return [this.editing.id];
    return [];
  }

  undo() {
    this.mutating++;
    try {
      const sel = this.session.undo();
      this.applySel(sel as Sel | undefined);
    } finally {
      this.mutating--;
    }
  }

  redo() {
    this.mutating++;
    try {
      const sel = this.session.redo();
      this.applySel(sel as Sel | undefined);
    } finally {
      this.mutating--;
    }
  }

  exec(id: string): boolean {
    const v = this.view;
    const ids = this.targets();
    const sel = this.currentSel();
    const has = ids.length > 0;
    const notTitle = ids.filter((x) => x !== v.zoom);
    switch (id) {
      case 'indent':
        return has && this.run((tx) => indent(tx, v, notTitle, sel));
      case 'unindent':
        return has && this.run((tx) => outdent(tx, v, notTitle, sel));
      case 'collapse':
        return has && this.run((tx) => toggleCollapse(tx, v, ids, false));
      case 'collapse-all':
        return has && this.run((tx) => toggleCollapse(tx, v, ids, true));
      case 'toggle-siblings':
        return has && this.run((tx) => toggleSiblings(tx, ids[0]!));
      case 'expand-1':
      case 'expand-2':
      case 'expand-3':
      case 'expand-4':
        return has && this.run((tx) => expandToLevel(tx, ids, Number(id.slice(-1))));
      case 'zoom-in': {
        const target = ids[0];
        if (!target) return false;
        const len = ((this.tree.get(target)?.props.c as string) ?? '').length;
        this.setZoom(target, caret(target, len));
        return true;
      }
      case 'zoom-out': {
        const z = this.view.zoom;
        const p = this.tree.get(z)?.parent;
        if (!p) return true;
        this.setZoom(p, caret(z, ((this.tree.get(z)?.props.c as string) ?? '').length));
        return true;
      }
      case 'zoom-prev-sibling':
      case 'zoom-next-sibling': {
        const z = this.view.zoom;
        const sib = id === 'zoom-prev-sibling' ? this.tree.prevSibling(z) : this.tree.nextSibling(z);
        if (sib) this.setZoom(sib);
        return true;
      }
      case 'delete-left':
      case 'delete-right':
        return notTitle.length > 0 && this.run((tx) => deleteItems(tx, v, notTitle, id === 'delete-left' ? 'left' : 'right'));
      case 'goto-note':
        return this.gotoNote();
      case 'toggle-checked':
        return notTitle.length > 0 && this.run((tx) => toggleChecked(tx, v, notTitle, sel));
      case 'uncheck-children':
        return has && this.run((tx) => uncheckDescendants(tx, ids, sel));
      case 'toggle-checkbox':
        return has && this.run((tx) => toggleProp(tx, ids, 'cb', sel));
      case 'toggle-children-checkbox':
        return has && this.run((tx) => setChildrenCheckbox(tx, ids, sel));
      case 'toggle-numbered':
        return has && this.run((tx) => toggleProp(tx, ids, 'num', sel));
      case 'duplicate':
        return notTitle.length > 0 && this.run((tx) => duplicate(tx, notTitle));
      case 'select-up':
      case 'select-down':
        return this.extendBlock(id === 'select-up' ? -1 : 1);
      case 'select-all': {
        const kids = visibleChildren(this.tree, v.zoom, v);
        if (kids.length) this.applySel({ k: 'block', ids: kids, anchor: kids[0]!, focus: kids[kids.length - 1]! });
        return true;
      }
      case 'undo':
        this.undo();
        return true;
      case 'redo':
        this.redo();
        return true;
      case 'delete-checked':
        return this.run((tx) => {
          const scope = has ? ids : [v.zoom];
          const walk = (x: NodeId) => {
            for (const c of [...(tx.tree.get(x)?.children ?? [])]) {
              if (tx.tree.get(c)!.props.ck) tx.move(c, null, null);
              else walk(c);
            }
          };
          scope.forEach(walk);
          return { sel: { k: 'none' } };
        });
      case 'sort-asc':
      case 'sort-desc':
        return has && this.run((tx) => {
          for (const p of ids) {
            const kids = [...tx.tree.get(p)!.children];
            const key = (x: NodeId) => String(tx.tree.get(x)!.props.c ?? '').toLocaleLowerCase();
            kids.sort((a, b) => key(a).localeCompare(key(b), undefined, { numeric: true }));
            if (id === 'sort-desc') kids.reverse();
            let after: NodeId | null = null;
            for (const k of kids) {
              tx.move(k, p, after);
              after = k;
            }
          }
          return { sel };
        });
      case 'swap-up':
        return notTitle.length > 0 && this.run((tx) => moveUp(tx, v, notTitle, sel));
      case 'swap-down':
        return notTitle.length > 0 && this.run((tx) => moveDown(tx, v, notTitle, sel));
      case 'goto-first': {
        const first = visibleChildren(this.tree, v.zoom, v)[0];
        if (first) this.applySel(caret(first, 0));
        return true;
      }
      case 'goto-last': {
        const all = flatten(this.tree, v);
        const last = all[all.length - 1];
        if (last) this.applySel(caret(last, ((this.tree.get(last)?.props.c as string) ?? '').length));
        return true;
      }
      case 'bold':
        return this.format('**');
      case 'italic':
        return this.format('__');
      case 'code':
        return this.format('`');
      case 'strike':
        return this.format('~~');
      case 'highlight':
        return this.format('==');
      case 'latex':
        return this.format('$$');
      case 'code-block':
        return this.format('```');
      case 'link':
        return this.editText((t, s, e) => wrapLink(t, s, e));
      case 'newline':
        return this.editText((t, s, e) => ({ text: t.slice(0, s) + '\n' + t.slice(e), s: s + 1, e: s + 1 }));
      case 'insert-date':
      case 'insert-time': {
        const d = new Date();
        const p2 = (x: number) => String(x).padStart(2, '0');
        let str = `!(${d.getFullYear()}-${p2(d.getMonth() + 1)}-${p2(d.getDate())}`;
        if (id === 'insert-time') str += ` ${p2(d.getHours())}:${p2(d.getMinutes())}`;
        str += ')';
        return this.editText((t, s, e) => ({ text: t.slice(0, s) + str + t.slice(e), s: s + str.length, e: s + str.length }));
      }
      case 'cycle-heading':
        return has && this.run((tx) => cycleLevel(tx, ids, 'h', 3, sel));
      case 'cycle-color':
        return has && this.run((tx) => cycleLevel(tx, ids, 'cl', 6, sel));
      default: {
        const m = /^(heading|color)-(\d)$/.exec(id);
        if (m && has) {
          const key = m[1] === 'heading' ? 'h' : 'cl';
          return this.run((tx) => setLevel(tx, ids, key, Number(m[2]), sel));
        }
        return this.host.onAppCommand(id);
      }
    }
  }

  private format(d: string): boolean {
    if (this.block) {
      const ids = this.targets();
      return this.run((tx) => {
        toggleWrapItems(tx, ids, d);
        return { sel: this.currentSel() };
      });
    }
    return this.editText((t, s, e) => toggleWrap(t, s, e, d));
  }

  /** Replace the edited field's text via a pure text transform (one undo step). */
  private editText(fn: (text: string, s: number, e: number) => { text: string; s: number; e: number }): boolean {
    const ed = this.editing;
    if (!ed) return false;
    const sel = getSel(ed.el) ?? { s: 0, e: 0 };
    const cur = getText(ed.el);
    const r = fn(cur, sel.s, sel.e);
    return this.run((tx) => {
      tx.set(ed.id, { [ed.f]: r.text });
      return { sel: { k: 'text', id: ed.id, f: ed.f, s: r.s, e: r.e } };
    });
  }

  private gotoNote(): boolean {
    const ed = this.editing;
    if (!ed) return false;
    if (ed.f === 'n') {
      const len = ((this.tree.get(ed.id)?.props.c as string) ?? '').length;
      this.applySel(caret(ed.id, len));
      return true;
    }
    const s = getSel(ed.el);
    const n = this.tree.get(ed.id)!;
    const note = (n.props.n as string | undefined) ?? '';
    if (s && s.s !== s.e && !note) {
      // move the selected text into the note
      const text = getText(ed.el);
      return this.run((tx) => {
        tx.set(ed.id, { c: text.slice(0, s.s) + text.slice(s.e), n: text.slice(s.s, s.e) });
        return { sel: caret(ed.id, s.e - s.s, 'n') };
      });
    }
    this.applySel(caret(ed.id, note.length, 'n'));
    return true;
  }

  private extendBlock(dir: -1 | 1): boolean {
    const t = this.tree;
    const v = this.view;
    if (!this.block) {
      const ed = this.editing;
      if (!ed || ed.id === v.zoom) return false;
      const s = getSel(ed.el);
      const len = getText(ed.el).length;
      const atEdge = dir < 0 ? onFirstLine(ed.el) || s?.s === 0 : onLastLine(ed.el) || s?.e === len;
      if (!atEdge) return false; // native text selection extend
      this.applySel({ k: 'block', ids: [ed.id], anchor: ed.id, focus: ed.id });
      return true;
    }
    const { anchor, focus } = this.block;
    const next = dir < 0 ? prevVisibleSibling(t, focus, v) ?? parentIfNotZoom(t, focus, v) : nextVisibleSibling(t, focus, v) ?? nextVisibleNonDescendant(t, focus, v);
    if (!next) return true;
    const ids = selectBetween(t, anchor, next, v);
    this.applySel({ k: 'block', ids, anchor, focus: next });
    return true;
  }

  // ---- keyboard ------------------------------------------------------------------

  private onKeyDown(e: KeyboardEvent) {
    if (e.isComposing || e.keyCode === 229) return;
    if (this.interceptor?.onKey(e)) {
      e.preventDefault();
      e.stopPropagation();
      return;
    }
    const combo = comboOf(e);
    if (!['Up', 'Down', 'Shift+Up', 'Shift+Down'].includes(combo)) this.goalX = null;

    if (this.handleHardKey(combo, e)) {
      e.preventDefault();
      return;
    }
    const cmd = this.keymap.resolve(combo);
    if (cmd && this.exec(cmd)) {
      e.preventDefault();
      e.stopPropagation();
    }
  }

  private handleHardKey(combo: string, e: KeyboardEvent): boolean {
    const t = this.tree;
    const v = this.view;
    const ed = this.editing;
    const blk = this.block;

    if (blk) {
      const ids = this.targets();
      switch (combo) {
        case 'Escape':
          this.applySel({ k: 'none' });
          return true;
        case 'Backspace':
        case 'Delete':
          return this.run((tx) => deleteItems(tx, v, ids, combo === 'Backspace' ? 'left' : 'right')) || true;
        case 'Enter':
          return this.run((tx) => replaceWithEmpty(tx, v, ids)) || true;
        case 'Up': {
          const p = prevVisible(t, ids[0]!, v);
          if (p) this.applySel(caret(p, 0));
          return true;
        }
        case 'Down': {
          const n = nextVisibleNonDescendant(t, ids[ids.length - 1]!, v);
          if (n) this.applySel(caret(n, 0));
          return true;
        }
        case 'Left':
          this.applySel(caret(ids[0]!, 0));
          return true;
        case 'Right': {
          const last = ids[ids.length - 1]!;
          this.applySel(caret(last, ((t.get(last)?.props.c as string) ?? '').length));
          return true;
        }
        case 'Mod+A':
          return this.progressiveSelectAll();
      }
      // printable key while block-selected: ignore (don't type into nothing)
      if (combo.length === 1 || combo.startsWith('Shift+') && combo.length === 7) return true;
      return false;
    }

    if (!ed) {
      if (combo === 'Escape' && this.lastSel.k === 'text' && this.rows.has(this.lastSel.id)) {
        this.applySel(this.lastSel);
        return true;
      }
      return false;
    }

    const el = ed.el;
    const s = getSel(el);
    if (!s) return false;
    const text = getText(el);
    const isTitle = ed.id === v.zoom;

    switch (combo) {
      case 'Enter':
        if (ed.f === 'n') return false; // newline in note
        return this.run((tx) => enter(tx, v, ed.id, s.s, s.e)) || true;
      case 'Backspace':
        if (s.s !== s.e || s.s !== 0) return false;
        if (ed.f === 'n') {
          if (text === '') {
            this.applySel(caret(ed.id, ((t.get(ed.id)?.props.c as string) ?? '').length));
            return true;
          }
          return false;
        }
        this.run((tx) => backspaceAtStart(tx, v, ed.id));
        return true;
      case 'Delete':
        if (ed.f === 'n' || s.s !== s.e || s.e !== text.length) return false;
        this.run((tx) => deleteAtEnd(tx, v, ed.id));
        return true;
      case 'Tab':
        if (ed.f === 'n') {
          return this.editText((tt, a, b) => ({ text: tt.slice(0, a) + '\t' + tt.slice(b), s: a + 1, e: a + 1 }));
        }
        return false; // keymap → indent
      case 'Up':
      case 'Down':
        return this.verticalMove(combo === 'Up' ? -1 : 1);
      case 'Left':
        if (s.s !== 0 || s.e !== 0) return false;
        if (ed.f === 'n') {
          this.applySel(caret(ed.id, ((t.get(ed.id)?.props.c as string) ?? '').length));
          return true;
        }
        {
          const p = prevVisible(t, ed.id, v);
          if (!p) return true;
          this.applySel(caret(p, ((t.get(p)?.props.c as string) ?? '').length));
        }
        return true;
      case 'Right':
        if (s.s !== text.length || s.e !== text.length || ed.f === 'n') return false;
        {
          const n = nextVisible(t, ed.id, v);
          if (n) this.applySel(caret(n, 0));
        }
        return true;
      case 'Escape':
        this.lastSel = { k: 'text', id: ed.id, f: ed.f, s: s.s, e: s.e };
        this.stopEdit();
        (document.activeElement as HTMLElement | null)?.blur();
        return true;
      case 'Mod+A':
        if (s.s === 0 && s.e === text.length && !isTitle) {
          this.applySel({ k: 'block', ids: [ed.id], anchor: ed.id, focus: ed.id });
          return true;
        }
        return false;
      case 'Mod+U':
        return true;
    }
    return false;
  }

  private progressiveSelectAll(): boolean {
    const t = this.tree;
    const v = this.view;
    const ids = this.targets();
    const parent = t.get(ids[0]!)?.parent;
    if (!parent) return true;
    const sibs = visibleChildren(t, parent, v);
    if (sibs.length !== ids.length) {
      this.applySel({ k: 'block', ids: sibs, anchor: sibs[0]!, focus: sibs[sibs.length - 1]! });
    } else if (parent !== v.zoom) {
      this.applySel({ k: 'block', ids: [parent], anchor: parent, focus: parent });
    }
    return true;
  }

  private verticalMove(dir: -1 | 1): boolean {
    const t = this.tree;
    const v = this.view;
    const ed = this.editing!;
    const el = ed.el;
    if (dir < 0 ? !onFirstLine(el) : !onLastLine(el)) return false;
    const s = getSel(el)!;
    const x = this.goalX ?? (s.s === 0 && dir > 0 ? -1 : caretRect(el).left);
    this.goalX = x;
    let target: NodeId | null;
    let f: 'c' | 'n' = 'c';
    if (dir < 0) {
      if (ed.f === 'n') target = ed.id;
      else {
        if (ed.id === v.zoom) return true;
        target = prevVisible(t, ed.id, v);
        if (target && this.noteVisible(target)) f = 'n';
      }
    } else {
      if (ed.f === 'c' && ed.id !== v.zoom && this.noteVisible(ed.id) && (t.get(ed.id)?.props.n as string)) {
        target = ed.id;
        f = 'n';
      } else {
        target = ed.id === v.zoom ? visibleChildren(t, v.zoom, v)[0] ?? null : nextVisible(t, ed.id, v);
      }
    }
    if (!target) {
      // first/last line with nowhere to go: jump to text start/end like code editors
      setSel(el, dir < 0 ? 0 : getText(el).length);
      return true;
    }
    this.startEdit(target, f, 0);
    const tel = this.editing!.el;
    if (x < 0) setSel(tel, 0);
    else placeAtX(tel, x, dir < 0 ? 'last' : 'first');
    this.trackSel();
    return true;
  }

  private noteVisible(id: NodeId): boolean {
    return this.prefs.showNotes === 'show' && !!(this.tree.get(id)?.props.n as string | undefined);
  }

  /** Mobile keyboards often send no usable keydown; handle structural input here. */
  private onBeforeInput(e: InputEvent) {
    const ed = this.editing;
    if (!ed) {
      if (this.block) e.preventDefault();
      return;
    }
    if (e.inputType === 'insertParagraph' || (e.inputType === 'insertLineBreak' && ed.f === 'c')) {
      if (ed.f === 'n') return;
      e.preventDefault();
      const s = getSel(ed.el);
      if (s) this.run((tx) => enter(tx, this.view, ed.id, s.s, s.e));
      return;
    }
    if (e.inputType === 'deleteContentBackward') {
      const s = getSel(ed.el);
      if (s && s.s === 0 && s.e === 0 && ed.f === 'c') {
        e.preventDefault();
        this.run((tx) => backspaceAtStart(tx, this.view, ed.id));
      }
    }
  }

  // ---- mouse -----------------------------------------------------------------------

  private rowFromEvent(e: Event): Row | null {
    const nodeEl = (e.target as HTMLElement).closest<HTMLElement>('.o-node, .o-title');
    if (!nodeEl || !this.el.contains(nodeEl)) return null;
    return this.rows.get(nodeEl.dataset.id!) ?? null;
  }

  private onMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    const r = this.rowFromEvent(e);
    if (!r) {
      // click in empty area below the list → end of last item
      if (target === this.el || target === this.items) {
        e.preventDefault();
        this.focusEnd();
      }
      return;
    }
    if (target.closest('.o-bullet, .o-check, a[href], .o-tag, .o-crumb')) return;
    const fieldEl = target.closest<HTMLElement>('.o-content, .o-note');
    const f: 'c' | 'n' = fieldEl?.classList.contains('o-note') ? 'n' : 'c';

    if (e.shiftKey && (this.editing || this.block)) {
      e.preventDefault();
      const from = this.block?.anchor ?? this.editing!.id;
      if (from === this.view.zoom || r.id === this.view.zoom) return;
      const ids = selectBetween(this.tree, from, r.id, this.view);
      this.applySel({ k: 'block', ids, anchor: from, focus: r.id });
      return;
    }
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      if (r.id === this.view.zoom) return;
      const cur = this.block?.ids ?? (this.editing ? [this.editing.id] : []);
      const ids = cur.includes(r.id) ? cur.filter((x) => x !== r.id) : normalizeBlock(this.tree, [...cur, r.id]);
      if (ids.length) this.applySel({ k: 'block', ids, anchor: ids[0]!, focus: ids[ids.length - 1]! });
      else this.applySel({ k: 'none' });
      return;
    }
    if (this.editing && fieldEl === this.editing.el) {
      this.startDragSelect(r.id);
      return; // native caret placement inside the edited field
    }
    e.preventDefault();
    const src = (this.tree.get(r.id)?.props[f] as string | undefined) ?? '';
    const off = fieldEl ? sourceOffsetAt(fieldEl, e.clientX, e.clientY, src) : 0;
    this.applySel(caret(r.id, off, f));
    this.startDragSelect(r.id);
  }

  /** Dragging from one item's text onto others turns into a block selection. */
  private startDragSelect(from: NodeId) {
    const move = (ev: MouseEvent) => {
      const el = document.elementFromPoint(ev.clientX, ev.clientY)?.closest<HTMLElement>('.o-node');
      const id = el?.dataset.id;
      if (!id || id === from && !this.block) return;
      if (from === this.view.zoom) return;
      const ids = selectBetween(this.tree, from, id, this.view);
      if (ids.length === 1 && ids[0] === from && !this.block && id === from) return;
      window.getSelection()?.removeAllRanges();
      this.applySel({ k: 'block', ids, anchor: from, focus: id });
    };
    const up = () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
  }

  private onClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    const r = this.rowFromEvent(e);
    const crumb = target.closest<HTMLElement>('.o-crumb');
    if (crumb) {
      e.preventDefault();
      this.setZoom(crumb.dataset.id!);
      return;
    }
    if (!r) return;
    if (target.closest('.o-bullet')) {
      e.preventDefault();
      const kids = (this.tree.get(r.id)?.children.length ?? 0) > 0;
      if (this.prefs.bulletZoom || !kids || e.altKey || matchMedia('(pointer: coarse)').matches) {
        this.setZoom(r.id);
      } else {
        this.run((tx) => toggleCollapse(tx, this.view, [r.id], false));
      }
      return;
    }
    if (target.closest('.o-check')) {
      e.preventDefault();
      this.run((tx) => toggleChecked(tx, this.view, [r.id], this.currentSel()));
      return;
    }
    const date = target.closest<HTMLElement>('.o-date');
    if (date && !target.closest('.o-editing') && this.host.onDateClick) {
      e.preventDefault();
      const field = date.closest('.o-note') ? 'n' : 'c';
      if (field === 'c') this.host.onDateClick(r.id, Number(date.dataset.s), Number(date.dataset.e), date);
      return;
    }
    const tag = target.closest<HTMLElement>('.o-tag');
    if (tag && !target.closest('.o-editing')) {
      e.preventDefault();
      this.host.onTagClick(tag.textContent ?? '', e.altKey);
      return;
    }
    const link = target.closest<HTMLAnchorElement>('a[href]');
    if (link && !target.closest('.o-editing')) {
      e.preventDefault();
      this.host.openLink(link.getAttribute('href')!);
    }
  }

  focusEnd() {
    const all = flatten(this.tree, this.view);
    const last = all[all.length - 1];
    if (last) {
      this.applySel(caret(last, ((this.tree.get(last)?.props.c as string) ?? '').length));
    } else {
      // empty document: create the first item
      this.run((tx) => ({ sel: caret(tx.create(this.view.zoom, null, {}), 0) }));
    }
  }

  // ---- clipboard -----------------------------------------------------------------

  private onCopy(e: ClipboardEvent, cut: boolean) {
    if (!this.block || !e.clipboardData) return;
    e.preventDefault();
    const ids = this.targets();
    const items = ids.map((id) => toPortable(this.tree, id, this.view));
    const text = portableToPlain(items);
    e.clipboardData.setData('text/plain', text);
    e.clipboardData.setData('text/html', portableToHtml(items));
    this.clip = { text, items };
    if (cut) this.run((tx) => deleteItems(tx, this.view, ids, 'left'));
  }

  private onPaste(e: ClipboardEvent) {
    const dt = e.clipboardData;
    if (!dt) return;
    const text = (dt.getData('text/plain') ?? '').replace(/\r\n?/g, '\n');
    const ed = this.editing;
    if (ed && (ed.f === 'n' || ed.id === this.view.zoom)) {
      e.preventDefault();
      this.editText((t, s, en) => ({ text: t.slice(0, s) + text + t.slice(en), s: s + text.length, e: s + text.length }));
      return;
    }
    const items = this.clip && this.clip.text === text ? this.clip.items : parseClipboardData(dt);
    if (!items.length) return;
    if (this.block) {
      e.preventDefault();
      const ids = this.targets();
      const parent = this.tree.get(ids[0]!)!.parent!;
      this.run((tx) => {
        const after = tx.tree.prevSibling(ids[0]!);
        for (const id of ids) tx.move(id, null, null);
        const nids = insertPortable(tx, parent, after, items);
        const last = nids[nids.length - 1]!;
        return { sel: caret(last, ((tx.tree.get(last)?.props.c as string) ?? '').length) };
      });
      return;
    }
    if (!ed) return;
    if (items.length === 1 && !items[0]!.children.length && !text.includes('\n')) return; // native single-line paste
    e.preventDefault();
    const s = getSel(ed.el) ?? { s: 0, e: 0 };
    this.run((tx) => pasteItems(tx, this.view, ed.id, s.s, s.e, items));
  }
}

function parentIfNotZoom(t: DocSession['live'], id: NodeId, v: View): NodeId | null {
  const p = t.get(id)?.parent;
  return p && p !== v.zoom ? p : null;
}

function kidsKey(children: NodeId[], collapsed: boolean, num: unknown): string {
  return `${collapsed ? 1 : 0}${num ? 1 : 0}${children.join(',')}`;
}

function div(cls: string): HTMLDivElement {
  const d = document.createElement('div');
  d.className = cls;
  return d;
}

function scrollCaretIntoView(el: HTMLElement) {
  const r = caretRect(el);
  const margin = 60;
  const vh = window.innerHeight;
  const scroller = el.closest('.o-scroll') ?? document.scrollingElement;
  if (!scroller) return;
  if (r.bottom > vh - margin) scroller.scrollBy({ top: r.bottom - vh + margin });
  else if (r.top < margin) scroller.scrollBy({ top: r.top - margin });
}
