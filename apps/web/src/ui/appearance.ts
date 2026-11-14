// Settings › Look: the user's appearance TOML (design/schema.md), edited two ways: one control per schema entry
// (generated from the schema: color, length, ratio, weight, font, choice, with their ranges), and the TOML text
// itself, checked as it is typed with the error shown at its line. Only a valid document is stored (pref
// `appearance`), and main.ts applies it at once. It exports and imports as a .toml file, so a custom theme is a file.

import { BASE_VALUES, parseAppearance, toToml, type Appearance } from '../design/appearance.ts';
import { SCHEMA, type Entry, type Value } from '../design/tokens.ts';
import type { Workspace } from '../store/workspace.ts';
import { h, toast } from './dom.ts';

const WEIGHTS = [100, 200, 300, 400, 500, 600, 700, 800, 900];

/** `#rgb` / `#rrggbb` as `#rrggbb` for a color input; null for what it cannot show (alpha, references). */
function opaqueHex(v: string): string | null {
  const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(v);
  if (!m) return null;
  return m[1].length === 3 ? `#${[...m[1]].map((c) => c + c).join('')}` : v.toLowerCase();
}

export function appearanceEditor(ws: Workspace): HTMLElement {
  let values: Appearance = (() => {
    const r = parseAppearance(ws.pref('appearance', ''));
    return 'values' in r ? r.values : [];
  })();
  const current = (id: string) => values.find(([k]) => k === id)?.[1];

  const text = h('textarea.toml-input', { spellcheck: false, placeholder: '[color]\ntag = "#008080"\n\n[type]\nsize = 17' });
  const error = h('div.error-text', { role: 'status' });
  const save = (next: Appearance, fromText = false) => {
    values = next;
    const toml = toToml(values);
    if (!fromText) text.value = toml;
    ws.setPref('appearance', values.length ? toml : undefined);
    paintControls();
  };
  const showError = (e: { line: number; message: string } | null) => {
    error.textContent = e ? (e.line ? `Line ${e.line}: ${e.message}` : e.message) : '';
    if (e?.line) {
      // put the caret on the line, so the error is where the eye goes
      const lines = text.value.split('\n');
      const start = lines.slice(0, e.line - 1).reduce((n, l) => n + l.length + 1, 0);
      text.dataset.errorLine = String(e.line);
      text.setSelectionRange(start, start + (lines[e.line - 1]?.length ?? 0));
    } else delete text.dataset.errorLine;
  };
  text.addEventListener('input', () => {
    const r = parseAppearance(text.value);
    if ('error' in r) return void (error.textContent = r.error.line ? `Line ${r.error.line}: ${r.error.message}` : r.error.message);
    error.textContent = '';
    save(r.values, true);
  });
  text.addEventListener('change', () => {
    const r = parseAppearance(text.value);
    showError('error' in r ? r.error : null);
  });

  const set = (id: string, v: Value | undefined) => {
    const next = values.filter(([k]) => k !== id);
    if (v !== undefined && v !== '') next.push([id, v]);
    save(next);
  };

  const control = (e: Entry): HTMLElement => {
    const v = current(e.id);
    const base = BASE_VALUES.get(e.id)!;
    const shown = v ?? base;
    switch (e.kind) {
      case 'color': {
        const field = h('input', { value: v === undefined ? '' : String(v), placeholder: String(base), size: 18 }) as HTMLInputElement;
        field.addEventListener('change', () => {
          const err = checkOne(e, field.value.trim());
          if (err) return toast(`${e.id}: ${err}`);
          set(e.id, field.value.trim() || undefined);
        });
        const hex = opaqueHex(String(shown));
        const swatch = h('input', { type: 'color', value: hex ?? '#000000', title: hex ? 'Pick a color' : 'Not an opaque color: edit the text' }) as HTMLInputElement;
        swatch.addEventListener('change', () => set(e.id, swatch.value));
        return h('span.km-keys', {}, swatch, field);
      }
      case 'length':
      case 'ratio': {
        const step = e.kind === 'ratio' ? '0.0001' : '0.5';
        const field = h('input', { type: 'number', min: e.min, max: e.max, step, value: v === undefined ? '' : String(v), placeholder: String(base) }) as HTMLInputElement;
        field.addEventListener('change', () => {
          if (field.value === '') return set(e.id, undefined);
          const n = Number(field.value);
          const err = checkOne(e, n);
          if (err) return toast(`${e.id}: ${err}`);
          set(e.id, n);
        });
        return h('span.km-keys', {}, field, e.kind === 'length' ? 'px' : '×');
      }
      case 'weight':
      case 'choice': {
        const opts = e.kind === 'weight' ? WEIGHTS.map(String) : e.choices!;
        const sel = h('select', {}, h('option', { value: '' }, `(${base})`), ...opts.map((o) => h('option', { value: o, selected: String(v) === o }, o))) as HTMLSelectElement;
        sel.addEventListener('change', () => set(e.id, sel.value === '' ? undefined : e.kind === 'weight' ? Number(sel.value) : sel.value));
        return h('span.km-keys', {}, sel);
      }
      case 'font': {
        const field = h('input', { value: v === undefined ? '' : String(v), placeholder: String(base) }) as HTMLInputElement;
        field.addEventListener('change', () => set(e.id, field.value.trim() || undefined));
        return h('span.km-keys', {}, field);
      }
    }
  };

  const tables = [...new Set(SCHEMA.map((e) => e.table))];
  const controls = h('div');
  const paintControls = () => {
    const open = new Set([...controls.querySelectorAll('details[open]')].map((d) => (d as HTMLElement).dataset.table));
    controls.replaceChildren(...tables.map((t) => {
      const entries = SCHEMA.filter((e) => e.table === t);
      const changed = entries.filter((e) => current(e.id) !== undefined).length;
      return h('details', { 'data-table': t, open: open.has(t) },
        h('summary', {}, `[${t}]`, changed ? h('span.hint', {}, ` ${changed} changed`) : null),
        h('div.km-list', {}, ...entries.map((e) => h('div.km-row', { title: e.doc },
          h('span.km-label', {}, e.key, h('div.hint', {}, e.doc)),
          control(e),
          current(e.id) !== undefined ? h('button.km-x', { title: 'Back to the theme’s value', onclick: () => set(e.id, undefined) }, '×') : null))));
    }));
  };

  const exportFile = () => {
    const blob = new Blob([text.value || toToml(values)], { type: 'application/toml' });
    const a = h('a', { href: URL.createObjectURL(blob), download: 'appearance.toml' }) as HTMLAnchorElement;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  };
  const importFile = () => {
    const input = h('input', { type: 'file', accept: '.toml,text/plain,application/toml' }) as HTMLInputElement;
    input.addEventListener('change', async () => {
      const file = input.files?.[0];
      if (!file) return;
      const src = await file.text();
      const r = parseAppearance(src);
      text.value = src;
      if ('error' in r) return showError(r.error);
      showError(null);
      save(r.values, true);
      toast(`Imported ${file.name}`);
    });
    input.click();
  };

  text.value = toToml(values);
  paintControls();
  return h('div', {},
    h('p.hint', {}, 'Change any part of the look over the chosen theme, size and density. It is a TOML file in the same format as the built-in themes (design/schema.md); only what you change is kept.'),
    h('label.field', {}, 'Appearance (TOML)', text, error),
    h('div.field.row', {},
      h('button.btn', { onclick: exportFile }, 'Export .toml'),
      h('button.btn', { onclick: importFile }, 'Import .toml…'),
      h('button.btn', { onclick: () => { text.value = ''; showError(null); save([]); } }, 'Reset all')),
    controls,
  );
}

function checkOne(e: Entry, v: unknown): string | null {
  if (v === '') return null;
  const r = parseAppearance(`[${e.table}]\n${e.key} = ${typeof v === 'number' ? v : JSON.stringify(v)}\n`);
  return 'error' in r ? r.error.message.replace(`${e.id}: `, '') : null;
}
