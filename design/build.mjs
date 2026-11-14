// Writes the look's generated files from design/*.toml (see design/schema.toml):
//   apps/web/src/design/tokens.css   custom properties per mode, for styles.css
//   apps/web/src/design/tokens.ts    schema, base and modes, for the web's appearance settings
//   native/crates/outliner-app/src/design.rs   the same, for the native app's style.rs
//   design/schema.md                 the schema, documented
// `node design/build.mjs` writes them; `node design/build.mjs --check` fails if any is stale (CI).
import { readFileSync, writeFileSync, readdirSync, existsSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
// smol-toml is the web app's dependency (it parses the user's appearance TOML there too)
const { parse } = createRequire(join(root, 'apps/web/package.json'))('smol-toml');

const read = (p) => parse(readFileSync(join(here, p), 'utf8'));
const schemaToml = read('schema.toml');
const base = read('base.toml');
const groupsToml = read('modes.toml');

const fail = (msg) => {
  console.error(`design: ${msg}`);
  process.exit(1);
};

// ---- schema ----
const KINDS = ['color', 'length', 'ratio', 'weight', 'font', 'choice'];
const schema = [];
for (const [table, entries] of Object.entries(schemaToml)) {
  for (const [key, e] of Object.entries(entries)) {
    if (!KINDS.includes(e.kind)) fail(`${table}.${key}: unknown kind ${e.kind}`);
    if (!e.doc) fail(`${table}.${key}: no doc`);
    schema.push({
      table, key, id: `${table}.${key}`, kind: e.kind, doc: e.doc,
      css: e.css ?? `--${table}-${key}`.replaceAll('_', '-'),
      ...(e.min !== undefined ? { min: e.min } : {}),
      ...(e.max !== undefined ? { max: e.max } : {}),
      ...(e.choices ? { choices: e.choices } : {}),
    });
  }
}
const byId = new Map(schema.map((e) => [e.id, e]));

// ---- values ----
const HEX = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i;
const RGBA = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+)\s*)?\)$/i;
const REF = /\{([a-z_]+\.[a-z0-9_]+)\}/g;

/** Checks one value against its entry; returns an error message or null. Same rules as the web's appearance.ts. */
function check(entry, v) {
  const refs = typeof v === 'string' ? [...v.matchAll(REF)].map((m) => m[1]) : [];
  for (const r of refs) if (!byId.has(r)) return `unknown entry {${r}}`;
  switch (entry.kind) {
    case 'color':
      if (typeof v !== 'string') return 'a color is a string';
      if (refs.length) return v === `{${refs[0]}}` && byId.get(refs[0]).kind === 'color' ? null : 'a color reference is one color entry';
      return v === 'transparent' || HEX.test(v) || RGBA.test(v) ? null : `not a color: ${v}`;
    case 'length':
      if (typeof v === 'string' && /^\d+(\.\d+)?pt$/.test(v)) return null;
      if (typeof v !== 'number') return 'a length is a number (pixels)';
      if (entry.min !== undefined && v < entry.min) return `less than ${entry.min}`;
      if (entry.max !== undefined && v > entry.max) return `more than ${entry.max}`;
      return null;
    case 'ratio':
      if (typeof v !== 'number') return 'a ratio is a number';
      if (entry.min !== undefined && v < entry.min) return `less than ${entry.min}`;
      if (entry.max !== undefined && v > entry.max) return `more than ${entry.max}`;
      return null;
    case 'weight':
      return Number.isInteger(v) && v >= 100 && v <= 900 ? null : 'a weight is a whole number from 100 to 900';
    case 'font':
      return typeof v === 'string' && v.trim() ? null : 'a font is a CSS font-family list';
    case 'choice':
      return entry.choices.includes(v) ? null : `one of ${entry.choices.join(', ')}`;
  }
}

/** A TOML document of values as [[id, value]], checked against the schema. */
function values(doc, where, all = false) {
  const out = [];
  for (const [table, entries] of Object.entries(doc)) {
    if (typeof entries !== 'object' || Array.isArray(entries)) fail(`${where}: ${table} is not a table`);
    for (const [key, v] of Object.entries(entries)) {
      const entry = byId.get(`${table}.${key}`);
      if (!entry) fail(`${where}: unknown entry ${table}.${key}`);
      const err = check(entry, v);
      if (err) fail(`${where}: ${table}.${key}: ${err}`);
      out.push([entry.id, v]);
    }
  }
  if (all) {
    const have = new Set(out.map(([id]) => id));
    for (const e of schema) if (!have.has(e.id)) fail(`${where}: no value for ${e.id}`);
  }
  return out;
}

