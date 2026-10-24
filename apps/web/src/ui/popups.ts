// In-editor popups: tag autocomplete (`#`/`@`), item-link autocomplete (`[[`)
// and the date picker (`!`, date-chip click, "add-date" command).
// See docs/02-editor-keyboard.md §1.9 and docs/03-features-ui.md §2.6, §3.

import {
  type DateSpec, type FuzzyMatch, type NodeId, extractDates, formatDateToken, withLocalTz, fuzzyFilter, inlineToPlain, parseDateToken,
} from '@outliner/core';
import type { Editor, TextContext } from '../editor/editor.ts';
import { caretRect } from '../editor/caret.ts';
import { DatePicker } from './datepicker.ts';
import { h } from './dom.ts';
import { placePopup } from './position.ts';

export interface LinkCandidate {
  doc: string;
  id: NodeId;
  /** item source text */
  text: string;
  /** "Doc › parent › …" */
  path: string;
}

export interface PopupHost {
  /** [tag, count] sorted by count desc; `global` = all documents */
  tags(global: boolean): [string, number][];
  /** pref global_tag_autocomplete */
  globalTags(): boolean;
  /** all items of all documents (current doc first) */
  linkCandidates(): LinkCandidate[];
  /** internal URL of an item (`#d=…&z=…`) */
  itemUrl(doc: string, id: NodeId): string;
  /** first day of week, 0 = Sunday … 6 = Saturday */
  startDay(): number;
  hour12(): boolean;
  /** pref show_timezone: write the local UTC offset into new timed dates */
  encodeTimezone?(): boolean;
}

const MAX_TAGS = 8;
const MAX_LINKS = 10;
const isWs = (c: string | undefined) => c === undefined || /\s/.test(c);

// ---- generic suggestion list -------------------------------------------------------------

interface Suggestion {
  label: string;
  ranges?: [number, number][];
  sub?: string;
  aside?: string;
}

class SuggestList {
  readonly el = h('div.ac-popup', { role: 'listbox' });
  items: Suggestion[] = [];
  active = 0;
  onPick: (i: number) => void = () => {};

  constructor() {
    this.el.addEventListener('mousedown', (e) => e.preventDefault());
  }

  get visible() {
    return this.el.isConnected && this.items.length > 0;
  }

  show(items: Suggestion[], anchor: DOMRect) {
    this.items = items;
    this.active = 0;
    if (!items.length) {
      this.el.remove();
      return;
    }
    this.paint();
    if (!this.el.isConnected) document.body.append(this.el);
    placePopup(this.el, anchor);
  }

  hide() {
    this.items = [];
    this.el.remove();
  }

  move(d: number) {
    if (!this.items.length) return;
    this.active = (this.active + d + this.items.length) % this.items.length;
    this.paint();
  }

  private paint() {
    this.el.textContent = '';
    this.items.forEach((it, i) => {
      const label = h('span.ac-label');
      appendHighlighted(label, it.label, it.ranges);
      const row = h('div.ac-item', { role: 'option', onclick: () => this.onPick(i) },
        h('div.ac-main', {}, label, it.aside ? h('span.ac-aside', {}, it.aside) : null),
        it.sub ? h('div.ac-sub', {}, it.sub) : null);
      row.classList.toggle('active', i === this.active);
      row.addEventListener('mousemove', () => {
        if (this.active !== i) {
          this.active = i;
          for (const [j, c] of [...this.el.children].entries()) c.classList.toggle('active', j === i);
        }
      });
      this.el.append(row);
    });
    this.el.children[this.active]?.scrollIntoView({ block: 'nearest' });
  }
}

function appendHighlighted(el: HTMLElement, text: string, ranges?: [number, number][]) {
  let at = 0;
  for (const [s, e] of ranges ?? []) {
    if (s > at) el.append(text.slice(at, s));
    el.append(h('b', {}, text.slice(s, e)));
    at = e;
  }
  if (at < text.length) el.append(text.slice(at));
}

// ---- manager ---------------------------------------------------------------------------------

interface AcState {
  kind: 'tag' | 'link';
  id: NodeId;
  f: 'c' | 'n';
  /** offset of `#`/`@` or of `[[` */
  start: number;
  /** caret at last input (the typed token ends here) */
  end: number;
  tags?: [string, number][];
  tagHits?: string[];
  links?: LinkCandidate[];
  linkHits?: LinkCandidate[];
}

