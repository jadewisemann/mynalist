import type { FileEntry, Workspace } from '../store/workspace.ts';
import { h, icon } from './dom.ts';
import { modal, showMenu } from './overlay.ts';

export type PaneName = 'files' | 'bookmarks' | 'tags';

export interface SidebarHost {
  currentDoc(): string | null;
  openDoc(id: string, zoom?: string, q?: string): void;
  exportDoc(id: string): void;
  importOpml(parent: string | null): void;
  tags(): { doc: [string, number][]; all: [string, number][] };
  searchTag(tag: string, global: boolean): void;
  bookmarkCurrent(): void;
}

export class Sidebar {
  readonly el: HTMLElement;
  private pane: HTMLElement;
  private tabs: Record<PaneName, HTMLButtonElement>;
  active: PaneName = 'files';
  private renaming: string | null = null;
  /** the live rename input, reused across re-renders so typing isn't lost */
  private renameEl: { id: string; el: HTMLInputElement } | null = null;
  private dragId: string | null = null;
  private ws: Workspace;
  private host: SidebarHost;

  constructor(ws: Workspace, host: SidebarHost) {
    this.ws = ws;
    this.host = host;
    const tab = (name: PaneName, label: string) => h('button.tab', { onclick: () => this.show(name) }, label);
    this.tabs = { files: tab('files', 'Files'), bookmarks: tab('bookmarks', 'Bookmarks'), tags: tab('tags', 'Tags') };
    this.pane = h('div.pane');
    this.pane.addEventListener('contextmenu', (e) => {
      if (this.active !== 'files' || (e.target as HTMLElement).closest('.file-row')) return;
      e.preventDefault();
      showMenu(e.clientX, e.clientY, [
        { label: 'New document', run: () => this.newFile('doc', null) },
        { label: 'New folder', run: () => this.newFile('folder', null) },
        { label: 'Import OPML…', run: () => this.host.importOpml(null) },
      ]);
    });
    this.el = h('aside.sidebar', {}, h('div.tabs', {}, this.tabs.files, this.tabs.bookmarks, this.tabs.tags), this.pane);
    ws.onChange(() => this.render());
    this.show('files');
  }

  show(name: PaneName) {
    this.active = name;
    for (const [k, b] of Object.entries(this.tabs)) b.classList.toggle('active', k === name);
    this.render();
  }

  render() {
    const p = this.pane;
    p.textContent = '';
    if (this.active === 'files') this.renderFiles();
    else if (this.active === 'bookmarks') this.renderBookmarks();
    else this.renderTags();
  }

  private head(title: string, ...buttons: HTMLElement[]) {
    this.pane.append(h('div.pane-head', {}, h('span', {}, title), h('span.spacer'), ...buttons));
  }

  newFile(kind: 'doc' | 'folder', parent: string | null) {
    const id = this.ws.create(kind, parent);
    this.renaming = id;
    if (kind === 'doc') this.host.openDoc(id);
    this.render();
  }

  private renderFiles() {
    const addBtn = h('button.icon-btn', { title: 'New…', onclick: (e: MouseEvent) => {
      showMenu(e.clientX, e.clientY + 10, [
        { label: 'New document', run: () => this.newFile('doc', null) },
        { label: 'New folder', run: () => this.newFile('folder', null) },
        { label: 'Import OPML…', run: () => this.host.importOpml(null) },
      ]);
    } }, icon('plus'));
    this.head('My files', addBtn);
    const files = this.ws.files();
    if (!files.length) {
      this.pane.append(h('div.empty-state', { style: 'margin-top:20px;font-size:14px' },
        h('a', { onclick: () => this.newFile('doc', null) }, 'Create a document')));
    }
    const walk = (es: FileEntry[], depth: number, parent: string | null) => {
      es.forEach((e, i) => {
        this.pane.append(this.fileRow(e, depth, parent, i));
        if (e.kind === 'folder' && !e.collapsed) walk(e.children, depth + 1, e.id);
      });
    };
    walk(files, 0, null);
  }

  private fileRow(e: FileEntry, depth: number, parent: string | null, index: number): HTMLElement {
    const cur = this.host.currentDoc();
    const isFolder = e.kind === 'folder';
    const name = this.renaming === e.id ? this.renameInput(e) : h('span.name', {}, e.title);
    const row = h(
      'div.file-row',
      {
        style: `padding-left:${8 + depth * 16}px`,
        draggable: this.renaming !== e.id,
        onclick: () => {
          if (isFolder) this.ws.setCollapsed(e.id, !e.collapsed);
          else this.host.openDoc(e.id);
        },
        ondblclick: () => {
          this.renaming = e.id;
          this.render();
        },
        oncontextmenu: (ev: MouseEvent) => {
          ev.preventDefault();
          this.fileMenu(e, ev.clientX, ev.clientY);
        },
      },
      h('span.tog', {}, isFolder ? (e.collapsed ? '▸' : '▾') : ''),
      h('span.ico', {}, icon(isFolder ? 'folder' : 'doc')),
      name,
      h('span.more', { onclick: (ev: MouseEvent) => (ev.stopPropagation(), this.fileMenu(e, ev.clientX, ev.clientY)) }, '⋯'),
    );
    if (e.id === cur) row.classList.add('active');
    if (e.archived) row.classList.add('archived');
    // drag & drop reordering
    row.addEventListener('dragstart', (ev) => {
      this.dragId = e.id;
      ev.dataTransfer!.effectAllowed = 'move';
      ev.dataTransfer!.setData('text/plain', e.title);
    });
    const zone = (ev: DragEvent): 'before' | 'after' | 'into' => {
      const r = row.getBoundingClientRect();
      const y = ev.clientY - r.top;
      if (isFolder && y > r.height * 0.25 && y < r.height * 0.75) return 'into';
      return y < r.height / 2 ? 'before' : 'after';
    };
    row.addEventListener('dragover', (ev) => {
      if (!this.dragId || this.dragId === e.id) return;
      ev.preventDefault();
      row.classList.remove('drop-before', 'drop-after', 'drop-into');
      row.classList.add(`drop-${zone(ev)}`);
    });
    row.addEventListener('dragleave', () => row.classList.remove('drop-before', 'drop-after', 'drop-into'));
    row.addEventListener('drop', (ev) => {
      ev.preventDefault();
      const id = this.dragId;
      this.dragId = null;
      if (!id || id === e.id) return;
      const z = zone(ev);
      if (z === 'into') this.ws.move(id, e.id, 1e9);
      else this.ws.move(id, parent, z === 'before' ? index : index + 1);
    });
    return row;
  }

