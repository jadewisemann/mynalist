// Desktop hover toolbar for items (docs/03 §4 node toolbar), behind pref item_toolbar_hover (off by default).
// One fixed-position bar for the whole editor, floated above the right end of the hovered row, so showing
// it never shifts the layout. Buttons swallow mousedown: the item being edited keeps its caret.

import { type NodeId, visibleChildren } from '@outliner/core';
import type { Editor } from '../editor/editor.ts';
import type { Workspace } from '../store/workspace.ts';
import { h } from './dom.ts';
import { swatches } from './itemmenu.ts';
import { showMenu } from './overlay.ts';

type Btn = 'unindent' | 'indent' | 'zoom-in' | 'delete' | 'check' | 'note' | 'date' | 'upload' | 'checkbox' | 'color' | 'heading' | 'duplicate' | 'export' | 'newline';

const BUTTONS: [Btn, string, string][] = [
  ['unindent', '⇤', 'Unindent'], ['indent', '⇥', 'Indent'], ['zoom-in', '⊕', 'Zoom in'], ['delete', '🗑', 'Delete'],
  ['check', '✓', 'Check off'], ['note', '✎', 'Edit note'], ['date', '◷', 'Add date'], ['upload', '📎', 'Upload a file'],
  ['checkbox', '☐', 'Toggle checkbox'], ['color', '🎨', 'Set color label'], ['heading', 'H', 'Set heading level'],
  ['duplicate', '⧉', 'Duplicate'], ['export', '⇪', 'Export item'], ['newline', '↵', 'Insert new line'],
];

/** delay before hiding, so the pointer can travel from the row onto the bar */
const HIDE_MS = 250;

export class HoverBar {
  readonly el = h('div.hover-bar', { hidden: true });
  private editor: Editor;
  private ws: Workspace;
  private btns = new Map<Btn, HTMLButtonElement>();
  /** item the bar is shown for */
  private id: NodeId | null = null;
  private timer = 0;
  private fine = matchMedia('(pointer: fine)');
  private narrow = matchMedia('(max-width: 760px)');

  constructor(editor: Editor, ws: Workspace) {
    this.editor = editor;
    this.ws = ws;
    for (const [b, glyph, title] of BUTTONS) {
      const el = h('button', {
        title, 'aria-label': title, 'data-btn': b,
        onmousedown: (e: Event) => e.preventDefault(),
        onclick: (e: MouseEvent) => this.press(b, e.currentTarget as HTMLElement),
      }, glyph);
      this.btns.set(b, el);
      this.el.append(el);
    }
    this.el.addEventListener('mouseenter', () => clearTimeout(this.timer));
    this.el.addEventListener('mouseleave', () => this.hideSoon());
    document.body.append(this.el);

    editor.el.addEventListener('mousemove', (e) => this.onMove(e));
    editor.el.addEventListener('mouseleave', () => this.hideSoon());
    // rows move under a fixed bar: drop it on scroll, resize and typing; the next mouse move brings it back
    document.addEventListener('scroll', () => this.hide(), true);
    window.addEventListener('resize', () => this.hide());
    editor.el.addEventListener('keydown', () => this.hide());
  }

  private enabled(): boolean {
    return this.ws.pref('item_toolbar_hover', false) && this.fine.matches && !this.narrow.matches && !this.editor.readOnly
      && !document.body.classList.contains('o-dragging');
  }

  private onMove(e: MouseEvent) {
    if (!this.enabled()) return this.hide();
    const node = (e.target as HTMLElement).closest<HTMLElement>('.o-items .o-node > .o-row')?.parentElement;
    const id = node?.dataset.id;
    if (!id) return this.hideSoon();
    clearTimeout(this.timer);
    if (id !== this.id || this.el.hidden) this.show(id);
  }

  /** the items a button acts on: the block selection when it covers the hovered item */
  private ids(id: NodeId): NodeId[] {
    return this.editor.blockIds()?.includes(id) ? this.editor.targets() : [id];
  }

