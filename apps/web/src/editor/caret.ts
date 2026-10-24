// Caret helpers for a plaintext contenteditable element.

/** Raw text of an editable element (text nodes + <br> as newline). */
export function getText(el: HTMLElement): string {
  let out = '';
  let trailingBr = false;
  const walk = (n: Node) => {
    for (let c = n.firstChild; c; c = c.nextSibling) {
      if (c.nodeType === Node.TEXT_NODE) {
        out += c.nodeValue ?? '';
        if (c.nodeValue) trailingBr = false;
      } else if (c.nodeName === 'BR') {
        out += '\n';
        trailingBr = true;
      } else {
        if (c.nodeName === 'DIV' && out && !out.endsWith('\n')) out += '\n';
        walk(c);
      }
    }
  };
  walk(el);
  // a final <br> is the browser's placeholder for an emptied line (e.g. after Ctrl+Backspace), not text
  return trailingBr ? out.slice(0, -1) : out;
}

/** Replace contents with a single text node (keeps a trailing newline visible). */
export function setText(el: HTMLElement, text: string) {
  el.textContent = text;
}

function offsetOf(el: HTMLElement, node: Node, off: number): number {
  if (node === el) {
    let n = 0;
    for (let i = 0; i < off && i < el.childNodes.length; i++) n += nodeLen(el.childNodes[i]!);
    return n;
  }
  let n = 0;
  let found = false;
  const walk = (p: Node) => {
    for (let c = p.firstChild; c && !found; c = c.nextSibling) {
      if (c === node) {
        if (c.nodeType === Node.TEXT_NODE) n += off;
        else for (let i = 0; i < off && i < c.childNodes.length; i++) n += nodeLen(c.childNodes[i]!);
        found = true;
        return;
      }
      if (c.contains(node)) {
        walk(c);
        return;
      }
      n += nodeLen(c);
    }
  };
  walk(el);
  return n;
}

function nodeLen(n: Node): number {
  if (n.nodeType === Node.TEXT_NODE) return n.nodeValue?.length ?? 0;
  if (n.nodeName === 'BR') return 1;
  let s = 0;
  for (let c = n.firstChild; c; c = c.nextSibling) s += nodeLen(c);
  return s;
}

/** Current selection offsets inside `el`, or null if the selection is elsewhere. */
export function getSel(el: HTMLElement): { s: number; e: number; backward: boolean } | null {
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0 || !sel.anchorNode || !el.contains(sel.anchorNode)) return null;
  const a = offsetOf(el, sel.anchorNode, sel.anchorOffset);
  const f = sel.focusNode && el.contains(sel.focusNode) ? offsetOf(el, sel.focusNode, sel.focusOffset) : a;
  return { s: Math.min(a, f), e: Math.max(a, f), backward: f < a };
}

function locate(el: HTMLElement, off: number): { node: Node; off: number } {
  let rem = off;
  let last: { node: Node; off: number } = { node: el, off: 0 };
  const walk = (p: Node): { node: Node; off: number } | null => {
    for (let c = p.firstChild; c; c = c.nextSibling) {
      if (c.nodeType === Node.TEXT_NODE) {
        const len = c.nodeValue?.length ?? 0;
        if (rem <= len) return { node: c, off: rem };
        rem -= len;
        last = { node: c, off: len };
      } else if (c.nodeName === 'BR') {
        if (rem === 0) return { node: p, off: Array.prototype.indexOf.call(p.childNodes, c) };
        rem -= 1;
        last = { node: p, off: Array.prototype.indexOf.call(p.childNodes, c) + 1 };
      } else {
        const r = walk(c);
        if (r) return r;
      }
    }
    return null;
  };
  return walk(el) ?? last;
}

export function setSel(el: HTMLElement, s: number, e = s) {
  const sel = window.getSelection();
  if (!sel) return;
  const a = locate(el, s);
  const b = s === e ? a : locate(el, e);
  const r = document.createRange();
  r.setStart(a.node, a.off);
  r.setEnd(b.node, b.off);
  sel.removeAllRanges();
  sel.addRange(r);
}

/** Bounding rect of the caret (collapsed range); falls back to the element. */
export function caretRect(el: HTMLElement): DOMRect {
  const sel = window.getSelection();
  if (sel && sel.rangeCount && sel.focusNode && el.contains(sel.focusNode)) {
    const r = document.createRange();
    r.setStart(sel.focusNode, sel.focusOffset);
    r.collapse(true);
    const rects = r.getClientRects();
    if (rects.length) return rects[rects.length - 1]!;
    // empty line / start of element: measure with a temporary marker
    const span = document.createElement('span');
    span.textContent = '​';
    r.insertNode(span);
    const rect = span.getBoundingClientRect();
    const parent = span.parentNode;
    span.remove();
    parent?.normalize();
    return rect;
  }
  return el.getBoundingClientRect();
}

function lineHeight(el: HTMLElement): number {
  const lh = parseFloat(getComputedStyle(el).lineHeight);
  return Number.isFinite(lh) ? lh : 20;
}

/** Is the caret on the first visual line of `el`? */
export function onFirstLine(el: HTMLElement): boolean {
  const cr = caretRect(el);
  const er = el.getBoundingClientRect();
  const pad = parseFloat(getComputedStyle(el).paddingTop) || 0;
  return cr.top - (er.top + pad) < lineHeight(el) * 0.7;
}

export function onLastLine(el: HTMLElement): boolean {
  const cr = caretRect(el);
  const er = el.getBoundingClientRect();
  const pad = parseFloat(getComputedStyle(el).paddingBottom) || 0;
  return er.bottom - pad - cr.bottom < lineHeight(el) * 0.7;
}

/** Text offset in `el` closest to viewport point (x, y). */
export function offsetAtPoint(el: HTMLElement, x: number, y: number): number | null {
  let node: Node | null = null;
  let off = 0;
  const d = document as Document & {
    caretPositionFromPoint?: (x: number, y: number) => { offsetNode: Node; offset: number } | null;
  };
  if (d.caretPositionFromPoint) {
    const p = d.caretPositionFromPoint(x, y);
    if (p) {
      node = p.offsetNode;
      off = p.offset;
    }
  } else if (document.caretRangeFromPoint) {
    const r = document.caretRangeFromPoint(x, y);
    if (r) {
      node = r.startContainer;
      off = r.startOffset;
    }
  }
  if (!node || !el.contains(node)) return null;
  return offsetOf(el, node, off);
}

/** Place caret in `el` at horizontal position x on its first or last line. */
export function placeAtX(el: HTMLElement, x: number, line: 'first' | 'last'): number {
  const r = el.getBoundingClientRect();
  const lh = lineHeight(el);
  const y = line === 'first' ? r.top + Math.min(lh / 2, r.height / 2) : r.bottom - Math.min(lh / 2, r.height / 2);
  const cx = Math.max(r.left + 1, Math.min(x, r.right - 1));
  const off = offsetAtPoint(el, cx, y);
  const len = getText(el).length;
  const at = off ?? (line === 'first' ? 0 : len);
  setSel(el, at);
  return at;
}
