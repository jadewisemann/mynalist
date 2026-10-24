import {
  type NodeId, type PortableItem, type Tree, type Tx, ROOT, exportHtml, exportOpml, exportText, insertPortable,
  opmlToPortable, parsePlainText,
} from '@outliner/core';
import { h, toast } from './dom.ts';
import { modal } from './overlay.ts';

type Format = 'text' | 'html' | 'opml';

export interface ExportOptions {
  /** current view: what "visible items only" leaves out */
  showChecked?: boolean;
  notes?: 'show' | 'teaser' | 'hide';
  /** active search filter */
  include?: (id: NodeId) => boolean;
  /** remembered "Export visible items only" (export_visible_only) */
  visibleOnly?: boolean;
  onVisibleOnly?: (v: boolean) => void;
  /** OPML head */
  source?: string;
  ownerName?: string;
  ownerEmail?: string;
}

/** Export a document (ROOT), an item, or a selection of items. */
export function exportDialog(tree: Tree, sel: NodeId | NodeId[], title: string, opts: ExportOptions = {}) {
  let format: Format = 'text';
  let indent: 'spaces' | 'asterisks' | 'dashes' | 'none' = 'spaces';
  const visibleOnly = h('input', { type: 'checkbox', checked: !!opts.visibleOnly });
  const out = h('textarea', { readOnly: true, style: 'width:100%;height:280px;font:13px var(--mono);resize:vertical' });
  const list = Array.isArray(sel) ? sel : [sel];
  // exporting the document root exports its children (the root is the title)
  const ids = () => (list.length === 1 && list[0] === ROOT ? tree.get(ROOT)!.children : list);
  const update = () => {
    const vis = visibleOnly.checked;
    const scope = {
      visibleOnly: vis,
      showChecked: opts.showChecked ?? true,
      notes: opts.notes === 'teaser' ? 'first' as const : opts.notes ?? 'show',
      include: vis ? opts.include : undefined,
    };
    out.value =
      format === 'text' ? exportText(tree, ids(), { ...scope, indent })
      : format === 'html' ? exportHtml(tree, ids(), { ...scope, document: true, title })
      : exportOpml(tree, ids(), { ...scope, title, source: opts.source, ownerName: opts.ownerName, ownerEmail: opts.ownerEmail });
  };
  const tab = (f: Format, label: string) => {
    const b = h('button.btn', { onclick: () => {
      format = f;
      for (const x of tabs.children) x.classList.remove('primary');
      b.classList.add('primary');
      indentSel.hidden = f !== 'text';
      update();
    } }, label);
    return b;
  };
  const tabs = h('div', { style: 'display:flex;gap:6px;margin-bottom:10px' }, tab('text', 'Plain text'), tab('html', 'Formatted (HTML)'), tab('opml', 'OPML'));
  (tabs.firstElementChild as HTMLElement).classList.add('primary');
  const indentSel = h('select', { onchange: () => { indent = indentSel.value as typeof indent; update(); } },
    h('option', { value: 'spaces' }, 'Indent with spaces'), h('option', { value: 'asterisks' }, 'Asterisks'),
    h('option', { value: 'dashes' }, 'Dashes'), h('option', { value: 'none' }, 'No indentation'));
  visibleOnly.addEventListener('change', () => {
    opts.onVisibleOnly?.(visibleOnly.checked);
    update();
  });
  update();
  const ext: Record<Format, [string, string]> = { text: ['txt', 'text/plain'], html: ['html', 'text/html'], opml: ['opml', 'text/x-opml'] };
  const label = list.length > 1 ? `${list.length} items` : `"${title}"`;
  modal(`Export ${label}`, h('div', {}, tabs,
    h('div', { style: 'display:flex;gap:12px;align-items:center;margin-bottom:8px;font-size:14px' }, indentSel,
      h('label', {}, visibleOnly, ' Export visible items only')),
    out), [
    { label: 'Copy', run: () => (void navigator.clipboard.writeText(out.value).then(() => toast('Copied!')), false) },
    {
      label: 'Download', primary: true, run: () => {
        const [e, mime] = ext[format];
        const a = h('a', { href: URL.createObjectURL(new Blob([out.value], { type: mime })), download: `${title || 'export'}.${e}` });
        a.click();
        setTimeout(() => URL.revokeObjectURL(a.href), 1000);
      },
    },
  ]);
}

/** Pick .opml/.txt files; each becomes a new document built by `create`. */
export function importOpmlFile(create: (title: string, build: (tx: Tx) => void) => void) {
  const input = h('input', { type: 'file', accept: '.opml,.xml,.txt,.md', multiple: true });
  input.addEventListener('change', async () => {
    for (const f of input.files ?? []) {
      const text = await f.text();
      const base = f.name.replace(/\.[^.]+$/, '');
      let items: PortableItem[];
      let title = base;
      if (/\.(opml|xml)$/i.test(f.name)) {
        const doc = opmlToPortable(text);
        if (!doc) {
          toast(`Could not read ${f.name}`);
          continue;
        }
        items = doc.items;
        title = doc.title || base;
      } else {
        items = parsePlainText(text);
      }
      create(title, (tx) => insertPortable(tx, ROOT, null, items));
    }
  });
  input.click();
}
