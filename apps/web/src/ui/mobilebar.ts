// Bottom toolbar on phones (docs/03 §4 node toolbar). It shows while an item (or the title) is being edited, or
// items are selected, and sits at the bottom of the layout viewport, i.e. on top of the virtual keyboard.
// Buttons and their order are customizable (pref mobile_toolbar_options_order); the default set and order are
// Dynalist's (checked by qa/mobile.mjs).

import type { Workspace } from '../store/workspace.ts';
import { h, icon } from './dom.ts';
import { modal } from './overlay.ts';

/** [command id, text glyph (customize dialog, and buttons without an icon), title, icon name?] */
export const MOBILE_BUTTONS: [string, string, string, string?][] = [
  ['unindent', '⇤', 'Unindent', 'unindent'], ['indent', '⇥', 'Indent', 'indent'], ['zoom-in', '⊕', 'Zoom in', 'zoom'],
  ['delete-left', '🗑', 'Delete item', 'trash'], ['toggle-checked', '✓', 'Check off', 'check'], ['goto-note', '✎', 'Edit note', 'note'],
  ['add-date', '◷', 'Add date', 'date'], ['swap-up', '↑', 'Move up', 'up'], ['swap-down', '↓', 'Move down', 'down'],
  ['move-to', '➚', 'Move item to…', 'send'], ['toggle-checkbox', '☐', 'Toggle checkbox', 'checkbox'],
  ['color-menu', '🎨', 'Set color label', 'color'], ['heading-menu', 'H', 'Set heading level', 'heading'],
  ['get-link', '🔗', 'Get link', 'link'],
  // not in the default set
  ['collapse', '▸', 'Expand/collapse'], ['zoom-out', '⊖', 'Zoom out'], ['undo', '↶', 'Undo', 'undo'], ['redo', '↷', 'Redo', 'redo'],
  ['insert-hash', '#', 'Insert #'], ['insert-at', '@', 'Insert @'], ['cycle-color', '◐', 'Cycle color label'],
  ['cycle-heading', 'Hn', 'Cycle heading'], ['duplicate', '⧉', 'Duplicate'], ['newline', '↵', 'New line in item'],
  ['export', '⇪', 'Export item'], ['search', '⌕', 'Search', 'search'],
];
export const MOBILE_DEFAULT = [
  'unindent', 'indent', 'zoom-in', 'delete-left', 'toggle-checked', 'goto-note', 'add-date', 'swap-up', 'swap-down', 'move-to',
  'toggle-checkbox', 'color-menu', 'heading-menu', 'get-link',
];
/** Buttons that act on one item: hidden while several items are selected (as in the item menu). */
const SINGLE_ONLY = new Set(['zoom-in', 'goto-note', 'add-date', 'get-link', 'insert-hash', 'insert-at', 'newline']);

export interface BarState {
  /** field being edited, if any */
  f: 'c' | 'n' | null;
  /** number of block-selected items */
  block: number;
  readOnly: boolean;
}

export class MobileBar {
  readonly el = h('nav.mobile-bar', { hidden: true });
  private ws: Workspace;
  private run: (cmd: string) => void;
  private multi = false;

  constructor(ws: Workspace, run: (cmd: string) => void) {
    this.ws = ws;
    this.run = run;
    this.render();
  }

  private order(): string[] {
    const known = new Set(MOBILE_BUTTONS.map((b) => b[0]));
    return this.ws.pref<string[]>('mobile_toolbar_options_order', MOBILE_DEFAULT).filter((id) => known.has(id));
  }

  /** Show while editing an item's text or the title (not a note), or with a block selection. */
  update(s: BarState) {
    this.el.hidden = s.readOnly || !(s.f === 'c' || s.block > 0);
    const multi = s.block > 1;
    if (multi !== this.multi) {
      this.multi = multi;
      for (const b of this.el.querySelectorAll<HTMLElement>('button[data-cmd]')) b.hidden = multi && SINGLE_ONLY.has(b.dataset.cmd!);
    }
  }

  render() {
    const byId = new Map(MOBILE_BUTTONS.map((b) => [b[0], b]));
    // pointerdown default would move focus out of the edited item (and close the keyboard)
    const btn = (id: string, glyph: string, title: string, ic: string | undefined, run: () => void) =>
      h('button', { title, 'aria-label': title, 'data-cmd': id, hidden: this.multi && SINGLE_ONLY.has(id), onpointerdown: (e: Event) => e.preventDefault(), onclick: run },
        ic ? icon(ic) : glyph);
    this.el.replaceChildren(
      ...this.order().map((id) => {
        const [, glyph, title, ic] = byId.get(id)!;
        return btn(id, glyph, title, ic, () => this.run(id));
      }),
      btn('customize', '⚙', 'Customize toolbar', 'gear', () => this.customize()));
  }

  private customize() {
    let order = this.order();
    const list = h('div.tb-list');
    const paint = () => {
      const shown = new Set(order);
      const rows = [...order, ...MOBILE_BUTTONS.map((b) => b[0]).filter((id) => !shown.has(id))].map((id) => {
        const [, glyph, title] = MOBILE_BUTTONS.find((b) => b[0] === id)!;
        const on = shown.has(id);
        const i = order.indexOf(id);
        const move = (d: number) => {
          const j = i + d;
          if (j < 0 || j >= order.length) return;
          [order[i], order[j]] = [order[j]!, order[i]!];
          paint();
        };
        return h('div.tb-row', {},
          h('label', {}, h('input', { type: 'checkbox', checked: on, onchange: () => {
            order = on ? order.filter((x) => x !== id) : [...order, id];
            paint();
          } }), ` ${glyph}  ${title}`),
          on ? h('span', {}, h('button.btn', { onclick: () => move(-1) }, '↑'), h('button.btn', { onclick: () => move(1) }, '↓')) : null);
      });
      list.replaceChildren(...rows);
    };
    paint();
    modal('Customize toolbar', list, [
      { label: 'Reset', run: () => (this.ws.setPref('mobile_toolbar_options_order', undefined), true) },
      { label: 'Save', primary: true, run: () => this.ws.setPref('mobile_toolbar_options_order', order) },
    ]);
  }
}

/** A bottom sheet of choices plus Cancel (the toolbar's colour and heading pickers). Keeps the edited item focused. */
export function pickSheet(options: [string, () => void][]) {
  const close = () => bg.remove();
  const opt = (label: string, run: () => void) =>
    h('button', { onpointerdown: (e: Event) => e.preventDefault(), onclick: () => (close(), run()) }, label);
  const bg = h('div.sheet-bg', { onpointerdown: (e: Event) => e.preventDefault(), onclick: (e: Event) => e.target === bg && close() },
    h('div.sheet', {}, h('div.sheet-opts', {}, ...options.map(([l, r]) => opt(l, r))), opt('Cancel', () => {})));
  document.body.append(bg);
  return close;
}
