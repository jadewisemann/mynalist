// Settings modal (docs/03 §8): Preferences (with "Show advanced settings"), Keymap, Sync & data.

import { COMMANDS, comboOf, prettyCombo, type Keymap } from '../editor/keymap.ts';
import type { Workspace } from '../store/workspace.ts';
import { h, toast } from './dom.ts';
import { modal } from './overlay.ts';

export interface SettingsHost {
  ws: Workspace;
  keymap(): Keymap;
  /** sync server section */
  server(): HTMLElement;
  /** describe the current inbox location, and pick a new one */
  inboxLabel(): string;
  pickInbox(): void;
  downloadBackup(): void;
  importFiles(): void;
  /** calendar feed / shares / uploads sections */
  serverExtras(): HTMLElement[];
}

export const THEMES: [string, string][] = [
  ['auto', 'Auto-detect'], ['light', 'Light'], ['dark', 'Dark'], ['sepia', 'Sepia'], ['scifi', 'Sci-Fi'],
  ['sakura', 'Sakura'], ['darkgold', 'Dark gold'], ['christmas', 'Christmas'],
];

export const FONTS: [string, string][] = [
  ['default', 'Fira Sans (default)'], ['system', 'System font'], ['inter', 'Inter (if installed)'], ['arial', 'Arial'],
  ['helvetica', 'Helvetica'], ['roboto', 'Roboto (if installed)'], ['serif', 'Serif (Georgia)'], ['mono', 'Monospace'],
];

export const EA_FONTS: [string, string][] = [
  ['default', 'Default'], ['pretendard', 'Pretendard'], ['notosanskr', 'Noto Sans KR'], ['malgun', 'Malgun Gothic'],
  ['batang', 'Batang'], ['yahei', 'Microsoft YaHei'], ['kaiti', 'KaiTi'], ['meiryo', 'Meiryo'], ['hiragino', 'Hiragino Kaku Gothic'],
];

