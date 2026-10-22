// Item context menu (bullet right-click / ⋮ hover icon), docs/03 §4.
import { type NodeId, visibleChildren } from '@outliner/core';
import type { Editor } from '../editor/editor.ts';
import { prettyCombo } from '../editor/keymap.ts';
import { h } from './dom.ts';
import { closeMenu, showMenu, type MenuItem } from './overlay.ts';

export function itemMenu(editor: Editor, id: NodeId, x: number, y: number, extra: { bookmark: () => void; exportItem: () => void; setInbox: () => void }) {
  const t = editor.tree;
  const v = editor.view;
  const ids = editor.targets().length ? editor.targets() : [id];
  const multi = ids.length > 1;
  const n = t.get(id);
  if (!n) return;
  const isRoot = id === v.zoom;
  const key = (cmd: string) => {
    const k = editor.keymap.keysFor(cmd)[0];
    return k ? prettyCombo(k) : undefined;
  };
  const cmd = (label: string, c: string): MenuItem => ({ label, key: key(c), run: () => editor.exec(c) });
  const hasKids = n.children.length > 0;
  const items: MenuItem[] = [];
  if (hasKids && !isRoot) items.push(cmd(n.collapsed ? 'Expand' : 'Collapse', 'collapse'));
  if (hasKids) items.push(cmd('Expand/collapse all', 'collapse-all'));
  if (hasKids) {
    items.push({ label: 'Expand to level…', custom: h('div.mi', {}, h('span', {}, 'Expand to level'),
      h('span', {}, ...[1, 2, 3, 4].map((lv) => h('button.btn', { style: 'padding:0 6px;margin-left:2px', onclick: () => (closeMenu(), editor.exec(`expand-${lv}`)) }, String(lv))))) });
  }
  if (!isRoot) items.push(cmd('Expand/collapse siblings', 'toggle-siblings'));
  if (!multi && !isRoot) items.push({ label: 'Zoom in', key: key('zoom-in'), run: () => editor.setZoom(id) });
  if (!multi) items.push(cmd(n.props.n ? 'Edit note' : 'Add note', 'goto-note-of'));
  items.push(cmd('Delete checked items', 'delete-checked'));
  if (!isRoot) items.push(cmd('Delete', 'delete-left'));
  if (!multi) items.push({ label: 'Add to bookmarks', run: extra.bookmark });
  if (hasKids) {
    items.push({ label: 'Sort children', custom: h('div.mi', {}, h('span', {}, 'Sort children'),
      h('span', {}, h('button.btn', { style: 'padding:0 6px', onclick: () => (closeMenu(), editor.exec('sort-asc')) }, 'A→Z'),
        h('button.btn', { style: 'padding:0 6px;margin-left:2px', onclick: () => (closeMenu(), editor.exec('sort-desc')) }, 'Z→A'))) });
  }
  items.push({ label: '', sep: true });
  if (!isRoot) {
    const parent = n.parent!;
    const first = visibleChildren(t, parent, v)[0] === ids[0];
    if (!first) items.push(cmd('Indent', 'indent'));
    if (parent !== v.zoom) items.push(cmd('Unindent', 'unindent'));
    items.push(cmd('Move to…', 'move-to'));
    items.push(cmd('Duplicate', 'duplicate'));
  }
  items.push({ label: '', sep: true });
  items.push(cmd(ids.every((i) => t.get(i)?.props.cb) ? 'Remove checkbox' : 'Add checkbox', 'toggle-checkbox'));
  if (hasKids) items.push(cmd('Toggle checkbox on children', 'toggle-children-checkbox'));
  if (!isRoot) items.push(cmd(ids.every((i) => t.get(i)?.props.ck) ? 'Uncheck' : 'Check off', 'toggle-checked'));
  if (hasKids) items.push(cmd('Uncheck all children', 'uncheck-children'));
  items.push(cmd(n.props.num ? 'Stop numbering children' : 'Number children', 'toggle-numbered'));
  items.push({ label: '', sep: true });
  if (!multi) items.push(cmd('Copy link', 'copy-link'));
  items.push({ label: 'Export…', run: extra.exportItem });
  if (!multi) items.push({ label: 'Set as inbox', run: extra.setInbox });
  items.push({ label: '', sep: true });
  const cur = (k: 'cl' | 'h') => (ids.every((i) => (t.get(i)?.props[k] ?? 0) === (t.get(id)?.props[k] ?? 0)) ? Number(t.get(id)?.props[k] ?? 0) : -1);
  const swatch = (k: 'cl' | 'h', val: number, text: string) => {
    const el = h('div.sw', { onmousedown: (e: Event) => e.preventDefault(), onclick: () => (closeMenu(), editor.exec(`${k === 'cl' ? 'color' : 'heading'}-${val}`)) }, text);
    if (k === 'cl' && val) el.classList.add(`o-cl${val}`);
    if (cur(k) === val) el.style.outline = '2px solid var(--accent)';
    return el;
  };
  items.push({ label: 'colors', custom: h('div.swatches', {}, ...[0, 1, 2, 3, 4, 5, 6].map((c) => swatch('cl', c, c ? 'A' : '∅'))) });
  items.push({ label: 'headings', custom: h('div.swatches', {}, ...[0, 1, 2, 3].map((lv) => swatch('h', lv, lv ? `H${lv}` : '∅'))) });
  // drop trailing / doubled separators
  const clean = items.filter((it, i) => !(it.sep && (i === 0 || items[i - 1]!.sep || i === items.length - 1)));
  showMenu(x, y, clean);
}
