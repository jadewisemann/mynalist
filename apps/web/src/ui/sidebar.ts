import type { NodeId } from '@outliner/core';
import type { ExternalDrop } from '../editor/drag.ts';
import type { Bookmark, FileEntry, Workspace } from '../store/workspace.ts';
import { h, icon } from './dom.ts';
import { modal, showMenu, type MenuItem } from './overlay.ts';

export type PaneName = 'files' | 'bookmarks' | 'tags';

/** dataTransfer type of a document dragged from the file pane (dropped onto an item: becomes that item) */
export const DOC_DRAG_TYPE = 'application/x-outliner-doc';

export interface SidebarHost {
  currentDoc(): string | null;
  openDoc(id: string): void;
  openBookmark(b: Bookmark): void;
  exportDoc(id: string): void;
  importOpml(parent: string | null): void;
  tags(): { doc: [string, number][]; all: [string, number][] };
  searchTag(tag: string, global: boolean): void;
  /** rename a tag in every document; `to` null deletes it */
  renameTag(tag: string, to: string | null): void;
  bookmarkCurrent(): void;
  findFile(): void;
  gotoInbox(): void;
  copyDoc(id: string): void;
  versionHistory(id: string): void;
  share(id: string): void;
  cleanNewlines(id: string): void;
  /** items dragged from the editor onto a document (or onto empty pane space: doc null = new documents) */
  dropItems(doc: string | null, ids: NodeId[], copy: boolean): void;
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
  private dragBookmark: string | null = null;
  private tagCollapsed = new Set<string>();
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
      showMenu(e.clientX, e.clientY, this.newMenu(null));
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