export function settingsDialog(host: SettingsHost, tab: 'prefs' | 'keymap' | 'data' = 'prefs') {
  const ws = host.ws;
  const sel = (key: string, def: string, opts: [string, string][]) => {
    const s = h('select', { onchange: () => ws.setPref(key, s.value) },
      ...opts.map(([v, l]) => h('option', { value: v, selected: ws.pref(key, def) === v }, l)));
    return s;
  };
  const chk = (key: string, def: boolean) => {
    const c = h('input', { type: 'checkbox', checked: ws.pref(key, def), onchange: () => ws.setPref(key, c.checked) });
    return c;
  };
  const text = (key: string, placeholder: string) => {
    const i = h('input', { value: ws.pref(key, ''), placeholder, onchange: () => ws.setPref(key, i.value.trim() || undefined) });
    return i;
  };
  const row = (label: string, input: HTMLElement, hint?: string) =>
    input instanceof HTMLInputElement && input.type === 'checkbox'
      ? h('label.field.row', {}, input, label, hint ? h('span.hint', {}, hint) : null)
      : h('label.field', {}, label, input, hint ? h('span.hint', {}, hint) : null);
  const section = (title: string, ...rows: HTMLElement[]) => h('section.set-sec', {}, h('h3', {}, title), ...rows);

  const inboxLabel = h('span', {}, host.inboxLabel());
  const advanced = h('div', { hidden: true },
    section('Advanced',
      row('Tag background color', chk('tag_bg', false)),
      row('Document border', chk('doc_border', false)),
      row('Auto-focus on first item', chk('auto_focus_first_item', false)),
      row('Show search tip popup', chk('show_search_tips', true)),
      row('Font for dyslexia', sel('dyslexia_font', 'none', [['none', 'None'], ['opendyslexic', 'OpenDyslexic (if installed)']])),
      row('Move item to', sel('move_item_sent_to_position', 'bottom', [['bottom', 'End of destination'], ['top', 'Start of destination']]),
        'Also used when dropping onto a collapsed item.'),
      row('Show "Clean up newlines" in document menu', chk('show_cleanup_newlines_option', false)),
      row('Export visible items only (default)', chk('export_visible_only', false)),
      row('Custom CSS', (() => {
        const ta = h('textarea.css-input', { placeholder: '.o-tag { color: teal; }', onchange: () => ws.setPref('custom_css', ta.value || undefined) });
        ta.value = ws.pref('custom_css', '');
        return ta;
      })()),
    ));
  const prefs = h('div', {},
    section('Appearance',
      row('Theme', sel('theme', 'auto', THEMES)),
      row('List density', sel('density', 'cozy', [['comfortable', 'Comfortable'], ['cozy', 'Cozy'], ['compact', 'Compact']])),
      row('Highlight current item', chk('current_highlight', false)),
      row('Narrow document layout', chk('center_doc', true)),
      row('Display images inline', chk('display_images_inline', false)),
      row('Right-to-left', chk('right_to_left', false)),
      row('Use true black (dark theme)', chk('use_true_black', false))),
    section('Viewing options',
      row('Show checked items', chk('show_checked', true)),
      row('Notes', sel('show_notes', 'show', [['show', 'Show'], ['teaser', '1st line'], ['hide', 'Hide']]))),
    section('Font options',
      row('Font size', sel('font_size', 'medium', [['small', 'Small'], ['medium', 'Medium'], ['large', 'Large']])),
      row('Font', sel('font', 'default', FONTS)),
      row('Eastern-Asian font', sel('eastern_asian_font', 'default', EA_FONTS))),
    section('Input',
      row('Enable spell checker', chk('spellcheck', false)),
      row('Show word count', chk('word_count', false)),
      row('Auto-complete tags from all docs', chk('global_tag_autocomplete', false)),
      row('WorkFlowy-style shortcuts', chk('workflowy_shortcuts', false))),
    section('Date',
      row('Time format', sel('time_format', '12h', [['12h', '12-hour'], ['24h', '24-hour']])),
      row('First day of week', sel('start_day', 'sun', [['sat', 'Saturday'], ['sun', 'Sunday'], ['mon', 'Monday']])),
      row('Date & time display format', text('custom_date_display_format', 'relative'), 'Empty or "relative" for "Tomorrow, 3 pm"; or a pattern like "YYYY-MM-DD HH:mm".'),
      row('Date display format (no time)', text('custom_date_display_format_no_time', 'relative'), 'e.g. "ddd, MMM D"'),
      row('Encode timezone in new dates', chk('show_timezone', false)),
      row('Highlight overdue dates', chk('highlight_overdue', false)),
      row('Recurring date', sel('recurrence_check_behavior', 'new_instance', [['new_instance', 'Next date appears in new item'], ['update', 'Existing date gets updated']]))),
    section('Internal links', row('Show backlinks', chk('show_backlinks', false))),
    section('Your inbox',
      h('div.field', {}, h('span', {}, 'Inbox destination: '), inboxLabel, ' ',
        h('button.btn', { onclick: () => host.pickInbox() }, 'Change…')),
      row('Move to position', sel('inbox_move_position', 'bottom', [['bottom', 'End of inbox'], ['top', 'Start of inbox']]))),
    section('Control',
      row('Use bullet point to zoom in', chk('use_bullet_to_zoom', false)),
      row('Unindent items in-place', chk('unindent_in_place', false), 'Following siblings become children of the unindented item.')),
    h('button.btn.link', { onclick: (e: MouseEvent) => {
      advanced.hidden = !advanced.hidden;
      (e.target as HTMLElement).textContent = advanced.hidden ? 'Show advanced settings' : 'Hide advanced settings';
    } }, 'Show advanced settings'),
    advanced,
  );

  const data = h('div', {},
    section('Sync server', host.server()),
    ...host.serverExtras(),
    section('Backup',
      h('p.hint', {}, 'A zip of every document as plain text and OPML, in its folder structure. The sync server also keeps daily database backups.'),
      h('button.btn', { onclick: () => host.downloadBackup() }, 'Download backup')),
    section('Import', h('button.btn', { onclick: () => host.importFiles() }, 'Import OPML / text files…'),
      h('p.hint', {}, 'From WorkFlowy: export as OPML there, then import the file here.')),
  );

  const panes = { prefs, keymap: keymapEditor(host), data };
  const tabs = h('div.set-tabs');
  const body = h('div.settings', {}, tabs);
  const show = (k: keyof typeof panes) => {
    for (const b of tabs.children) b.classList.toggle('active', (b as HTMLElement).dataset.k === k);
    body.replaceChildren(tabs, panes[k]);
  };
  for (const [k, l] of [['prefs', 'Preferences'], ['keymap', 'Keymap'], ['data', 'Sync & data']] as const) {
    tabs.append(h('button.tab', { 'data-k': k, onclick: () => show(k) }, l));
  }
  show(tab);
  modal('Settings', body, [{ label: 'Done', primary: true }]);
}