const baseValues = values(base, 'base.toml', true);
const groups = [];
for (const [name, g] of Object.entries(groupsToml)) {
  const dir = join(here, 'modes', name);
  const files = existsSync(dir) ? readdirSync(dir).filter((f) => f.endsWith('.toml')).sort() : [];
  const modes = files.map((f) => ({ name: f.slice(0, -5), values: values(read(`modes/${name}/${f}`), `modes/${name}/${f}`) }));
  for (const m of modes) {
    if (m.name === g.default) fail(`modes/${name}: the default mode ${m.name} has a file (it is the base)`);
    if (!g.names?.[m.name]) fail(`modes/${name}/${m.name}.toml: no name in modes.toml`);
    if (!g.attr && !g.media?.[m.name]) fail(`modes/${name}/${m.name}.toml: no attr or media in modes.toml`);
  }
  for (const n of Object.keys(g.names ?? {})) {
    if (n !== g.default && !modes.some((m) => m.name === n)) fail(`modes.toml: ${name}.${n} has no file`);
  }
  groups.push({ name, default: g.default, attr: g.attr ?? null, media: g.media ?? null, names: g.names, modes });
}
// two groups (but `media`, which is applied over everything as print stylesheets are) never set the same entry:
// the web's selectors of different groups have equal weight, so which wins would depend on their order
const setBy = new Map();
for (const g of groups) {
  if (g.name === 'media') continue;
  for (const m of g.modes) {
    for (const [id] of m.values) {
      const other = setBy.get(id);
      if (other && other !== g.name) fail(`${id} is set by modes of both ${other} and ${g.name}`);
      setBy.set(id, g.name);
    }
  }
}

// ---- CSS ----
const cssValue = (entry, v) => {
  if (typeof v === 'string') return v.replace(REF, (_, id) => `var(${byId.get(id).css})`);
  if (entry.kind === 'length') return `${v}px`;
  return String(v);
};
const decls = (vals, indent) => vals.map(([id, v]) => `${indent}${byId.get(id).css}: ${cssValue(byId.get(id), v)};`).join('\n');
const HEAD = (c) => `${c} Generated by design/build.mjs from design/*.toml: do not edit. ${c === '//' ? '' : '*/'}`.trimEnd();
let css = `${HEAD('/*')}\n\n:root {\n${decls(baseValues, '  ')}\n}\n`;
for (const g of groups) {
  for (const m of g.modes) {
    if (g.attr) css += `\n:root[${g.attr}='${m.name}'] {\n${decls(m.values, '  ')}\n}\n`;
    else css += `\n@media ${g.media[m.name]} {\n  :root {\n${decls(m.values, '    ')}\n  }\n}\n`;
  }
}

// ---- TS ----
const ts = `${HEAD('//')}
// The look's schema, base values and modes (design/*.toml); appearance.ts layers a user's TOML over them.

export type Kind = 'color' | 'length' | 'ratio' | 'weight' | 'font' | 'choice';
export interface Entry {
  table: string;
  key: string;
  /** \`table.key\` */
  id: string;
  kind: Kind;
  doc: string;
  /** the web's custom property (or CSS property) */
  css: string;
  min?: number;
  max?: number;
  choices?: string[];
}
export type Value = string | number;
export interface Mode { name: string; values: [string, Value][] }
export interface Group {
  name: string;
  default: string;
  attr: string | null;
  media: Record<string, string> | null;
  names: Record<string, string>;
  modes: Mode[];
}

export const SCHEMA: Entry[] = ${JSON.stringify(schema, null, 2)};

export const BASE: [string, Value][] = ${JSON.stringify(baseValues)};

export const GROUPS: Group[] = ${JSON.stringify(groups, null, 2)};
`;

// ---- Rust ----
function rgba(v) {
  if (v === 'transparent') return 0;
  let m = HEX.exec(v);
  if (m) {
    let h = m[1];
    if (h.length === 3) h = [...h].map((c) => c + c).join('');
    if (h.length === 6) h += 'ff';
    return parseInt(h, 16) >>> 0;
  }
  m = RGBA.exec(v);
  const a = m[4] === undefined ? 255 : Math.round(Number(m[4]) * 255);
  return ((Number(m[1]) << 24) | (Number(m[2]) << 16) | (Number(m[3]) << 8) | a) >>> 0;
}
const rsStr = (s) => JSON.stringify(s);
function rsValue(entry, v) {
  if (typeof v === 'string' && v.includes('{')) return `Value::Text(${rsStr(v)})`;
  switch (entry.kind) {
    case 'color': return `Value::Color(0x${rgba(v).toString(16).padStart(8, '0')})`;
    case 'length': return `Value::Number(${typeof v === 'string' ? `${(parseFloat(v) * 4) / 3}` : v}_f32)`.replace('_f32', '');
    case 'ratio': return `Value::Number(${v})`;
    case 'weight': return `Value::Number(${v})`;
    default: return `Value::Text(${rsStr(v)})`;
  }
}
const num = (s) => (s.includes('.') || s.includes('e') ? s : `${s}.0`);
const rsVals = (vals) =>
  vals.map(([id, v]) => `    (${rsStr(id)}, ${rsValue(byId.get(id), v).replace(/Value::Number\(([^)]+)\)/, (_, n) => `Value::Number(${num(n)})`)}),`).join('\n');
