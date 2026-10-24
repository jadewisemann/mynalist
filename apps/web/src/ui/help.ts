// "Shortcuts & formatting" side panel (docs/03 §9): triggers, formatting, every command with its keys.

import { COMMANDS, prettyCombo, type Keymap } from '../editor/keymap.ts';
import { h, icon } from './dom.ts';

const TRIGGERS: [string, string][] = [
  ['#  or  @', 'Tag autocomplete'],
  ['!', 'Date picker'],
  ['[[', 'Link to another item or document'],
];

const FORMATS: [string, string][] = [
  ['**bold**', 'Bold'], ['__italic__', 'Italic'], ['~~strike~~', 'Strikethrough'], ['==highlight==', 'Highlight'],
  ['`code`', 'Inline code'], ['```lang↵…```', 'Code block'], ['[text](url)', 'Link'], ['![alt](url)', 'Image'],
  ['$$E=mc^2$$', 'LaTeX'], ['#tag @tag', 'Tags'], ['!(2026-01-31 14:00)', 'Date'], ['!(2026-01-31 | 1w)', 'Recurring date'],
];

export class HelpPanel {
  readonly el = h('aside.help-panel', { hidden: true });
  private keymap: () => Keymap;

  constructor(keymap: () => Keymap) {
    this.keymap = keymap;
  }

  toggle(force?: boolean) {
    this.el.hidden = force === undefined ? !this.el.hidden : !force;
    if (!this.el.hidden) this.render();
  }

  render() {
    const km = this.keymap();
    const table = (rows: [string, string][]) => h('table.help-table', {}, ...rows.map(([a, b]) => h('tr', {}, h('td', {}, a), h('td', {}, b))));
    const groups = new Map<string, [string, string][]>();
    for (const c of COMMANDS) {
      const keys = km.keysFor(c.id);
      const list = groups.get(c.group) ?? [];
      list.push([c.label, keys.length ? keys.map(prettyCombo).join(' / ') : '—']);
      groups.set(c.group, list);
    }
    this.el.replaceChildren(
      h('div.help-head', {}, h('h2', {}, 'Shortcuts & formatting'), h('span.spacer'),
        h('button.icon-btn', { title: 'Close', onclick: () => this.toggle(false) }, icon('close'))),
      h('div.help-body', {},
        h('h3', {}, 'Triggers'), table(TRIGGERS.map(([a, b]) => [b, a])),
        h('h3', {}, 'Formatting'), table(FORMATS.map(([a, b]) => [b, a])),
        ...[...groups].flatMap(([g, rows]) => [h('h3', {}, g), table(rows)]),
        h('p.hint', {}, 'Change shortcuts in Settings → Keymap.')));
  }
}
