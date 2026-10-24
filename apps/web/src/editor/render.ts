// Rendering of item text (display mode) and mapping clicks back to source offsets.

import { mapDisplayOffset, renderInline, type RenderOptions } from '@outliner/core';
import { enhance } from './enhance.ts';

let opts: RenderOptions = {};

export function setRenderOptions(o: RenderOptions) {
  opts = o;
}

export function renderText(src: string, extra?: RenderOptions): string {
  if (!src) return '';
  return renderInline(src, extra ? { ...opts, ...extra } : opts);
}

/** Render `src` into `el` (with code highlighting and LaTeX). */
export function renderInto(el: HTMLElement, src: string, extra?: RenderOptions) {
  el.innerHTML = renderText(src, extra);
  if (src.includes('```') || src.includes('$$')) enhance(el);
}

/** Source offset for a click at (x, y) inside a rendered element. */
export function sourceOffsetAt(el: HTMLElement, x: number, y: number, src: string): number {
  let node: Node | null = null;
  let off = 0;
  const d = document as Document & {
    caretPositionFromPoint?: (x: number, y: number) => { offsetNode: Node; offset: number } | null;
  };
  if (d.caretPositionFromPoint) {
    const p = d.caretPositionFromPoint(x, y);
    if (p) ({ offsetNode: node, offset: off } = p);
  } else if (document.caretRangeFromPoint) {
    const r = document.caretRangeFromPoint(x, y);
    if (r) {
      node = r.startContainer;
      off = r.startOffset;
    }
  }
  if (!node || !el.contains(node)) return src.length;
  if (node.nodeType !== Node.TEXT_NODE) {
    // landed between elements: use the boundary of the nearest child
    const child = node.childNodes[off] ?? node.childNodes[off - 1];
    const holder = (child as Element | undefined)?.closest?.('[data-s]') as HTMLElement | null | undefined;
    if (!holder) return src.length;
    return Number(off < node.childNodes.length ? holder.dataset.s : holder.dataset.e);
  }
  const holder = node.parentElement?.closest<HTMLElement>('[data-s]');
  if (!holder || !el.contains(holder)) return src.length;
  let before = 0;
  const walker = document.createTreeWalker(holder, NodeFilter.SHOW_TEXT);
  for (let t = walker.nextNode(); t && t !== node; t = walker.nextNode()) before += t.nodeValue?.length ?? 0;
  const s = Number(holder.dataset.s);
  const e = Number(holder.dataset.e);
  return Math.min(src.length, mapDisplayOffset(s, e, holder.textContent?.length ?? 0, before + off));
}
