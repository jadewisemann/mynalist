// A user's appearance: a TOML document in the look's schema (design/schema.md) holding only what they change, e.g.
//   [color]
//   tag = "#008080"
//   [type]
//   size = 17
// It is layered over the chosen theme and modes (tokens.css) as inline custom properties on <html>, so it applies
// at once. The same rules as the native app's `appearance.rs` and the generator (design/build.mjs): every value is
// checked against its entry, and a document with an error is rejected whole, with the line of the error.

import { parse, TomlError } from 'smol-toml';
import { BASE, SCHEMA, type Entry, type Value } from './tokens.ts';

export const ENTRIES = new Map(SCHEMA.map((e) => [e.id, e]));
export const BASE_VALUES = new Map(BASE);

export interface AppearanceError {
  /** 1-based; 0 when the document as a whole is wrong */
  line: number;
  message: string;
}

export type Appearance = [string, Value][];

const HEX = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i;
const RGBA = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+)\s*)?\)$/i;
const REF = /\{([a-z_]+\.[a-z0-9_]+)\}/g;

/** Checks one value against its entry: an error message, or null. */
export function checkValue(entry: Entry, v: unknown): string | null {
  const refs = typeof v === 'string' ? [...v.matchAll(REF)].map((m) => m[1]) : [];
  for (const r of refs) if (!ENTRIES.has(r)) return `unknown entry {${r}}`;
  const range = (n: number) => {
    if (entry.min !== undefined && n < entry.min) return `less than ${entry.min}`;
    if (entry.max !== undefined && n > entry.max) return `more than ${entry.max}`;
    return null;
  };
  switch (entry.kind) {
    case 'color':
      if (typeof v !== 'string') return 'a color is a string';
      if (refs.length) return v === `{${refs[0]}}` && ENTRIES.get(refs[0])!.kind === 'color' ? null : 'a color reference is one color entry';
      if (v === 'transparent' || HEX.test(v)) return null;
      {
        const m = RGBA.exec(v);
        if (m && [m[1], m[2], m[3]].every((c) => +c <= 255) && (m[4] === undefined || +m[4] <= 1)) return null;
      }
      return `not a color: ${v}`;
    case 'length':
      if (typeof v === 'string' && /^\d+(\.\d+)?pt$/.test(v)) return null;
      if (typeof v !== 'number') return 'a length is a number (pixels)';
      return range(v);
    case 'ratio':
      if (typeof v !== 'number') return 'a ratio is a number';
      return range(v);
    case 'weight':
      return Number.isInteger(v) && (v as number) >= 100 && (v as number) <= 900 ? null : 'a weight is a whole number from 100 to 900';
    case 'font':
      return typeof v === 'string' && v.trim() ? null : 'a font is a CSS font-family list';
    case 'choice':
      return entry.choices!.includes(v as string) ? null : `one of ${entry.choices!.join(', ')}`;
  }
}

/** The 1-based line of `key` in `[table]` (or of the table's header, or 0). */
function lineOf(text: string, table: string, key?: string): number {
  let inTable = false;
  let header = 0;
  const lines = text.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const t = lines[i].trim();
    const h = /^\[([^\]]*)\]/.exec(t);
    if (h) {
      inTable = h[1].trim() === table;
      if (inTable) header = i + 1;
      continue;
    }
    if (inTable && key !== undefined && t.split('=')[0].trim().replace(/^"|"$/g, '') === key) return i + 1;
  }
  return header;
}

/** Parses and checks a user's appearance TOML. */
export function parseAppearance(text: string): { values: Appearance } | { error: AppearanceError } {
  let doc: Record<string, unknown>;
  try {
    doc = parse(text) as Record<string, unknown>;
  } catch (e) {
    if (e instanceof TomlError) return { error: { line: e.line, message: e.message.split('\n')[0] } };
    return { error: { line: 0, message: String(e) } };
  }
  const values: Appearance = [];
  for (const [table, entries] of Object.entries(doc)) {
    if (!entries || typeof entries !== 'object' || Array.isArray(entries)) {
      return { error: { line: lineOf(text, table), message: `${table} is not a table` } };
    }
    for (const [key, v] of Object.entries(entries)) {
      const id = `${table}.${key}`;
      const line = lineOf(text, table, key);
      const entry = ENTRIES.get(id);
      if (!entry) return { error: { line, message: `unknown entry ${id}` } };
      const err = checkValue(entry, v);
      if (err) return { error: { line, message: `${id}: ${err}` } };
      values.push([id, v as Value]);
    }
  }
  return { values };
}

/** A value as CSS (`{table.key}` as that entry's custom property). */
export function cssValue(entry: Entry, v: Value): string {
  if (typeof v === 'string') return v.replace(REF, (_, id: string) => `var(${ENTRIES.get(id)!.css})`);
  if (entry.kind === 'length') return `${v}px`;
  return String(v);
}

/** A TOML string or number. */
function tomlValue(v: Value): string {
  return typeof v === 'number' ? String(v) : JSON.stringify(v);
}

/** The TOML text of an appearance, tables in the schema's order. */
export function toToml(values: Appearance): string {
  const byTable = new Map<string, [string, Value][]>();
  const order = new Map(SCHEMA.map((e, i) => [e.id, i]));
  for (const [id, v] of [...values].sort((a, b) => (order.get(a[0]) ?? 0) - (order.get(b[0]) ?? 0))) {
    const e = ENTRIES.get(id)!;
    if (!byTable.has(e.table)) byTable.set(e.table, []);
    byTable.get(e.table)!.push([e.key, v]);
  }
  return [...byTable].map(([t, kv]) => `[${t}]\n${kv.map(([k, v]) => `${k} = ${tomlValue(v)}`).join('\n')}\n`).join('\n');
}

/** The properties a user's appearance sets on <html> (layered over tokens.css). */
export function appearanceStyle(values: Appearance): [string, string][] {
  return values.map(([id, v]) => {
    const e = ENTRIES.get(id)!;
    return [e.css, cssValue(e, v)];
  });
}

const applied = new WeakMap<HTMLElement, string[]>();

/** Applies a user's appearance to `root`, replacing what an earlier one set. */
export function applyAppearance(root: HTMLElement, values: Appearance) {
  for (const p of applied.get(root) ?? []) root.style.removeProperty(p);
  const style = appearanceStyle(values);
  for (const [p, v] of style) root.style.setProperty(p, v);
  applied.set(root, style.map(([p]) => p));
}
