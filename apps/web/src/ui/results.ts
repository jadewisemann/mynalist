// Flat search ("Search results for … in …") and global search ("Search everywhere") screens, docs/03 §5.1.
// They replace the document view; Esc or "Go back" returns to it.

import {
  ROOT, SORT_LABELS, compileQuery, searchTree, sortComparator, type NodeId, type SortMode, type Tree,
} from '@outliner/core';
import { renderText } from '../editor/render.ts';
import { h } from './dom.ts';

export interface ResultsHost {
  docs(): { id: string; title: string }[];
  tree(doc: string): Tree;
  title(doc: string): string;
  /** checked items visible in this doc (global or per-doc setting) */
  showChecked(doc: string): boolean;
  open(doc: string, id: NodeId): void;
  back(): void;
  /** query or sort changed from inside the screen */
  update(q: string, sort: SortMode | ''): void;
}

export interface ResultsState {
  mode: 'flat' | 'global';
  q: string;
  sort: SortMode | '';
  doc: string | null;
  zoom: NodeId;
}

const SORTS = SORT_LABELS.filter(([m]) => m !== 'reverse');

export function breadcrumb(t: Tree, id: NodeId, stop: NodeId = ROOT): string {
  const anc = t.ancestors(id);
  return anc
    .slice(anc.indexOf(stop) + 1)
    .map((a) => String(t.get(a)?.props.c ?? '').slice(0, 40))
    .join(' › ');
}

export class ResultsView {
  readonly el = h('div.results-view');
  private input = h('input.results-q', { placeholder: 'Search everywhere…' });
  private state: ResultsState | null = null;
  private timer = 0;
  private host: ResultsHost;

  constructor(host: ResultsHost) {
    this.host = host;
    this.el.hidden = true;
    this.el.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && !e.isComposing) {
        e.preventDefault();
        this.host.back();
      }
    });
    this.input.addEventListener('input', () => {
      clearTimeout(this.timer);
      this.timer = window.setTimeout(() => this.host.update(this.input.value, this.state?.sort ?? ''), 250);
    });
  }

  get active(): boolean {
    return !this.el.hidden;
  }

  show(st: ResultsState) {
    const first = !this.state || this.state.mode !== st.mode || this.el.hidden;
    this.state = st;
    this.el.hidden = false;
    this.render();
    if (first) (st.mode === 'global' ? this.input : this.el).focus();
  }

  hide() {
    this.el.hidden = true;
    this.state = null;
  }

  render() {
    const st = this.state;
    if (!st) return;
    const m = compileQuery(st.q, Date.now());
    const hl = { highlight: m.terms };
    const docs = st.mode === 'global' ? this.host.docs() : st.doc ? [{ id: st.doc, title: this.host.title(st.doc) }] : [];
    let total = 0;
    let filtered = 0;
    const groups: { doc: string; title: string; t: Tree; ids: NodeId[] }[] = [];
    for (const d of docs) {
      const t = this.host.tree(d.id);
      const scope = st.mode === 'flat' ? st.zoom : ROOT;
      const shown = searchTree(t, m, scope, { showChecked: this.host.showChecked(d.id) }).matches;
      const all = m.empty ? shown : searchTree(t, m, scope, { showChecked: true }).matches;
      filtered += all.length - shown.length;
      if (!shown.length) continue;
      const cmp = st.sort ? sortComparator(t, st.sort) : null;
      groups.push({ doc: d.id, title: d.title, t, ids: cmp ? [...shown].sort(cmp) : shown });
      total += shown.length;
    }
    const sortSel = h('select', { onchange: () => this.host.update(st.q, sortSel.value as SortMode | '') },
      h('option', { value: '' }, 'None'), ...SORTS.map(([v, l]) => h('option', { value: v, selected: st.sort === v }, l)));
    const head = st.mode === 'flat'
      ? h('div.results-head', {}, h('h2', {}, `Search results for "${st.q}" in "${st.doc ? this.host.title(st.doc) : ''}"`))
      : h('div.results-head', {}, h('h2', {}, 'Search everywhere'), this.input);
    if (st.mode === 'global' && document.activeElement !== this.input) this.input.value = st.q;
    const bar = h('div.results-bar', {},
      h('span', {}, total ? `Found ${total} match${total === 1 ? '' : 'es'}:` : ''),
      h('span.spacer'),
      h('label', {}, 'Sort by ', sortSel),
      h('button.btn', { onclick: () => this.host.back() }, 'Go back [Esc]'));
    const row = (doc: string, t: Tree, id: NodeId) => {
      const n = t.get(id)!;
      const text = h('div.res-text');
      text.innerHTML = renderText(String(n.props.c ?? ''), hl) || '&nbsp;';
      const note = typeof n.props.n === 'string' && n.props.n ? h('div.res-note') : null;
      if (note) note.innerHTML = renderText(String(n.props.n), hl);
      const crumb = breadcrumb(t, id, st.mode === 'flat' ? st.zoom : ROOT);
      return h('div.res-item', {
        class: n.props.ck ? 'checked' : undefined,
        tabindex: 0,
        onclick: (e: MouseEvent) => (e.target as HTMLElement).closest('a') ? undefined : this.host.open(doc, id),
        onkeydown: (e: KeyboardEvent) => e.key === 'Enter' && this.host.open(doc, id),
      }, crumb ? h('div.res-crumb', {}, crumb) : null, text, note);
    };
    let body: HTMLElement;
    if (!total) {
      body = h('div.results-empty', {}, filtered
        ? `No results found. ${filtered} match${filtered === 1 ? ' has' : 'es have'} been filtered out by your filter settings. Append "is:completed" to your search to include checked items.`
        : st.q ? 'No results found.' : 'Type to search all documents.');
    } else if (st.mode === 'flat') {
      body = h('div.results-list', {}, ...groups[0]!.ids.map((id) => row(groups[0]!.doc, groups[0]!.t, id)));
    } else {
      body = h('div.results-list', {}, ...groups.map((g) =>
        h('div.res-card', {}, h('div.res-doc', {}, g.title), ...g.ids.map((id) => row(g.doc, g.t, id)))));
    }
    if (st.mode === 'flat' && filtered && total) bar.firstElementChild!.textContent += ` (${filtered} hidden by your filter settings)`;
    this.el.replaceChildren(head, bar, body);
    this.el.tabIndex = -1;
  }
}