/** Commands whose keys can't be changed (they are part of text editing itself). */
const FIXED = new Set(['undo', 'redo', 'select-up', 'select-down']);

function keymapEditor(host: SettingsHost): HTMLElement {
  const ws = host.ws;
  const overrides = () => ({ ...ws.pref<Record<string, string[]>>('shortcut_override', {}) });
  const filter = h('input', { placeholder: 'Search commands…' });
  const list = h('div.km-list');
  let recording: { id: string; el: HTMLElement } | null = null;
  const save = (o: Record<string, string[]>) => ws.setPref('shortcut_override', Object.keys(o).length ? o : undefined);
  const paint = () => {
    const km = host.keymap();
    const q = filter.value.toLowerCase();
    list.replaceChildren(...COMMANDS.filter((c) => !q || c.label.toLowerCase().includes(q) || c.id.includes(q)).map((c) => {
      const keys = km.keysFor(c.id);
      const custom = c.id in overrides();
      const chips = keys.map((k) => h('span.km-key', {}, prettyCombo(k),
        FIXED.has(c.id) ? null : h('button.km-x', { title: 'Remove', onclick: () => {
          const o = overrides();
          o[c.id] = keys.filter((x) => x !== k);
          save(o);
          setTimeout(paint);
        } }, '×')));
      const add = FIXED.has(c.id) ? h('span.hint', {}, 'fixed') : h('button.btn.km-add', { onclick: (e: MouseEvent) => {
        recording = { id: c.id, el: e.currentTarget as HTMLElement };
        recording.el.textContent = 'Press keys…';
      } }, '+');
      const reset = custom ? h('button.btn.link', { onclick: () => {
        const o = overrides();
        delete o[c.id];
        save(o);
        setTimeout(paint);
      } }, 'Reset to default') : null;
      return h('div.km-row', {}, h('span.km-label', {}, c.label), h('span.km-keys', {}, ...chips, add, reset));
    }));
  };
  filter.addEventListener('input', paint);
  const onKey = (e: KeyboardEvent) => {
    if (!recording || !list.isConnected) return;
    if (['Shift', 'Control', 'Alt', 'Meta'].includes(e.key)) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    const id = recording.id;
    recording = null;
    if (e.key === 'Escape') return paint();
    const combo = comboOf(e);
    if (!/^(Mod|Ctrl|Alt|Shift)\+/.test(combo) && !/^F\d+$/.test(combo)) {
      toast('Shortcuts need at least one modifier (Ctrl, Alt or Shift)');
      return paint();
    }
    const km = host.keymap();
    const o = overrides();
    const taken = km.resolve(combo);
    if (taken && taken !== id) o[taken] = km.keysFor(taken).filter((k) => k !== combo);
    o[id] = [...new Set([...km.keysFor(id), combo])];
    save(o);
    setTimeout(paint);
  };
  document.addEventListener('keydown', onKey, true);
  const el = h('div.keymap', {}, filter, list,
    h('button.btn', { onclick: () => (save({}), setTimeout(paint)) }, 'Reset all shortcuts'));
  const stop = new MutationObserver(() => {
    if (!el.isConnected && !document.querySelector('.settings')) {
      document.removeEventListener('keydown', onKey, true);
      stop.disconnect();
    }
  });
  stop.observe(document.body, { childList: true });
  paint();
  return el;
}
