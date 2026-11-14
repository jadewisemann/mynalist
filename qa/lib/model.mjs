// The render model's text (the `[model]` part of the native structural snapshot, and what the web's model page
// writes): parsing, comparing two models, and checking a model against design/spec.json.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { BASE, GROUPS } from '../../apps/web/src/design/tokens.ts';

const root = join(dirname(fileURLToPath(import.meta.url)), '../..');

const STR = /"(?:[^"\\]|\\.)*"/;
const num = (line, key) => {
  const m = new RegExp(`(?:^|\\s)${key}=([-\\d.]+)`).exec(line);
  return m ? Number(m[1]) : undefined;
};

/** A span's style: family (quotes and spacing dropped), size, line, weight, color, the rest. */
export function parseStyle(s) {
  const m = /^(.*) ([\d.]+)\/([\d.]+) w(\d+) (#[0-9a-f]+)(.*)$/.exec(s);
  if (!m) return { raw: s };
  return { family: m[1].replace(/["']/g, '').split(',').map((f) => f.trim()).join(', '), size: +m[2], line: +m[3], weight: +m[4], color: m[5], rest: m[6].trim() };
}
const styleKey = (st) => (st.raw ? st.raw : `${st.family} ${st.size}/${st.line} w${st.weight} ${st.color}${st.rest ? ' ' + st.rest : ''}`);

export function parseModel(text) {
  const model = { topbar: null, title: null, rows: [], toolbar: null };
  let texts = null;
  let curText = null;
  for (const line of text.split('\n')) {
    if (line.startsWith('[views]')) break;
    const t = line.trimStart();
    if (t.startsWith('topbar ')) model.topbar = { top: num(t, 'top'), height: num(t, 'height') };
    else if (t.startsWith('toolbar ')) model.toolbar = { top: num(t, 'top'), height: num(t, 'height') };
    else if (t.startsWith('title ')) {
      model.title = { top: num(t, 'top'), height: num(t, 'height'), texts: [] };
      texts = model.title.texts;
    } else if (t.startsWith('row ')) {
      const row = {
        depth: num(t, 'depth'), top: num(t, 'top'), height: num(t, 'height'), bullet: num(t, 'bullet'),
        number: /number=("(?:[^"\\]|\\.)*")/.exec(t)?.[1], numberRight: num(t, 'number_right'),
        dot: /dot=(\S+ bg=\S+ r=\S+(?: border=\S+ \S+)?)/.exec(t)?.[1], dotColor: /dot=\S+ bg=(\S+)/.exec(t)?.[1],
        checkbox: num(t, 'checkbox'), text: num(t, 'text'), label: /label=(\S+)/.exec(t)?.[1], texts: [], notes: [],
      };
      model.rows.push(row);
      texts = row.texts;
    } else if (t.startsWith('note ')) {
      const note = { top: num(t, 'top'), texts: [] };
      model.rows[model.rows.length - 1].notes.push(note);
      texts = note.texts;
    } else if (t.startsWith('text ')) {
      curText = { x: num(t, 'x'), top: num(t, 'top'), text: STR.exec(t)[0], spans: [] };
      texts.push(curText);
    } else if (t.startsWith('input ')) {
      texts.push({ x: num(t, 'x'), input: STR.exec(t)[0], spans: [] });
    } else if (t.startsWith('"') && curText) {
      const s = STR.exec(t)[0];
      curText.spans.push({ text: s, style: parseStyle(t.slice(s.length + 1)) });
    }
  }
  return model;
}

/** Differences between a native and a web model of the same fixture and look, as lines. x positions are taken
 * from the title's text; a row's top from the row above (layout engines round lengths differently, and a
 * difference must not carry over to every row below), a text's top from its row; text widths (and so the x of
 * anything after a text) are not compared. */
export function compareModels(a, b, tol = { pos: 1.5, size: 1 }) {
  const out = [];
  const o = (m) => ({ x: m.title.texts[0].x, y: m.title.texts[0].top });
  const [oa, ob] = [o(a), o(b)];
  const near = (what, x, y, t, shiftA = 0, shiftB = 0) => {
    if (x === undefined && y === undefined) return;
    if (x === undefined || y === undefined) return out.push(`${what}: native ${x}, web ${y}`);
    if (Math.abs(x - shiftA - (y - shiftB)) > t) out.push(`${what}: native ${x - shiftA}, web ${y - shiftB}`);
  };
  near('top bar height', a.topbar?.height, b.topbar?.height, 0.5);
  const sameTexts = (where, ta, tb, rowA, rowB) => {
    if (ta.length !== tb.length) return out.push(`${where}: ${ta.length} texts native, ${tb.length} web`);
    ta.forEach((x, i) => {
      const y = tb[i];
      if (x.text !== y.text) return out.push(`${where} text ${i}: native ${x.text}, web ${y.text}`);
      if (i === 0) {
        near(`${where} text x`, x.x, y.x, tol.pos, oa.x, ob.x);
        near(`${where} text top`, x.top, y.top, tol.pos, rowA, rowB);
      }
      const sa = x.spans.map((s) => `${s.text} ${styleKey(s.style)}`).join(' | ');
      const sb = y.spans.map((s) => `${s.text} ${styleKey(s.style)}`).join(' | ');
      if (sa !== sb) out.push(`${where} spans: native ${sa}; web ${sb}`);
    });
  };
  sameTexts('title', a.title.texts, b.title.texts, oa.y, ob.y);
  if (a.rows.length !== b.rows.length) out.push(`rows: ${a.rows.length} native, ${b.rows.length} web`);
  a.rows.slice(0, b.rows.length).forEach((x, i) => {
    const y = b.rows[i];
    const w = `row ${i}`;
    if (x.depth !== y.depth) out.push(`${w} depth: native ${x.depth}, web ${y.depth}`);
    const [pa, pb] = i ? [a.rows[i - 1].top, b.rows[i - 1].top] : [oa.y, ob.y];
    near(`${w} top`, x.top, y.top, tol.pos, pa, pb);
    near(`${w} height`, x.height, y.height, tol.size);
    near(`${w} bullet`, x.bullet, y.bullet, tol.pos, oa.x, ob.x);
    near(`${w} checkbox`, x.checkbox, y.checkbox, tol.pos, oa.x, ob.x);
    near(`${w} text`, x.text, y.text, tol.pos, oa.x, ob.x);
    for (const k of ['number', 'dot', 'label']) if (x[k] !== y[k]) out.push(`${w} ${k}: native ${x[k]}, web ${y[k]}`);
    sameTexts(w, x.texts, y.texts, x.top, y.top);
    if (x.notes.length !== y.notes.length) out.push(`${w} notes: native ${x.notes.length}, web ${y.notes.length}`);
    x.notes.slice(0, y.notes.length).forEach((n, k) => {
      near(`${w} note top`, n.top, y.notes[k].top, tol.pos, x.top, y.top);
      sameTexts(`${w} note`, n.texts, y.notes[k].texts, n.top, y.notes[k].top);
    });
  });
  return out;
}

/** Differences that are known and not this check's to fix: each says why. */
export const KNOWN = [
  { re: /row \d+ spans: native "const a = 1;/, why: 'the native app has no syntax highlighting in code blocks yet' },
  { re: /e\^\{i\\\\pi\}\+1=0.*Latin Modern Math|Latin Modern Math.*e\^\{i/, why: 'the native app shows LaTeX source in the mono font; the web typesets it (KaTeX)' },
  { re: /^inline-\w+-rest: row [12] (height|top):/, why: 'the web typesets LaTeX (KaTeX), a taller line than its source' },
  { re: /^inline-phone-rest: row [3-6] (height|top):/, why: 'where a long line wraps depends on glyph widths, which differ between the text engines' },
  { re: /"(highlight|mark)" .* #222222 bg=#ffdd0061 r=2; web .*"(highlight|mark)" .* #000000 bg=#ffdd0061/, why: 'the web\'s <mark> takes the browser\'s black text (a web quirk to decide on)' },
].map((k) => ({ ...k, test: (p) => k.re.test(p) }));

// ---- design/spec.json ----

/** The design entries for modes (`{ theme, size, ... , device }`) and a user's values, resolved. */
export function resolveTokens(modes, user = []) {
  const v = new Map(BASE);
  for (const g of GROUPS) {
    const mode = g.name === 'device' ? modes.device ?? g.default : modes[g.name] ?? g.default;
    for (const [id, x] of g.modes.find((m) => m.name === mode)?.values ?? []) v.set(id, x);
  }
  for (const [id, x] of user) v.set(id, x);
  for (let pass = 0; pass < 4; pass++) {
    for (const [id, x] of v) {
      if (typeof x === 'string' && x.includes('{')) v.set(id, x.replace(/\{([a-z_]+\.[a-z0-9_]+)\}/g, (_, r) => v.get(r)));
    }
  }
  return v;
}

/** `#rgb`, `#rrggbb`, `#rrggbbaa` as the model writes colors. */
export function modelColor(c) {
  let h = c.replace('#', '').toLowerCase();
  if (h.length === 3) h = [...h].map((x) => x + x).join('');
  if (h.length === 8 && h.endsWith('ff')) h = h.slice(0, 6);
  return `#${h}`;
}

/** An expression over entries: numbers, `table.key`, + - * / and parentheses. */
export function evaluate(expr, tokens) {
  const toks = expr.match(/[a-z_]+\.[a-z0-9_]+|\d+(?:\.\d+)?|[-+*/()]/g);
  let i = 0;
  const atom = () => {
    const t = toks[i++];
    if (t === '(') {
      const v = sum();
      i++;
      return v;
    }
    if (t === '-') return -atom();
    if (/^\d/.test(t)) return Number(t);
    const v = tokens.get(t);
    if (typeof v === 'string' && v.endsWith('pt')) return (parseFloat(v) * 4) / 3;
    if (typeof v !== 'number') throw new Error(`${t} is not a number in ${expr}`);
    return v;
  };
  const prod = () => {
    let v = atom();
    while (toks[i] === '*' || toks[i] === '/') v = toks[i++] === '*' ? v * atom() : v / atom();
    return v;
  };
  const sum = () => {
    let v = prod();
    while (toks[i] === '+' || toks[i] === '-') v = toks[i++] === '+' ? v + prod() : v - prod();
    return v;
  };
  return sum();
}

/** The fixture's visible items in row order, with what the spec's row kinds need. */
export function fixtureRows(doc) {
  const items = readFileSync(join(root, `native/crates/outliner-app/tests/snapshots/${doc}.items.jsonl`), 'utf8')
    .split('\n').filter(Boolean).map((l) => JSON.parse(l));
  const byId = new Map(items.map((x) => [x.id, x]));
  const out = [];
  const walk = (id, depth) => {
    const kids = items.filter((x) => x.parent === id);
    for (const k of kids) {
      const p = k.props;
      const c = String(p.c ?? '');
      out.push({
        depth, heading: p.h ?? 0, note: !!(p.n && String(p.n).trim()), checked: !!p.ck, checkbox: !!p.cb, label: p.cl ?? 0,
        numbered: !!byId.get(id)?.props?.num, code: c.includes('```'), latex: c.includes("$$"), long: c.length > 24,
      });
      if (!k.collapsed) walk(k.id, depth + 1);
    }
  };
  walk('root', 0);
  return out;
}

const KINDS = {
  plain: (r) => !r.heading && !r.note && !r.code && !r.checked && !r.checkbox && !r.latex && !r.long,
  dot: (r) => !r.numbered && !r.checkbox,
  heading1: (r) => r.heading === 1,
  heading2: (r) => r.heading === 2,
  heading3: (r) => r.heading === 3,
  note: (r) => r.note,
  checkbox: (r) => r.checkbox,
  checked: (r) => r.checked && !r.heading,
  label: (r) => r.label > 0,
};

/** The broken relations of design/spec.json in a model of `doc` shown in `modes` (device included). */
export function checkSpec(spec, model, modes, doc, user = []) {
  const tokens = resolveTokens(modes, user);
  const items = fixtureRows(doc);
  const out = [];
  const first = (texts) => texts[0]?.spans[0]?.style;
  for (const rel of spec.relations) {
    const expectOf = (item) =>
      rel.expect === 'label' ? modelColor(tokens.get(`label.${item.label}`))
      : rel.expect.startsWith('color.') ? modelColor(tokens.get(rel.expect))
      : evaluate(rel.expect, tokens);
    const check = (where, got, want) => {
      if (got === undefined) return;
      const ok = typeof want === 'number' ? Math.abs(got - want) <= (rel.tolerance ?? 0) : got === want;
      if (!ok) out.push(`${rel.name} (${where}): ${got}, want ${typeof want === 'number' ? +want.toFixed(3) : want}`);
    };
    if (rel.measure === 'topbar.height') { check('top bar', model.topbar?.height, expectOf()); continue; }
    if (rel.measure === 'title.text_left') { check('title', model.title.texts[0].x - model.rows[0].text, expectOf()); continue; }
    if (rel.measure === 'title.to_first_row') {
      const t = model.title.texts[0];
      check('title', model.rows[0].top - (t.top + t.spans[0].style.line), expectOf());
      continue;
    }
    model.rows.forEach((row, i) => {
      const item = items[i];
      if (!item) return;
      if (rel.rows === 'child') {
        const prev = model.rows[i - 1];
        if (!prev || row.depth !== prev.depth + 1) return;
      } else if (rel.rows && !KINDS[rel.rows](item)) return;
      const want = expectOf(item);
      const where = `row ${i}`;
      const text = row.texts[0];
      switch (rel.measure) {
        case 'row.height': return check(where, row.height, want);
        case 'row.text_top': return text && check(where, text.top - row.top, want);
        case 'row.bullet_to_text': return row.dot && check(where, row.text - row.bullet, want);
        case 'row.indent': return check(where, row.text - model.rows[i - 1].text, want);
        case 'note.gap': return row.notes[0] && text && check(where, row.notes[0].top - (text.top + text.spans[0].style.line), want);
        case 'row.checkbox_to_text': return check(where, row.text - row.checkbox, want);
        case 'span.size': return check(where, first(row.texts)?.size, want);
        case 'note.size': return row.notes[0] && check(where, first(row.notes[0].texts)?.size, want);
        case 'span.color': return check(where, first(row.texts)?.color, want);
        case 'row.dot_color': return row.dotColor && check(where, row.dotColor, want);
        case 'row.label': return check(where, row.label, want);
        default: throw new Error(`unknown measure ${rel.measure}`);
      }
    });
  }
  return out;
}
