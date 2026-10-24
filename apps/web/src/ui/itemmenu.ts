// Item context menu (bullet right-click / ⋮ hover icon), docs/03 §4.
import { SORT_LABELS, type NodeId, visibleChildren } from '@outliner/core';
import type { Editor } from '../editor/editor.ts';
import { prettyCombo } from '../editor/keymap.ts';
import { h } from './dom.ts';
import { closeMenu, showMenu, type MenuItem } from './overlay.ts';

export interface ItemMenuExtra {
  bookmark(): void;
  exportItem(): void;
  setInbox(): void;
  getLink(): void;
  showReferences(): void;
  share(): void;
}

export function itemMenu(editor: Editor, id: NodeId, x: number, y: number, extra: ItemMenuExtra) {
  const t = editor.tree;
  const v = editor.view;
  const ids = id === editor.view.zoom ? [id] : editor.targets().length ? editor.targets() : [id];
  const multi = ids.length > 1;
  const n = t.get(id);
  if (!n) return;
  const isRoot = id === v.zoom;
  const key = (cmd: string) => {
    const k = editor.keymap.keysFor(cmd)[0];
    return k ? prettyCombo(k) : undefined;
  };
  const cmd = (label: string, c: string): MenuItem => ({ label, key: key(c), run: () => editor.execOn(ids, c) });
  const hasKids = n.children.length > 0;
  const shut = n.collapsed && hasKids;
  const items: MenuItem[] = [];
  // order and labels follow Dynalist's item menu (qa/menus.mjs); the extras are ours
  if (hasKids && !isRoot) items.push(cmd(shut ? 'Expand' : 'Collapse', 'collapse'));
  if (hasKids) items.push(cmd(shut ? 'Expand all' : 'Collapse all', 'collapse-all'));
  items.push({ label: 'Expand to level…', custom: h('div.mi', {}, h('span', {}, 'Expand to level'),
    h('span', {}, ...[1, 2, 3, 4].map((lv) => h('button.btn', { style: 'padding:0 6px;margin-left:2px', onclick: () => (closeMenu(), editor.execOn(ids, `expand-${lv}`)) }, String(lv))))) });
  if (hasKids && !isRoot) items.push(cmd(shut ? 'Expand all siblings' : 'Collapse all siblings', 'toggle-siblings'));
  if (!multi && !isRoot) items.push({ label: 'Zoom in', key: key('zoom-in'), run: () => editor.setZoom(id) });
  if (!multi) items.push({ ...cmd(n.props.n ? 'Edit note' : 'Add note', 'goto-note-of'), key: key('goto-note') });
  if (!isRoot) items.push(cmd('Delete', 'delete-left'));
  items.push(cmd('Delete checked items', 'delete-checked'));
  if (!multi) items.push({ label: 'Add to bookmarks', run: extra.bookmark });
  items.push({ label: 'Sort', sub: SORT_LABELS.map(([mode, label]) => ({ label, run: () => editor.execOn(ids, `sort-${mode}`) })) });
  items.push(cmd('Search and replace…', 'search-replace'));
  items.push({ label: '', sep: true });
  if (!isRoot) {
    const parent = n.parent!;
    const first = visibleChildren(t, parent, v)[0] === ids[0];
    if (!first) items.push(cmd('Indent', 'indent'));
    if (parent !== v.zoom) items.push(cmd('Unindent', 'unindent'));
    items.push(cmd('Move to…', 'move-to'));
    items.push(cmd('Insert template…', 'insert-template'));
    items.push(cmd('Duplicate', 'duplicate'));
  }
  items.push({ label: '', sep: true });
  items.push(cmd(ids.every((i) => t.get(i)?.props.cb) ? 'Remove checkbox' : 'Add checkbox', 'toggle-checkbox'));
  if (hasKids) items.push(cmd(n.children.every((c) => t.get(c)?.props.cb) ? 'Remove checkbox from children' : 'Add checkbox to children', 'toggle-children-checkbox'));
  if (!isRoot) items.push(cmd(ids.every((i) => t.get(i)?.props.ck) ? 'Uncheck' : 'Check off', 'toggle-checked'));
  if (hasKids) items.push(cmd('Uncheck all children', 'uncheck-children'));
  items.push(cmd(n.props.num ? 'Stop numbering children' : 'Number children', 'toggle-numbered'));
  items.push({ label: '', sep: true });
  if (!multi) items.push({ label: 'Get link', run: extra.getLink }, { label: 'Share…', run: extra.share });
  if (!multi) items.push({ label: 'Show all references', run: extra.showReferences });
  items.push({ label: 'Export…', run: extra.exportItem });
  if (!multi) items.push({ label: 'Set as inbox', run: extra.setInbox });
  items.push({ label: '', sep: true });
  items.push({ label: 'colors', custom: swatches(editor, id, ids, 'cl') });
  items.push({ label: 'headings', custom: swatches(editor, id, ids, 'h') });
  // drop trailing / doubled separators
  const clean = items.filter((it, i) => !(it.sep && (i === 0 || items[i - 1]!.sep || i === items.length - 1)));
  showMenu(x, y, clean);
}

/** Color label ('cl') or heading ('h') swatch strip for `ids`; the current value (of `id`) is outlined. */
export function swatches(editor: Editor, id: NodeId, ids: NodeId[], k: 'cl' | 'h'): HTMLElement {
  const t = editor.tree;
  const cur = ids.every((i) => (t.get(i)?.props[k] ?? 0) === (t.get(id)?.props[k] ?? 0)) ? Number(t.get(id)?.props[k] ?? 0) : -1;
  const swatch = (val: number, text: string) => {
    const el = h('div.sw', { onmousedown: (e: Event) => e.preventDefault(), onclick: () => (closeMenu(), editor.execOn(ids, `${k === 'cl' ? 'color' : 'heading'}-${val}`)) }, text);
    if (k === 'cl' && val) el.classList.add(`o-cl${val}`);
    if (cur === val) el.style.outline = '2px solid var(--accent)';
    return el;
  };
  return h('div.swatches', {}, ...(k === 'cl'
    ? [0, 1, 2, 3, 4, 5, 6].map((c) => swatch(c, c ? 'A' : '∅'))
    : [0, 1, 2, 3].map((lv) => swatch(lv, lv ? `H${lv}` : '∅'))));
}