interface DateState {
  picker: DatePicker;
  id: NodeId;
  f: 'c' | 'n';
  s: number;
  e: number;
  /** opened by typing `!` (any further typing closes it) */
  typed: boolean;
  /** insert (add-date): pad the token with spaces */
  insert: boolean;
  /** the item was being edited when the picker opened */
  editing: boolean;
}

export class Popups {
  private ac: AcState | null = null;
  private list = new SuggestList();
  private date: DateState | null = null;

  private editor: Editor;
  private host: PopupHost;

  constructor(editor: Editor, host: PopupHost) {
    this.editor = editor;
    this.host = host;
    this.list.onPick = (i) => this.pick(i);
    document.addEventListener('selectionchange', () => this.onSelectionChange());
    document.addEventListener('mousedown', (e) => this.onOutsideDown(e), true);
    window.addEventListener('resize', () => this.reposition());
    window.addEventListener('scroll', () => this.reposition(), true);
    window.visualViewport?.addEventListener('resize', () => this.reposition());
  }

  get isOpen() {
    return !!this.ac || !!this.date;
  }

  closeAll() {
    this.closeAc();
    this.date?.picker.close();
  }

  // ---- input hook --------------------------------------------------------------------------

  onTextInput(ctx: TextContext) {
    // typing while the date picker is open (e.g. `!(` by hand) dismisses it
    this.date?.picker.close();
    const c = ctx.caret;
    if (c === null) return this.closeAc();
    const t = ctx.text;
    if (this.ac && (this.ac.id !== ctx.id || this.ac.f !== ctx.f)) this.closeAc();
    if (this.ac) {
      this.ac.end = c;
      this.updateAc(ctx);
      return;
    }
    const ch = t[c - 1];
    if ((ch === '#' || ch === '@') && isWs(t[c - 2])) {
      this.openAc('tag', ctx, c - 1);
    } else if (ch === '[' && t[c - 2] === '[' && isWs(t[c - 3])) {
      this.openAc('link', ctx, c - 2);
    } else if (ch === '!' && isWs(t[c - 2])) {
      this.openDate({ id: ctx.id, f: ctx.f, s: c - 1, e: c, typed: true, insert: false, spec: null }, () => caretRect(ctx.el));
    }
  }

  // ---- autocomplete --------------------------------------------------------------------------

  private openAc(kind: 'tag' | 'link', ctx: TextContext, start: number) {
    this.ac = { kind, id: ctx.id, f: ctx.f, start, end: ctx.caret ?? start + 1 };
    this.editor.interceptor = { onKey: (e) => this.acKey(e) };
    this.updateAc(ctx);
  }

  private closeAc() {
    if (!this.ac) return;
    this.ac = null;
    this.list.hide();
    if (!this.date) this.editor.interceptor = null;
  }

