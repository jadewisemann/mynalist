// Mind map (docs/03 §1.4): a read-only full-screen view of the zoomed subtree. Boxes keep color labels,
// headings, checked styling and notes; each one can be expanded/collapsed here without touching the doc.

import { ROOT, type NodeId, type Tree } from '@outliner/core';
import { renderText } from '../editor/render.ts';
import { h, icon } from './dom.ts';

export interface MindMapOpts {
  tree: Tree;
  zoom: NodeId;
  title: string;
  showChecked: boolean;
  onBack(): void;
  onZoom(id: NodeId): void;
}

export class MindMap {
  readonly el = h('div.mindmap', { hidden: true });
  private scale = 1;
  private open = new Map<NodeId, boolean>();
  private opts: MindMapOpts | null = null;

  constructor() {
    this.el.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') this.opts?.onBack();
    });
  }

  show(o: MindMapOpts) {
    if (this.opts?.tree !== o.tree) this.open.clear();
    this.opts = o;
    this.el.hidden = false;
    this.render();
    this.el.focus();
  }

  hide() {
    this.el.hidden = true;
    this.opts = null;
  }

  get active() {
    return !this.el.hidden;
  }

  render() {
    const o = this.opts;
    if (!o) return;
    const t = o.tree;
    const isOpen = (id: NodeId) => this.open.get(id) ?? (id === o.zoom || !t.get(id)!.collapsed);
    const kidsOf = (id: NodeId) => t.get(id)!.children.filter((c) => o.showChecked || !t.get(c)!.props.ck);
    const node = (id: NodeId): HTMLElement => {
      const n = t.get(id)!;
      const kids = kidsOf(id);
      const box = h('div.mm-box', { class: [n.props.ck ? 'checked' : '', n.props.h ? `h${n.props.h}` : '', n.props.cl ? `o-cl${n.props.cl}` : ''].filter(Boolean).join(' ') || undefined });
      const text = h('div.mm-text');
      text.innerHTML = id === ROOT && !n.props.c ? o.title : renderText(String(n.props.c ?? '')) || '&nbsp;';
      box.append(text);
      if (typeof n.props.n === 'string' && n.props.n) {
        const note = h('div.mm-note');
        note.innerHTML = renderText(n.props.n);
        box.append(note);
      }
      if (id !== o.zoom) box.addEventListener('dblclick', () => o.onZoom(id));
      const el = h('div.mm-node', {}, box);
      if (kids.length) {
        const opened = isOpen(id);
        box.append(h('button.mm-toggle', { title: opened ? 'Collapse' : 'Expand', onclick: () => {
          this.open.set(id, !opened);
          this.render();
        } }, opened ? '−' : String(kids.length)));
        if (opened) el.append(h('div.mm-kids', {}, ...kids.map(node)));
      }
      return el;
    };
    const crumbs = h('div.mm-crumbs', {}, ...t.ancestors(o.zoom).map((a) =>
      h('a', { onclick: () => o.onZoom(a) }, a === ROOT ? o.title : String(t.get(a)?.props.c ?? '').slice(0, 30))));
    const canvas = h('div.mm-canvas', { style: `transform: scale(${this.scale})` }, node(o.zoom));
    const zoomBy = (d: number) => {
      this.scale = Math.min(2, Math.max(0.3, Math.round((this.scale + d) * 10) / 10));
      canvas.style.transform = `scale(${this.scale})`;
    };
    this.el.tabIndex = -1;
    this.el.replaceChildren(
      h('div.mm-bar', {},
        h('button.btn', { onclick: () => o.onBack() }, '← Back to document'), crumbs, h('span.spacer'),
        h('button.icon-btn', { title: 'Zoom out', onclick: () => zoomBy(-0.1) }, '−'),
        h('button.icon-btn', { title: 'Zoom in', onclick: () => zoomBy(0.1) }, icon('plus'))),
      h('div.mm-scroll', {}, canvas));
  }
}