  private rowOf(id: NodeId): HTMLElement | null {
    return this.editor.el.querySelector<HTMLElement>(`.o-items .o-node[data-id="${CSS.escape(id)}"] > .o-row`);
  }

  show(id: NodeId) {
    const row = this.rowOf(id);
    const n = this.editor.tree.get(id);
    if (!row || !n) return this.hide();
    this.id = id;
    const ids = this.ids(id);
    const v = this.editor.view;
    // same visibility rules as the item menu, shown as disabled buttons so the bar keeps its shape
    this.btns.get('indent')!.disabled = visibleChildren(this.editor.tree, n.parent!, v)[0] === ids[0];
    this.btns.get('unindent')!.disabled = n.parent === v.zoom;
    const done = ids.every((i) => this.editor.tree.get(i)?.props.ck);
    this.btns.get('check')!.title = done ? 'Uncheck' : 'Check off';
    this.btns.get('check')!.classList.toggle('on', !!done);
    this.el.hidden = false;
    this.place(row);
  }

  private place(row: HTMLElement) {
    const r = row.getBoundingClientRect();
    const w = this.el.offsetWidth;
    const hgt = this.el.offsetHeight;
    // above the row; below it when the row is at the top edge of the viewport
    const top = r.top - hgt >= 4 ? r.top - hgt : r.bottom;
    this.el.style.left = `${Math.max(4, Math.min(r.right - w, innerWidth - w - 4))}px`;
    this.el.style.top = `${top}px`;
  }

  hide() {
    clearTimeout(this.timer);
    this.el.hidden = true;
    this.id = null;
  }

  private hideSoon() {
    clearTimeout(this.timer);
    this.timer = window.setTimeout(() => this.hide(), HIDE_MS);
  }

  /** Put the caret at the end of `id` unless it is already being edited (commands that work on the caret). */
  private editHere(id: NodeId) {
    if (this.editor.textContext()?.id === id) return;
    const len = String(this.editor.tree.get(id)?.props.c ?? '').length;
    this.editor.applySel({ k: 'text', id, f: 'c', s: len, e: len });
  }

  private press(b: Btn, btn: HTMLElement) {
    const id = this.id;
    if (!id || this.editor.readOnly || !this.editor.tree.get(id)) return;
    const ids = this.ids(id);
    const ed = this.editor;
    switch (b) {
      case 'unindent':
      case 'indent':
      case 'duplicate':
      case 'export':
        ed.execOn(ids, b);
        break;
      case 'zoom-in':
        this.hide();
        ed.setZoom(id);
        return;
      case 'delete':
        this.hide();
        ed.execOn(ids, 'delete-left');
        return;
      case 'check':
        ed.execOn(ids, 'toggle-checked');
        break;
      case 'checkbox':
        ed.execOn(ids, 'toggle-checkbox');
        break;
      case 'note':
        ed.execOn([id], 'goto-note-of');
        break;
      case 'date':
        this.editHere(id);
        ed.exec('add-date');
        break;
      case 'newline':
        this.editHere(id);
        ed.exec('newline');
        break;
      case 'upload': {
        const input = h('input', { type: 'file', multiple: true });
        input.onchange = () => ed.attachFiles(id, [...(input.files ?? [])]);
        input.click();
        break;
      }
      case 'color':
      case 'heading': {
        const r = btn.getBoundingClientRect();
        showMenu(r.left, r.bottom + 4, [{ label: b, custom: swatches(ed, id, ids, b === 'color' ? 'cl' : 'h') }]);
        break;
      }
    }
    // the row may have moved (indent, newline, a checked item hidden…): follow it, or go away
    requestAnimationFrame(() => {
      if (this.id !== id) return;
      if (this.rowOf(id)) this.show(id);
      else this.hide();
    });
  }
}
