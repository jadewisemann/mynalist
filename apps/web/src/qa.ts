// Read-only snapshot of what the editor shows, for the differential QA suite (qa/).
// Shape matches qa/lib/state.mjs: visible items in document order + focus/selection.

import { ROOT, content, hasCheckbox, heading, isChecked, note, numbered, visibleChildren, type NodeId } from '@outliner/core';
import type { Editor } from './editor/editor.ts';

export interface QaItem {
  id: string;
  d: number;
  c: string;
  n: string;
  ck: boolean;
  cb: boolean;
  h: number;
  num: boolean;
  col: boolean;
}

export interface QaState {
  zoom: string | null;
  title: string;
  items: QaItem[];
  focus: { i: number; f: 'c' | 'n'; s: number; e: number } | null;
  selected: number[];
}

export function qaSnapshot(ed: Editor): QaState {
  const t = ed.tree;
  const v = ed.view;
  const items: QaItem[] = [];
  const index = new Map<NodeId, number>();
  const walk = (id: NodeId, d: number) => {
    const parentNum = numbered(t.get(id));
    for (const c of visibleChildren(t, id, v)) {
      const n = t.get(c)!;
      index.set(c, items.length);
      items.push({
        id: c,
        d,
        c: content(n),
        n: note(n),
        ck: isChecked(n),
        cb: hasCheckbox(n),
        h: heading(n),
        num: parentNum,
        col: n.children.length > 0 && (v.filter ? visibleChildren(t, c, v).length === 0 : n.collapsed),
      });
      walk(c, d + 1);
    }
  };
  walk(v.zoom, 0);
  const sel = ed.currentSel();
  let focus: QaState['focus'] = null;
  let selected: number[] = [];
  if (sel.k === 'text') focus = { i: sel.id === v.zoom ? -1 : (index.get(sel.id) ?? -2), f: sel.f, s: sel.s, e: sel.e };
  else if (sel.k === 'block') selected = sel.ids.map((id) => index.get(id) ?? -2).sort((a, b) => a - b);
  return { zoom: v.zoom === ROOT ? null : content(t.get(v.zoom)), title: content(t.get(v.zoom)), items, focus, selected };
}
