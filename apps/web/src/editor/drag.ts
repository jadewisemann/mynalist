// Drag items by their bullet (docs/02 §7). Mouse: press + move > 3 px.
// Touch: long-press 500 ms (cancelled by > 5 px movement). Mod = copy.

import { type NodeId, ROOT, insertPortable, normalizeBlock, toPortable, visibleChildren } from '@outliner/core';
import type { Editor } from './editor.ts';

type Drop = { parent: NodeId; after: NodeId | null; row: HTMLElement; mode: 'before' | 'after' | 'inside' };

/** A drop target outside the editor (the file pane): `hit` highlights it, `drop` takes the items. */
export interface ExternalDrop {
  hit(x: number, y: number): { el: HTMLElement; drop(ids: NodeId[], copy: boolean): void } | null;
}

export function installDrag(editor: Editor, opts: { insideAtTop: () => boolean; external?: ExternalDrop }) {
  const root = editor.el;
  let line: HTMLElement | null = null;
  let label: HTMLElement | null = null;

  root.addEventListener('pointerdown', (e) => {
    const bullet = (e.target as HTMLElement).closest<HTMLElement>('.o-node > .o-row > .o-bullet');
    if (!bullet || e.button !== 0 || editor.readOnly) return;
    const id = bullet.closest<HTMLElement>('.o-node')!.dataset.id!;
    const sx = e.clientX;
    const sy = e.clientY;
    const touch = e.pointerType === 'touch';
    let dragging = false;
    let timer = 0;
    let drop: Drop | null = null;
    let ext: ReturnType<ExternalDrop['hit']> = null;
    let copy = false;
    let ids: NodeId[] = [];
    let scrollTimer = 0;
    let lastY = sy;
    let lastX = sx;

    const begin = () => {
      dragging = true;
      const block = editor.blockIds();
      ids = block?.includes(id) ? normalizeBlock(editor.tree, block) : [id];
      line = document.createElement('div');
      line.className = 'o-drop-line';
      label = document.createElement('div');
      label.className = 'o-drag-label';
      document.body.append(line, label);
      document.body.classList.add('o-dragging');
      if (touch) navigator.vibrate?.(30);
      bullet.setPointerCapture(e.pointerId);
      update(lastX, lastY);
      scrollTimer = window.setInterval(autoscroll, 50);
    };

    const autoscroll = () => {
      const sc = root.closest('.o-scroll') as HTMLElement | null;
      if (!sc) return;
      const r = sc.getBoundingClientRect();
      const edge = 50;
      const d = lastY < r.top + edge ? -1 : lastY > r.bottom - edge ? 1 : 0;
      if (!d) return;
      const depth = d < 0 ? r.top + edge - lastY : lastY - (r.bottom - edge);
      sc.scrollBy({ top: d * Math.min(30, 10 + depth / 2.5) });
      update(lastX, lastY);
    };

    const update = (x: number, y: number) => {
      copy = e.ctrlKey || e.metaKey || copyHeld;
      label!.textContent = `${copy ? 'Copy' : 'Move'} ${ids.length} item${ids.length > 1 ? 's' : ''}`;
      label!.style.transform = `translate(${x + 14}px, ${y + 10}px)`;
      drop = hitTest(x, y);
      const nextExt = drop ? null : opts.external?.hit(x, y) ?? null;
      if (ext?.el !== nextExt?.el) ext?.el.classList.remove('drop-target');
      ext = nextExt;
      ext?.el.classList.add('drop-target');
      if (!drop) {
        line!.hidden = true;
        return;
      }
      line!.hidden = false;
      const content = drop.row.querySelector<HTMLElement>(':scope > .o-row .o-body')!;
      const rr = content.getBoundingClientRect();
      const rowBox = drop.row.querySelector<HTMLElement>(':scope > .o-row')!.getBoundingClientRect();
      const indent = drop.mode === 'inside' ? 30 : 0;
      line!.style.left = `${rr.left + indent}px`;
      line!.style.width = `${Math.max(40, rr.width - indent)}px`;
      line!.style.top = `${drop.mode === 'before' ? rowBox.top - 1 : rowBox.bottom - 1}px`;
    };

    const hitTest = (x: number, y: number): Drop | null => {
      const t = editor.tree;
      const v = editor.view;
      // left of the document column (over the file pane): not an editor drop
      if (x < ((root.closest('.o-scroll') ?? root) as HTMLElement).getBoundingClientRect().left) return null;
      const el = document.elementFromPoint(Math.max(x, root.getBoundingClientRect().left + 40), y)?.closest<HTMLElement>('.o-node');
      if (!el || !root.contains(el)) return null;
      const tid = el.dataset.id!;
      if (ids.some((d) => d === tid || t.isAncestor(d, tid))) return null;
      const n = t.get(tid);
      if (!n?.parent) return null;
      const rowBox = el.querySelector<HTMLElement>(':scope > .o-row')!.getBoundingClientRect();
      const upper = y < rowBox.top + rowBox.height / 2;
      const kids = visibleChildren(t, tid, v);
      const threshold = rowBox.left + 100 + 0.2 * Math.max(0, rowBox.width - 100) + 30;
      if (!upper && kids.length === 0 && x > threshold) {
        const all = n.children;
        const after = opts.insideAtTop() ? null : all[all.length - 1] ?? null;
        return { parent: tid, after, row: el, mode: 'inside' };
      }
      if (upper) {
        const prev = t.prevSibling(tid);
        if (prev && ids.includes(prev) && ids.length === 1) return null;
        return { parent: n.parent, after: prev, row: el, mode: 'before' };
      }
      if (kids.length > 0) return { parent: tid, after: null, row: el, mode: 'after' };
      return { parent: n.parent, after: tid, row: el, mode: 'after' };
    };

    let copyHeld = false;
    const onKey = (ev: KeyboardEvent) => {
      copyHeld = ev.ctrlKey || ev.metaKey;
      if (dragging) update(lastX, lastY);
    };

    const move = (ev: PointerEvent) => {
      lastX = ev.clientX;
      lastY = ev.clientY;
      const dist = Math.hypot(ev.clientX - sx, ev.clientY - sy);
      if (!dragging) {
        if (touch) {
          if (dist > 5) cleanup();
          return;
        }
        if (dist > 3) begin();
        return;
      }
      ev.preventDefault();
      copyHeld = ev.ctrlKey || ev.metaKey;
      update(ev.clientX, ev.clientY);
    };

    const up = () => {
      const was = dragging;
      const target = drop;
      const outside = ext;
      const asCopy = copy;
      cleanup();
      if (!was) return;
      // swallow the click that follows a drag
      // (only if it comes right away: a drag ending elsewhere produces no click)
      const swallow = (ev: Event) => (ev.stopPropagation(), ev.preventDefault());
      window.addEventListener('click', swallow, { capture: true, once: true });
      setTimeout(() => window.removeEventListener('click', swallow, { capture: true }), 0);
      if (outside) {
        outside.drop(ids, asCopy);
        return;
      }
      if (!target) return;
      editor.run((tx) => {
        let after = target.after;
        if (asCopy) {
          const items = ids.map((i) => toPortable(tx.tree, i));
          const nids = insertPortable(tx, target.parent, after, items);
          return { sel: { k: 'block', ids: nids, anchor: nids[0]!, focus: nids[nids.length - 1]! } };
        }
        if (target.parent !== ROOT) tx.collapse(target.parent, false);
        for (const i of ids) {
          if (i === after) continue;
          tx.move(i, target.parent, after);
          after = i;
        }
        return { sel: { k: 'block', ids, anchor: ids[0]!, focus: ids[ids.length - 1]! } };
      });
    };

    const cleanup = () => {
      clearTimeout(timer);
      clearInterval(scrollTimer);
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', cleanup);
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('keyup', onKey);
      line?.remove();
      label?.remove();
      ext?.el.classList.remove('drop-target');
      ext = null;
      line = label = null;
      document.body.classList.remove('o-dragging');
      dragging = false;
    };

    window.addEventListener('pointermove', move, { passive: false });
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', cleanup);
    window.addEventListener('keydown', onKey);
    window.addEventListener('keyup', onKey);
    if (touch) timer = window.setTimeout(begin, 500);
  });
}
