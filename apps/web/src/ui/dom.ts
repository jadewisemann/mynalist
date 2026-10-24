// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Handler = (e: any) => void;
type Attrs = Record<string, string | number | boolean | Handler | undefined | null>;

/** Tiny element builder: h('div.cls', {onclick}, child, 'text') */
export function h<K extends keyof HTMLElementTagNameMap>(
  sel: K | `${K}.${string}`,
  attrs: Attrs = {},
  ...children: (Node | string | null | undefined | false)[]
): HTMLElementTagNameMap[K] {
  const [tag, ...cls] = sel.split('.');
  const el = document.createElement(tag as K);
  if (cls.length) el.className = cls.join(' ');
  for (const [k, v] of Object.entries(attrs)) {
    if (v === undefined || v === null || v === false) continue;
    if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2), v as Handler);
    else if (k === 'class') el.className += ' ' + String(v);
    else if (k in el && typeof v !== 'string') (el as unknown as Record<string, unknown>)[k] = v;
    else el.setAttribute(k, v === true ? '' : String(v));
  }
  for (const c of children) if (c !== null && c !== undefined && c !== false) el.append(c);
  return el;
}

const PATHS: Record<string, string> = {
  menu: 'M3 6h18M3 12h18M3 18h18',
  side: 'M3 4h18v16H3zM9 4v16',
  search: 'M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM21 21l-5-5',
  gear: 'M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM19.4 15a1.6 1.6 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.6 1.6 0 0 0-1.8-.3 1.6 1.6 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.6 1.6 0 0 0-1-1.5 1.6 1.6 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.6 1.6 0 0 0 .3-1.8 1.6 1.6 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.6 1.6 0 0 0 1.5-1 1.6 1.6 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.6 1.6 0 0 0 1.8.3H9a1.6 1.6 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.6 1.6 0 0 0 1 1.5 1.6 1.6 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.6 1.6 0 0 0-.3 1.8V9a1.6 1.6 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.6 1.6 0 0 0-1.5 1z',
  back: 'M15 18l-6-6 6-6',
  fwd: 'M9 18l6-6-6-6',
  doc: 'M6 2h9l5 5v15H6zM14 2v6h6',
  folder: 'M3 6h6l2 2h10v12H3z',
  plus: 'M12 5v14M5 12h14',
  more: 'M5 12h.01M12 12h.01M19 12h.01',
  star: 'M12 3l2.8 5.7 6.2.9-4.5 4.4 1 6.2L12 17.3 6.5 20.2l1-6.2L3 9.6l6.2-.9z',
  close: 'M6 6l12 12M18 6L6 18',
  filter: 'M3 5h18l-7 8v6l-4 2v-8z',
  inbox: 'M3 13l3-8h12l3 8v6H3zM3 13h5l1 3h6l1-3h5',
  fold: 'M7 4l5 5 5-5M7 20l5-5 5 5',
  zoomin: 'M12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0-6 0M12 3v3M12 18v3M3 12h3M18 12h3',
  eye: 'M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12zM12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z',
};

export function icon(name: keyof typeof PATHS | string): SVGSVGElement {
  const NS = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '1.8');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  const p = document.createElementNS(NS, 'path');
  p.setAttribute('d', PATHS[name] ?? '');
  svg.append(p);
  return svg;
}

/** Show a short message; returns a function that removes it early. */
export function toast(msg: string, ms = 2200): () => void {
  const t = h('div.toast', {}, msg);
  document.body.append(t);
  setTimeout(() => t.remove(), ms);
  return () => t.remove();
}