  /** Drop target for items dragged out of the editor. */
  externalDrop(): ExternalDrop {
    return {
      hit: (x, y) => {
        if (this.el.hidden || this.active !== 'files') return null;
        const el = document.elementFromPoint(x, y) as HTMLElement | null;
        if (!el || !this.el.contains(el)) return null;
        const row = el.closest<HTMLElement>('.file-row[data-kind="doc"]');
        if (row) return { el: row, drop: (ids, copy) => this.host.dropItems(row.dataset.id!, ids, copy) };
        return { el: this.pane, drop: (ids, copy) => this.host.dropItems(null, ids, copy) };
      },
    };
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

  private newMenu(parent: string | null): MenuItem[] {
    return [
      { label: 'New document', run: () => this.newFile('doc', parent) },
      { label: 'New folder', run: () => this.newFile('folder', parent) },
      { label: 'Import OPML…', run: () => this.host.importOpml(parent) },
    ];
  }

  private renderFiles() {
    const btn = (name: string, title: string, run: (e: MouseEvent) => void) => h('button.icon-btn', { title, onclick: run }, icon(name));
    this.head('My files',
      btn('search', 'Find a document (Ctrl+O)', () => this.host.findFile()),
      btn('inbox', 'Go to inbox', () => this.host.gotoInbox()),
      btn('fold', 'Collapse all folders', () => this.collapseAll()),
      btn('plus', 'New…', (e) => showMenu(e.clientX, e.clientY + 10, this.newMenu(null))));
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

  private collapseAll() {
    const walk = (es: FileEntry[]) => {
      for (const e of es) {
        if (e.kind !== 'folder') continue;
        if (!e.collapsed) this.ws.setCollapsed(e.id, true);
        walk(e.children);
      }
    };
    walk(this.ws.files());
  }

  private fileRow(e: FileEntry, depth: number, parent: string | null, index: number): HTMLElement {
    const cur = this.host.currentDoc();
    const isFolder = e.kind === 'folder';
    const name = this.renaming === e.id ? this.renameInput(e) : h('span.name', {}, e.title);
    const row = h(
      'div.file-row',
      {
        style: `padding-left:${8 + depth * 16}px`,
        'data-id': e.id,
        'data-kind': e.kind,
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
    // drag & drop reordering (and documents onto editor items)
    row.addEventListener('dragstart', (ev) => {
      this.dragId = e.id;
      ev.dataTransfer!.effectAllowed = 'move';
      ev.dataTransfer!.setData('text/plain', e.title);
      if (!isFolder) ev.dataTransfer!.setData(DOC_DRAG_TYPE, e.id);
    });
    row.addEventListener('dragend', () => (this.dragId = null));
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
    const items: MenuItem[] = [];
    if (e.kind === 'folder') {
      items.push(...this.newMenu(e.id), { label: '', sep: true });
    } else {
      items.push(
        { label: 'Make a copy', run: () => this.host.copyDoc(e.id) },
        { label: 'Version history', run: () => this.host.versionHistory(e.id) },
        { label: 'Share…', run: () => this.host.share(e.id) },
        { label: 'Export…', run: () => this.host.exportDoc(e.id) },
        { label: 'Add to bookmarks', run: () => this.ws.addBookmark({ title: e.title, doc: e.id }) },
      );
      if (this.ws.pref('show_cleanup_newlines_option', false)) items.push({ label: 'Clean up newlines', run: () => this.host.cleanNewlines(e.id) });
      items.push(
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

  private renameBookmark(b: Bookmark) {
    const input = h('input', { value: b.title });
    modal('Rename bookmark', h('div.field', {}, input), [
      { label: 'Cancel' },
      { label: 'Save', primary: true, run: () => this.ws.renameBookmark(b.id, input.value.trim() || b.title) },
    ]);
    input.select();
  }

  private removeBookmark(b: Bookmark) {
    modal('Remove bookmark', h('div', {}, `Remove the bookmark "${b.title}"?`), [
      { label: 'Cancel' },
      { label: 'Remove', primary: true, run: () => this.ws.removeBookmark(b.id) },
    ]);
  }

  private renderBookmarks() {
    this.head('Bookmarks', h('button.icon-btn', { title: 'Bookmark current view', onclick: () => this.host.bookmarkCurrent() }, icon('star')));
    const list = this.ws.bookmarks();
    if (!list.length) this.pane.append(h('div.pane-empty', {}, 'Bookmark a document, a zoomed item or a search with the ☆ button.'));
    list.forEach((b, i) => {
      const kind = b.global ? 'search' : b.q ? 'search' : b.zoom ? 'zoom' : 'doc';
      const row = h('div.file-row', {
        style: 'padding-left:12px',
        draggable: true,
        title: `${this.ws.title(b.doc)}${b.q ? ` — "${b.q}"` : ''}${b.flat ? ' (flat)' : ''}`,
        onclick: () => this.host.openBookmark(b),
        ondblclick: () => this.renameBookmark(b),
        oncontextmenu: (ev: MouseEvent) => {
          ev.preventDefault();
          showMenu(ev.clientX, ev.clientY, [
            { label: 'Rename', run: () => this.renameBookmark(b) },
            { label: 'Remove', run: () => this.removeBookmark(b) },
          ]);
        },
      }, h('span.ico', {}, icon(kind === 'search' ? 'search' : kind === 'zoom' ? 'zoomin' : 'star')), h('span.name', {}, b.title),
      h('span.more', { onclick: (ev: MouseEvent) => (ev.stopPropagation(), this.removeBookmark(b)), title: 'Remove' }, '×'));
      row.addEventListener('dragstart', (ev) => {
        this.dragBookmark = b.id;
        ev.dataTransfer!.effectAllowed = 'move';
        ev.dataTransfer!.setData('text/plain', b.title);
      });
      row.addEventListener('dragover', (ev) => {
        if (!this.dragBookmark || this.dragBookmark === b.id) return;
        ev.preventDefault();
        const before = ev.clientY < row.getBoundingClientRect().top + row.offsetHeight / 2;
        row.classList.toggle('drop-before', before);
        row.classList.toggle('drop-after', !before);
      });
      row.addEventListener('dragleave', () => row.classList.remove('drop-before', 'drop-after'));
      row.addEventListener('drop', (ev) => {
        ev.preventDefault();
        const id = this.dragBookmark;
        this.dragBookmark = null;
        if (!id || id === b.id) return;
        const before = ev.clientY < row.getBoundingClientRect().top + row.offsetHeight / 2;
        const from = list.findIndex((x) => x.id === id);
        let to = before ? i : i + 1;
        if (from < to) to--;
        this.ws.moveBookmark(id, to);
      });
      this.pane.append(row);
    });
  }

  private renderTags() {
    const { doc, all } = this.host.tags();
    const alpha = this.ws.pref<string>('tag_sort', 'count') === 'alpha';
    const sorted = (list: [string, number][]) =>
      alpha ? [...list].sort((a, b) => a[0].localeCompare(b[0], undefined, { sensitivity: 'base' })) : list;
    const section = (key: string, title: string, list: [string, number][], global: boolean) => {
      const collapsed = this.tagCollapsed.has(key);
      this.pane.append(h('div.pane-head.clickable', { onclick: () => {
        if (collapsed) this.tagCollapsed.delete(key);
        else this.tagCollapsed.add(key);
        this.render();
      } }, h('span', {}, `${collapsed ? '▸' : '▾'} ${title} (${list.length})`), h('span.spacer'),
      key === 'doc' ? h('button.btn.link', { title: 'Sort order', onclick: (ev: MouseEvent) => {
        ev.stopPropagation();
        this.ws.setPref('tag_sort', alpha ? 'count' : 'alpha');
      } }, alpha ? 'A–Z' : 'By count') : null));
      if (collapsed) return;
      for (const [tag, n] of sorted(list)) {
        this.pane.append(h('div.tag-row', {
          onclick: () => this.host.searchTag(tag, global),
          oncontextmenu: (ev: MouseEvent) => {
            ev.preventDefault();
            showMenu(ev.clientX, ev.clientY, [
              { label: 'Search in this document', run: () => this.host.searchTag(tag, false) },
              { label: 'Search everywhere', run: () => this.host.searchTag(tag, true) },
              { label: '', sep: true },
              { label: 'Rename tag…', run: () => this.renameTag(tag) },
              { label: 'Delete tag…', run: () => modal('Delete tag', h('div', {}, `Remove ${tag} from every item in all documents?`), [
                { label: 'Cancel' },
                { label: 'Delete', primary: true, run: () => this.host.renameTag(tag, null) },
              ]) },
            ]);
          },
        }, h('span', {}, tag), h('span.count', {}, `(${n})`)));
      }
    };
    section('doc', 'Document tags', doc, false);
    section('all', 'All tags', all, true);
  }

  private renameTag(tag: string) {
    const input = h('input', { value: tag });
    modal('Rename tag', h('div', {}, h('label.field', {}, `Rename ${tag} in all documents to`, input)), [
      { label: 'Cancel' },
      {
        label: 'Rename',
        primary: true,
        run: () => {
          let to = input.value.trim();
          if (!to || to === tag) return;
          if (!/^[#@]/.test(to)) to = tag[0] + to;
          this.host.renameTag(tag, to);
        },
      },
    ]);
    input.select();
  }
}
