// Bottom toolbar on phones (docs/03 §4 node toolbar). Buttons and their order are customizable
// (pref mobile_toolbar_options_order).

import type { Workspace } from '../store/workspace.ts';
import { h } from './dom.ts';
import { modal } from './overlay.ts';

export const MOBILE_BUTTONS: [string, string, string][] = [
  ['unindent', '⇤', 'Unindent'], ['indent', '⇥', 'Indent'], ['swap-up', '↑', 'Move up'], ['swap-down', '↓', 'Move down'],
  ['toggle-checked', '☑', 'Check off'], ['goto-note', '✎', 'Edit note'], ['collapse', '▸', 'Expand/collapse'],
  ['zoom-in', '⊕', 'Zoom in'], ['zoom-out', '⊖', 'Zoom out'], ['undo', '↶', 'Undo'], ['redo', '↷', 'Redo'],
  ['insert-hash', '#', 'Insert #'], ['insert-at', '@', 'Insert @'], ['add-date', '◷', 'Add date'],
  ['toggle-checkbox', '☐', 'Toggle checkbox'], ['cycle-color', '🎨', 'Color label'], ['cycle-heading', 'H', 'Heading'],
  ['duplicate', '⧉', 'Duplicate'], ['delete-left', '🗑', 'Delete item'], ['newline', '↵', 'New line in item'],
  ['export', '⇪', 'Export item'], ['search', '⌕', 'Search'],
];
const DEFAULT = ['unindent', 'indent', 'swap-up', 'swap-down', 'toggle-checked', 'goto-note', 'collapse', 'undo', 'redo', 'insert-hash', 'add-date', 'search'];

export class MobileBar {
  readonly el = h('nav.mobile-bar');
  private ws: Workspace;
  private run: (cmd: string) => void;

  constructor(ws: Workspace, run: (cmd: string) => void) {
    this.ws = ws;
    this.run = run;
    this.render();
  }

  private order(): string[] {
    const known = new Set(MOBILE_BUTTONS.map((b) => b[0]));
    return this.ws.pref<string[]>('mobile_toolbar_options_order', DEFAULT).filter((id) => known.has(id));
  }

  render() {
    const byId = new Map(MOBILE_BUTTONS.map((b) => [b[0], b]));
    const btn = (label: string, title: string, run: () => void) =>
      h('button', { title, 'aria-label': title, onpointerdown: (e: Event) => e.preventDefault(), onclick: run }, label);
    this.el.replaceChildren(
      ...this.order().map((id) => {
        const [, glyph, title] = byId.get(id)!;
        return btn(glyph, title, () => this.run(id));
      }),
      btn('⚙', 'Customize toolbar', () => this.customize()));
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