  private renameInput(e: FileEntry): HTMLElement {
    if (this.renameEl?.id === e.id) {
      const el = this.renameEl.el;
      queueMicrotask(() => el.isConnected && document.activeElement !== el && el.focus());
      return el;
    }
    const input = h('input', { value: e.title });
    this.renameEl = { id: e.id, el: input };
    let done = false;
    const commit = (save: boolean) => {
      if (done) return;
      done = true;
      this.renaming = null;
      this.renameEl = null;
      if (save && input.value.trim() && input.value !== e.title) this.ws.rename(e.id, input.value.trim());
      else this.render();
    };
    input.addEventListener('keydown', (ev) => {
      if (ev.isComposing) return;
      if (ev.key === 'Enter') commit(true);
      if (ev.key === 'Escape') commit(false);
      ev.stopPropagation();
    });
    input.addEventListener('blur', () => setTimeout(() => {
      // blur caused by a re-render moving the element: keep editing
      if (!input.isConnected || document.activeElement !== input) commit(true);
    }));
    input.addEventListener('click', (ev) => ev.stopPropagation());
    queueMicrotask(() => {
      input.focus();
      input.select();
    });
    return input;
  }

  private fileMenu(e: FileEntry, x: number, y: number) {
    const items = [];
    if (e.kind === 'folder') {
      items.push(
        { label: 'New document', run: () => this.newFile('doc', e.id) },
        { label: 'New folder', run: () => this.newFile('folder', e.id) },
        { label: 'Import OPML…', run: () => this.host.importOpml(e.id) },
        { label: '', sep: true },
      );
    } else {
      items.push(
        { label: 'Export…', run: () => this.host.exportDoc(e.id) },
        { label: 'Add to bookmarks', run: () => this.ws.addBookmark({ title: e.title, doc: e.id }) },
        { label: e.archived ? 'Unarchive' : 'Archive', run: () => this.ws.setArchived(e.id, !e.archived) },
        { label: '', sep: true },
      );
    }
    items.push(
      { label: 'Rename', run: () => ((this.renaming = e.id), this.render()) },
      {
        label: 'Delete',
        run: () => {
          modal('Delete', h('div', {}, `Are you sure you want to delete the ${e.kind === 'folder' ? 'folder' : 'document'} "${e.title}"?`), [
            { label: 'Cancel' },
            { label: 'Delete', primary: true, run: () => this.ws.remove(e.id) },
          ]);
        },
      },
    );
    showMenu(x, y, items);
  }

  private renderBookmarks() {
    this.head(`Bookmarks`, h('button.icon-btn', { title: 'Bookmark current view', onclick: () => this.host.bookmarkCurrent() }, icon('star')));
    for (const b of this.ws.bookmarks()) {
      const row = h('div.file-row', {
        style: 'padding-left:12px',
        onclick: () => this.host.openDoc(b.doc, b.zoom, b.q),
        oncontextmenu: (ev: MouseEvent) => {
          ev.preventDefault();
          showMenu(ev.clientX, ev.clientY, [
            {
              label: 'Rename',
              run: () => {
                const input = h('input', { value: b.title });
                modal('Rename bookmark', h('div.field', {}, input), [
                  { label: 'Cancel' },
                  { label: 'Save', primary: true, run: () => this.ws.renameBookmark(b.id, input.value) },
                ]);
              },
            },
            { label: 'Remove', run: () => this.ws.removeBookmark(b.id) },
          ]);
        },
      }, h('span.ico', {}, icon('star')), h('span.name', {}, b.title));
      this.pane.append(row);
    }
  }

  private renderTags() {
    const { doc, all } = this.host.tags();
    const section = (title: string, list: [string, number][], global: boolean) => {
      this.head(`${title} (${list.length})`);
      for (const [tag, n] of list) {
        this.pane.append(h('div.tag-row', { onclick: () => this.host.searchTag(tag, global) }, h('span', {}, tag), h('span.count', {}, `(${n})`)));
      }
    };
    section('Document tags', doc, false);
    section('All tags', all, true);
  }
}