  private updateAc(ctx: TextContext) {
    const ac = this.ac!;
    const t = ctx.text;
    const c = ctx.caret ?? -1;
    if (ac.kind === 'tag') {
      const q = t.slice(ac.start, c);
      if (c <= ac.start || !/^[#@]\S*$/.test(q) || !isWs(t[ac.start - 1])) return this.closeAc();
      ac.tags ??= this.tagSource(q);
      const prefix = q[0]!;
      const body = q.slice(1).toLowerCase();
      // the tag being typed is itself in the document: don't count that occurrence
      const pool = ac.tags.filter(([tag]) => tag[0] === prefix && tag.toLowerCase() !== q.toLowerCase());
      const starts = pool.filter(([tag]) => tag.slice(1).toLowerCase().startsWith(body));
      const rest = body ? fuzzyFilter(body, pool.filter((x) => !starts.includes(x)), ([tag]) => tag.slice(1)).map((m) => m.item) : [];
      const hits = [...starts, ...rest].slice(0, MAX_TAGS);
      this.list.show(hits.map(([tag, n]) => ({
        label: tag,
        ranges: tag.slice(1).toLowerCase().startsWith(body) && body ? [[1, 1 + body.length]] : undefined,
        aside: String(n),
      })), caretRect(ctx.el));
      ac.tagHits = hits.map(([tag]) => tag);
    } else {
      const q = t.slice(ac.start + 2, c);
      if (c < ac.start + 2 || t.slice(ac.start, ac.start + 2) !== '[[' || /[\n\]]/.test(q)) return this.closeAc();
      ac.links ??= this.host.linkCandidates().filter((x) => x.id !== ac.id);
      const ranked: { item: LinkCandidate; match?: FuzzyMatch }[] = q.trim()
        ? fuzzyFilter(q, ac.links, (x) => plain(x.text))
        : ac.links.map((item) => ({ item }));
      const top = ranked.slice(0, MAX_LINKS);
      ac.linkHits = top.map((m) => m.item);
      this.list.show(top.map(({ item, match }) => ({ label: plain(item.text), ranges: match?.ranges, sub: item.path })), caretRect(ctx.el));
    }
  }

  private tagSource(q: string): [string, number][] {
    const tags = this.host.tags(this.host.globalTags());
    // drop one occurrence of the token being typed (it is already in the text)
    return tags
      .map(([tag, n]): [string, number] => [tag, tag.toLowerCase() === q.toLowerCase() ? n - 1 : n])
      .filter(([, n]) => n > 0);
  }

  private acKey(e: KeyboardEvent): boolean {
    if (!this.ac) return false;
    if (e.key === 'Escape') {
      // with no suggestions showing, Escape also leaves the item (Dynalist)
      const shown = this.list.visible;
      this.closeAc();
      return shown;
    }
    if (!this.list.visible || e.ctrlKey || e.metaKey || e.altKey) return false;
    switch (e.key) {
      case 'ArrowDown':
        this.list.move(1);
        return true;
      case 'ArrowUp':
        this.list.move(-1);
        return true;
      case 'Enter':
      case 'Tab':
        if (e.shiftKey) return false;
        this.pick(this.list.active);
        return true;
    }
    return false;
  }

  private pick(i: number) {
    const ac = this.ac;
    if (!ac) return;
    const ctx = this.editor.textContext();
    const text = ctx && ctx.id === ac.id && ctx.f === ac.f ? ctx.text : String(this.editor.tree.get(ac.id)?.props[ac.f] ?? '');
    const end = Math.max(ac.end, ctx?.caret ?? 0);
    this.closeAc();
    if (ac.kind === 'tag') {
      const tag = ac.tagHits?.[i];
      if (!tag) return;
      const spaced = text[end] === ' ';
      const ins = spaced ? tag : tag + ' ';
      this.editor.replaceRange(ac.id, ac.f, ac.start, end, ins, ac.start + tag.length + 1);
    } else {
      const it = ac.linkHits?.[i];
      if (!it) return;
      const title = (plain(it.text).replace(/\s+/g, ' ').trim() || 'Untitled').replace(/[[\]\\]/g, '\\$&');
      const md = `[${title}](${this.host.itemUrl(it.doc, it.id)})`;
      this.editor.replaceRange(ac.id, ac.f, ac.start, end, md, ac.start + 1, ac.start + 1 + title.length);
    }
  }

  private onSelectionChange() {
    const ac = this.ac;
    if (!ac) return;
    const ctx = this.editor.textContext();
    if (!ctx) {
      // focus moved to the popup or elsewhere; blur handling closes via mousedown
      if (!document.hasFocus()) return;
      return this.closeAc();
    }
    if (ctx.id !== ac.id || ctx.f !== ac.f || ctx.caret === null) return this.closeAc();
    const min = ac.kind === 'tag' ? ac.start + 1 : ac.start + 2;
    if (ctx.caret < min || ctx.caret > ac.end) this.closeAc();
  }

  private onOutsideDown(e: MouseEvent) {
    const t = e.target as Node;
    if (this.ac && !this.list.el.contains(t)) this.closeAc();
    if (this.date && !this.date.picker.el.contains(t) && !(t instanceof Element && t.closest('.overlay'))) {
      this.date.picker.close();
    }
  }

  private reposition() {
    if (this.ac && this.list.visible) {
      const ctx = this.editor.textContext();
      if (ctx) placePopup(this.list.el, caretRect(ctx.el));
    }
    this.date?.picker.reposition();
  }

  // ---- date picker ---------------------------------------------------------------------------

  /** A rendered date chip was clicked. */
  openDateChip(id: NodeId, s: number, e: number, anchor: HTMLElement) {
    const src = String(this.editor.tree.get(id)?.props.c ?? '');
    const spec = parseDateToken(src.slice(s, e));
    if (!spec) return;
    let rect = anchor.getBoundingClientRect();
    this.openDate({ id, f: 'c', s, e, typed: false, insert: false, spec }, () => {
      if (anchor.isConnected) rect = anchor.getBoundingClientRect();
      return rect;
    });
  }

  /** "add-date" command: edit the date under the caret / the item's date, or insert one. */
  addDate() {
    const ctx = this.editor.textContext();
    const id = ctx?.id ?? this.editor.targets()[0];
    if (!id) return false;
    const f = ctx?.f ?? 'c';
    const text = ctx ? ctx.text : String(this.editor.tree.get(id)?.props.c ?? '');
    const dates = extractDates(text);
    let caret = ctx?.caret ?? text.length;
    const at = ctx ? dates.find((d) => caret >= d.start && caret <= d.end) : dates[0];
    const anchor = () => {
      const c = this.editor.textContext();
      if (c && c.id === id) return caretRect(c.el);
      const row = this.editor.el.querySelector<HTMLElement>(`[data-id="${CSS.escape(id)}"] > .o-row .o-content`);
      return (row ?? this.editor.el).getBoundingClientRect();
    };
    if (at) this.openDate({ id, f, s: at.start, e: at.end, typed: false, insert: false, spec: at.spec }, anchor);
    else {
      // don't split a word: insert after it
      while (caret < text.length && !isWs(text[caret]) && !isWs(text[caret - 1])) caret++;
      this.openDate({ id, f, s: caret, e: caret, typed: false, insert: true, spec: null }, anchor);
    }
    return true;
  }

  private openDate(o: { id: NodeId; f: 'c' | 'n'; s: number; e: number; typed: boolean; insert: boolean; spec: DateSpec | null }, anchor: () => DOMRect) {
    this.closeAc();
    this.date?.picker.close();
    const picker = new DatePicker({
      spec: o.spec,
      startDay: this.host.startDay(),
      hour12: this.host.hour12(),
      anchor,
      onDone: (spec) => this.finishDate(state, spec),
      onDelete: o.spec ? () => this.finishDate(state, null) : undefined,
      onCancel: () => this.restoreCaret(state),
      onClose: () => {
        if (this.date === state) {
          this.date = null;
          if (!this.ac && this.editor.interceptor === interceptor) this.editor.interceptor = null;
        }
      },
    });
    const state: DateState = { picker, id: o.id, f: o.f, s: o.s, e: o.e, typed: o.typed, insert: o.insert,
      editing: this.editor.textContext()?.id === o.id };
    const interceptor = { onKey: (e: KeyboardEvent) => picker.handleKey(e) };
    this.date = state;
    this.editor.interceptor = interceptor;
    picker.open();
    // keys go to the picker itself when the item isn't being edited
    if (this.editor.textContext()?.id !== o.id) picker.el.focus({ preventScroll: true });
  }

  private finishDate(st: DateState, spec: DateSpec | null) {
    const ed = this.editor;
    const ctx = ed.textContext();
    const text = ctx && ctx.id === st.id && ctx.f === st.f ? ctx.text : String(ed.tree.get(st.id)?.props[st.f] ?? '');
    let { s, e } = st;
    if (spec) {
      let token = formatDateToken(this.host.encodeTimezone?.() ? withLocalTz(spec) : spec);
      const at = s;
      if (st.insert) {
        if (s > 0 && !isWs(text[s - 1])) token = ' ' + token;
        if (e < text.length && !isWs(text[e])) token += ' ';
      }
      ed.replaceRange(st.id, st.f, s, e, token, at + token.length);
    } else {
      // delete the date and one adjacent space
      if (text[e] === ' ' && isWs(text[s - 1])) e++;
      else if (text[s - 1] === ' ' && (e >= text.length || isWs(text[e]))) s--;
      ed.replaceRange(st.id, st.f, s, e, '', s);
    }
    // a chip clicked in display mode: stay in display mode
    if (!st.editing && !st.typed && !st.insert) ed.applySel({ k: 'none' });
  }

  /** Esc after `!` / add-date: put the caret back where it was. */
  private restoreCaret(st: DateState) {
    if (!st.typed && !st.insert) return;
    if (this.editor.textContext()?.id !== st.id) this.editor.applySel({ k: 'text', id: st.id, f: st.f, s: st.e, e: st.e });
  }
}

function plain(src: string): string {
  return inlineToPlain(src);
}
