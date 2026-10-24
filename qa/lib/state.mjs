// Normalized editor state: { zoom, items: [{d,c,n,ck,cb,h,num,col}], focus: {i,f,s,e} | null, selected: [i] },
// plus render (styled runs per item) or marks (search highlights per item: [content, note]) when a scenario asks.
// Rendered as an outline text so both apps can be diffed line by line and read by a human.

function mark(text, f, s, e) {
  if (!f) return text;
  return s === e ? text.slice(0, s) + '|' + text.slice(s) : text.slice(0, s) + '[' + text.slice(s, e) + ']' + text.slice(e);
}

export function outline(st) {
  if (!st) return ['<no state>'];
  const lines = [];
  if (st.zoom != null) lines.push(`ZOOM: ${st.zoom}`);
  const fc = st.focus;
  lines.push(`TITLE ${fc?.i === -1 ? mark(st.title ?? '', true, fc.s, fc.e) : st.title ?? ''}`.replace(/\n/g, '⏎'));
  if (fc && fc.i < -1) lines.push(`FOCUS ? ${JSON.stringify(fc)}`);
  st.items.forEach((it, i) => {
    const ind = '  '.repeat(it.d);
    const sel = st.selected.includes(i) ? '▌' : ' ';
    const bullet = it.col ? '▸' : it.num ? '#' : '•';
    const box = it.cb ? (it.ck ? '[x] ' : '[ ] ') : it.ck ? '(done) ' : '';
    const h = it.h ? `H${it.h} ` : '';
    const c = fc?.i === i && fc.f === 'c' ? mark(it.c, true, fc.s, fc.e) : it.c;
    lines.push(`${sel}${ind}${bullet} ${box}${h}${c}`.replace(/\n/g, '⏎'));
    if (it.n || (fc?.i === i && fc.f === 'n')) {
      const n = fc?.i === i && fc.f === 'n' ? mark(it.n, true, fc.s, fc.e) : it.n;
      lines.push(`${sel}${ind}    ¶ ${n}`.replace(/\n/g, '⏎'));
    }
  });
  if (!fc && !st.selected.length) lines.push('(no focus)');
  if (st.render) {
    lines.push('— rendered —');
    st.render.forEach((runs, i) => lines.push(`${i}: ${runs.map((r) => (r.f ? `<${r.f}${r.x ? ' ' + r.x : ''}>${r.t}</>` : r.t)).join('')}`));
  }
  if (st.marks) {
    lines.push('— highlighted —');
    // marks[0] is the title, then the items
    st.marks.forEach(([c, n], i) => (c || n) && lines.push(`${i ? i - 1 : 'T'}: ${c}${n ? `  ¶ ${n}` : ''}`.replace(/\n/g, '⏎')));
  }
  return lines;
}

export function same(a, b) {
  return JSON.stringify(outline(a)) === JSON.stringify(outline(b));
}

/** What differs, as short tags (for grouping failures). */
/** Treat a caret one character off (same item, same field) as equal: font-metric noise. */
export function withinCaretTol(a, b, tol) {
  if (!a?.focus || !b?.focus || a.focus.i !== b.focus.i || a.focus.f !== b.focus.f) return false;
  const strip = (st) => st.items.map((it) => it.c.replace(/x/g, '')).join('\n') + (st.title ?? '').replace(/x/g, '');
  return Math.abs(a.focus.s - b.focus.s) <= tol && Math.abs(a.focus.e - b.focus.e) <= tol && strip(a) === strip(b);
}

export function diffKinds(a, b) {
  if (!a || !b) return ['missing'];
  const k = new Set();
  if (a.zoom !== b.zoom || a.title !== b.title) k.add('zoom');
  const strip = (st) => st.items.map(({ d, c, n }) => JSON.stringify([d, c, n]));
  if (JSON.stringify(strip(a)) !== JSON.stringify(strip(b))) k.add('tree');
  const props = (st) => st.items.map(({ ck, cb, h, num, col }) => JSON.stringify([ck, cb, h, num, col]));
  if (!k.has('tree') && JSON.stringify(props(a)) !== JSON.stringify(props(b))) k.add('props');
  const f = (st) => JSON.stringify(st.focus);
  if (f(a) !== f(b)) k.add(a.focus && b.focus && a.focus.i === b.focus.i && a.focus.f === b.focus.f ? 'caret' : 'focus');
  if (JSON.stringify(a.selected) !== JSON.stringify(b.selected)) k.add('selection');
  if (a.render && JSON.stringify(a.render) !== JSON.stringify(b.render)) k.add('render');
  if (a.marks && JSON.stringify(a.marks) !== JSON.stringify(b.marks)) k.add('marks');
  return [...k];
}