const opt = (x) => (x === undefined ? 'None' : `Some(${num(String(x))})`);
const rs = `${HEAD('//')}
//! The look's schema, base values and modes (\`design/*.toml\`, documented in \`design/schema.md\`). \`style.rs\` reads
//! the resolved values (\`crate::appearance\`); nothing here is written by hand.

/// What an entry holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Color,
    Length,
    Ratio,
    Weight,
    Font,
    Choice,
}

/// One entry of the schema.
#[derive(Debug)]
pub struct Entry {
    /// \`table.key\`
    pub id: &'static str,
    pub kind: Kind,
    pub doc: &'static str,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub choices: &'static [&'static str],
}

/// A value as the TOML files hold it: a color as 0xRRGGBBAA, a length in pixels, a ratio or weight, or text (a
/// font, a choice, or a value holding \`{table.key}\` references).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Color(u32),
    Number(f32),
    Text(&'static str),
}

/// A mode group: its default mode (the base) and the others with what they change.
#[derive(Debug)]
pub struct Group {
    pub name: &'static str,
    pub default: &'static str,
    pub modes: &'static [(&'static str, &'static [(&'static str, Value)])],
}

pub const SCHEMA: &[Entry] = &[
${schema.map((e) => `    Entry { id: ${rsStr(e.id)}, kind: Kind::${e.kind[0].toUpperCase() + e.kind.slice(1)}, doc: ${rsStr(e.doc)}, min: ${opt(e.min)}, max: ${opt(e.max)}, choices: &[${(e.choices ?? []).map(rsStr).join(', ')}] },`).join('\n')}
];

pub const BASE: &[(&str, Value)] = &[
${rsVals(baseValues)}
];

pub const GROUPS: &[Group] = &[
${groups.map((g) => `    Group {
        name: ${rsStr(g.name)},
        default: ${rsStr(g.default)},
        modes: &[
${g.modes.map((m) => `            (${rsStr(m.name)}, &[\n${rsVals(m.values).replace(/^ {4}/gm, '                ')}\n            ]),`).join('\n')}
        ],
    },`).join('\n')}
];
`;

// ---- schema.md ----
const md = `<!-- Generated by design/build.mjs from design/schema.toml and design/modes.toml: do not edit. -->
# The look's schema

One TOML format holds the whole look of both apps: \`design/base.toml\` (every entry), one file per mode in
\`design/modes/<group>/<mode>.toml\`, and a user's appearance (only what they change). All use the tables and keys
below. A user's appearance is layered over the chosen modes, for example:

\`\`\`toml
[color]
tag = "#008080"

[type]
size = 17
\`\`\`

Kinds: \`color\` ("#rrggbb", "#rgb", "#rrggbbaa", "rgba(r, g, b, a)", "transparent"), \`length\` (pixels), \`ratio\`
(a plain number), \`weight\` (100..900), \`font\` (a CSS font-family list), \`choice\`. A color or font may be
\`"{table.key}"\` (another entry's value; in a font also inside a list).

## Modes

| group | default | modes | web |
| --- | --- | --- | --- |
${groups.map((g) => `| ${g.name} | ${g.default} | ${g.modes.map((m) => m.name).join(', ')} | ${g.attr ? `\`${g.attr}\` on \`<html>\`` : Object.values(g.media).map((q) => `\`@media ${q}\``).join(', ')} |`).join('\n')}

${Object.keys(schemaToml).map((table) => `## [${table}]

| key | kind | range | web | base | what |
| --- | --- | --- | --- | --- | --- |
${schema.filter((e) => e.table === table).map((e) => {
  const v = baseValues.find(([id]) => id === e.id)[1];
  const range = e.choices ? e.choices.join(' / ') : e.min !== undefined ? `${e.min}..${e.max}` : '';
  return `| ${e.key} | ${e.kind} | ${range} | \`${e.css}\` | \`${String(v).replaceAll('|', '\\|')}\` | ${e.doc} |`;
}).join('\n')}
`).join('\n')}`;

// ---- write / check ----
const outputs = [
  ['apps/web/src/design/tokens.css', css],
  ['apps/web/src/design/tokens.ts', ts],
  ['native/crates/outliner-app/src/design.rs', rs],
  ['design/schema.md', md],
];
const checking = process.argv.includes('--check');
let stale = [];
for (const [path, text] of outputs) {
  const full = join(root, path);
  const now = existsSync(full) ? readFileSync(full, 'utf8') : null;
  if (now === text) continue;
  if (checking) stale.push(path);
  else {
    mkdirSync(dirname(full), { recursive: true });
    writeFileSync(full, text);
    console.log(`wrote ${path}`);
  }
}
if (stale.length) fail(`stale: ${stale.join(', ')}; run node design/build.mjs and commit them`);
