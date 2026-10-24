import { Capacitor } from '@capacitor/core';
import {
  ROOT, escapeInline, Tx, backlinkCounts, compileQuery, extractTags, findBacklinks, fuzzyFilter, inlineToPlain, insertPortable, parseItemUrl,
  replaceTag, searchTree, toPortable, type NodeId, type Sel, type SortMode, type Tree,
} from '@outliner/core';
import { Editor } from './editor/editor.ts';
import { comboOf, Keymap } from './editor/keymap.ts';
import { renderText, setRenderOptions } from './editor/render.ts';
import { exportDialog, importOpmlFile } from './ui/exporter.ts';
import { SyncClient, type SyncState } from './store/sync.ts';
import { Workspace, type Bookmark } from './store/workspace.ts';
import { h, icon, toast } from './ui/dom.ts';
import { finder, modal, showMenu, type MenuItem } from './ui/overlay.ts';
import { DOC_DRAG_TYPE, Sidebar, type PaneName } from './ui/sidebar.ts';
import { installDrag } from './editor/drag.ts';
import { itemMenu } from './ui/itemmenu.ts';
import { Popups } from './ui/popups.ts';
import { ResultsView, breadcrumb } from './ui/results.ts';
import { versionHistory } from './ui/history.ts';
import { serverExtras, shareDialog } from './ui/share.ts';
import { captureDialog, inboxRoute, inboxTarget } from './ui/inbox.ts';
import { searchReplaceDialog } from './ui/replace.ts';
import { backupZip, downloadBlob } from './ui/backup.ts';
import { settingsDialog } from './ui/settings.ts';
import { HelpPanel } from './ui/help.ts';
import { MobileBar } from './ui/mobilebar.ts';
import { MindMap } from './ui/mindmap.ts';
import { tutorial, whatsNew } from './ui/tour.ts';
import '@fontsource/fira-sans/400.css';
import '@fontsource/fira-sans/400-italic.css';
import '@fontsource/fira-sans/700.css';
import '@fontsource/fira-sans/700-italic.css';
import '@fontsource/fira-mono/400.css';
import './platform.ts';
import { qaSnapshot } from './qa.ts';

interface Route {
  doc: string | null;
  zoom: NodeId;
  q: string;
  /** search results screen instead of the document */
  view?: 'flat' | 'global';
  sort?: SortMode | '';
}

const LAST_DOC = 'outliner:lastDoc';
const WELCOME = 'outliner:welcome';
const ZOOMS = 'outliner:zooms';
const APP_COMMANDS_ANYWHERE = [
  'help', 'settings', 'toggle-file-pane', 'toggle-bookmark-pane', 'toggle-tag-pane', 'save', 'search', 'go-anywhere', 'goto-item',
  'back', 'forward', 'undo', 'redo', 'bookmark', 'inbox-capture', 'goto-inbox', 'version-history', 'download-backup',
  'toggle-show-checked', 'toggle-show-notes', 'toggle-doc-checked', 'toggle-doc-notes', 'toggle-spellcheck', 'search-replace',
];

class App {
  sync = new SyncClient();
  ws!: Workspace;
  editor!: Editor;
  sidebar!: Sidebar;
  popups!: Popups;
  results!: ResultsView;
  help!: HelpPanel;
  mobile!: MobileBar;
  mindmap = new MindMap();
  route: Route = { doc: null, zoom: ROOT, q: '' };
  private root = document.getElementById('app')!;
  private docArea = h('div.doc-wrap');
  private emptyState = h('div.empty-state');
  private backlinksEl = h('div.backlinks', { hidden: true });
  private statusBar = h('div.status-bar', { hidden: true });
  private customCss = h('style', { id: 'custom-css' });
  private searchInput = h('input', { placeholder: 'Search… (is:completed, #tag, edited:3d)' });
  private searchTips = h('div.search-tips', { hidden: true });
  private searchBox = h('div.search-box', { hidden: true }, icon('search'), this.searchInput,
    h('button.icon-btn', { title: 'Bookmark this search', onclick: () => this.bookmarkCurrent() }, icon('star')),
    h('button.icon-btn', { title: 'Close search', onclick: () => this.closeSearch() }, icon('close')));
  private syncLabel = h('span.sync-status', { onclick: () => this.syncDialog() });
  private docTimer = 0;
  private unsubDoc: (() => void) | null = null;

  async start() {
    await this.sync.init();
    this.ws = new Workspace(this.sync);
    this.ws.ensureStructure();
    this.sync.paused = (doc) => doc !== this.route.doc && !!this.ws.get(doc)?.archived;
    this.sync.onState = (s) => this.paintSync(s);
    this.sync.onRemoteDoc = () => this.sidebar?.render();
    (window as unknown as { __qa: () => unknown }).__qa = () => qaSnapshot(this.editor);

    this.editor = new Editor({
      onZoom: (zoom) => this.go({ ...this.route, zoom }, true, false),
      onTitleEdit: (title) => this.route.doc && this.ws.syncTitleFromDoc(this.route.doc, title),
      onAppCommand: (id) => this.appCommand(id),
      openLink: (url) => this.openLink(url),
      onTagClick: (tag, alt) => (alt ? this.globalSearch(tag) : this.toggleSearchTerm(tag)),
      onTextInput: (ctx) => this.popups.onTextInput(ctx),
      onDateClick: (id, s, e, el) => this.popups.openDateChip(id, s, e, el),
      onItemMenu: (id, x, y) => itemMenu(this.editor, id, x, y, {
        bookmark: () => {
          const doc = this.route.doc;
          if (doc) this.ws.addBookmark({ title: inlineToPlain(String(this.editor.tree.get(id)?.props.c ?? '')) || 'Untitled', doc, zoom: id });
          toast('Bookmarked');
        },
        exportItem: () => this.exportItems(this.editor.targets().length > 1 ? this.editor.targets() : [id]),
        setInbox: () => {
          if (this.route.doc) this.ws.setPref('inbox_location', id === ROOT ? this.route.doc : `${this.route.doc}/${id}`);
          toast('Inbox set');
        },
        getLink: () => this.getLinkDialog(id),
        showReferences: () => this.showReferences(id),
        share: () => this.route.doc && shareDialog(this.sync, this.route.doc, id === ROOT ? null : id,
          id === ROOT ? this.ws.title(this.route.doc) : inlineToPlain(String(this.editor.tree.get(id)?.props.c ?? '')) || 'Untitled'),
      }),
      backlinkCounts: () => (this.route.doc && this.ws.pref('show_backlinks', false) ? backlinkCounts(this.trees(), this.route.doc) : null),
      onBacklinks: (id) => this.showReferences(id),
      openInNewTab: (zoom) => this.route.doc && window.open(this.itemUrl(this.route.doc, zoom), '_blank', 'noopener'),
      uploadFiles: (files) => this.uploadFiles(files),
    });
    this.popups = new Popups(this.editor, {
      tags: (global) => this.tagCounts()[global ? 'all' : 'doc'],
      globalTags: () => this.ws.pref('global_tag_autocomplete', false),
      linkCandidates: () => this.linkCandidates(),
      itemUrl: (doc, id) => formatHash({ doc, zoom: id, q: '' }),
      startDay: () => ({ sat: 6, sun: 0, mon: 1 } as Record<string, number>)[String(this.ws.pref('start_day', 'sun'))] ?? 0,
      hour12: () => this.ws.pref('time_format', '12h') === '12h',
      encodeTimezone: () => this.ws.pref('show_timezone', false),
    });
    this.sidebar = new Sidebar(this.ws, {
      currentDoc: () => this.route.doc,
      openDoc: (doc) => this.go({ doc, zoom: this.rememberedZoom(doc), q: '' }),
      openBookmark: (b) => this.openBookmark(b),
      exportDoc: (id) => exportDialog(this.sync.doc(id).live, ROOT, this.ws.title(id), this.exportOpts(id)),
      importOpml: (parent) => this.importFiles(parent),
      tags: () => this.tagCounts(),
      searchTag: (tag, global) => (global ? this.globalSearch(tag) : this.openSearch(tag)),
      renameTag: (tag, to) => this.renameTag(tag, to),
      bookmarkCurrent: () => this.bookmarkCurrent(),
      findFile: () => this.goAnywhere(),
      gotoInbox: () => this.appCommand('goto-inbox'),
      copyDoc: (id) => this.copyDoc(id),
      versionHistory: (id) => this.versionHistory(id),
      share: (id) => shareDialog(this.sync, id, null, this.ws.title(id)),
      cleanNewlines: (id) => this.cleanNewlines(id),
      dropItems: (doc, ids, copy) => this.dropItems(doc, ids, copy),
    });
    installDrag(this.editor, {
      insideAtTop: () => this.ws.pref<string>('move_item_sent_to_position', 'bottom') === 'top',
      external: this.sidebar.externalDrop(),
    });
    this.installDocDrop();
    this.results = new ResultsView({
      docs: () => this.liveDocs(),
      tree: (doc) => this.sync.doc(doc).live,
      title: (doc) => this.ws.title(doc),
      showChecked: (doc) => this.docShowChecked(doc),
      open: (doc, id) => this.revealItem(doc, id),
      back: () => this.go({ ...this.route, view: undefined, sort: undefined, q: this.route.view === 'flat' ? this.route.q : '' }),
      update: (q, sort) => this.go({ ...this.route, q, sort }, false),
    });
    this.help = new HelpPanel(() => this.editor.keymap);
    this.mobile = new MobileBar(this.ws, (cmd) => this.editor.exec(cmd));
    this.ws.onChange(() => {
      this.applyPrefs();
      if (this.route.doc && !this.ws.get(this.route.doc)) this.go({ doc: null, zoom: ROOT, q: '' }, false);
    });

    this.buildShell();
    this.paintSync(this.sync.state);
    this.applyPrefs();
    if (!this.ws.files().length && !this.sync.config) this.createWelcome();

    window.addEventListener('hashchange', () => this.applyRoute(parseHash()));
    document.addEventListener('keydown', (e) => this.onGlobalKey(e));
    window.addEventListener('beforeunload', (e) => {
      this.sync.flushSaves();
      // edits not on the server yet while it is reachable: warn (offline edits are kept locally anyway)
      if (this.sync.config && this.sync.dirty && this.sync.state === 'saving') {
        e.preventDefault();
        e.returnValue = '';
      }
    });

    const initial = parseHash();
    if (!initial.doc && !initial.view) {
      initial.doc = localStorage.getItem(LAST_DOC) ?? this.ws.allDocs()[0]?.id ?? null;
      if (initial.doc) initial.zoom = this.rememberedZoom(initial.doc);
    }
    this.applyRoute(initial);
    this.go(initial, false, false);
  }

  // ---- shell ---------------------------------------------------------------------

  private buildShell() {
    const btn = (name: string, title: string, run: (e: MouseEvent) => void) =>
      h('button.icon-btn', { title, onclick: run }, icon(name));
    const top = h('header.topbar', {},
      btn('side', 'Toggle sidebar', () => this.toggleSidebar()),
      btn('back', 'Back', () => history.back()),
      btn('fwd', 'Forward', () => history.forward()),
      h('span.spacer'),
      this.syncLabel,
      btn('search', 'Search (Ctrl+F)', () => this.openSearch()),
      btn('gear', 'Settings', () => this.settingsDialog()),
      btn('menu', 'Menu', (e) => this.mainMenu(e)),
    );
    const actions = h('div.doc-actions', {},
      btn('star', 'Bookmark this view', () => this.bookmarkCurrent()),
      btn('eye', 'View options', (e) => this.viewOptions(e)));
    const tools = h('div.doc-tools', {}, this.searchBox, this.searchTips, actions);
    const scroll = h('div.doc-scroll.o-scroll', {}, tools, this.docArea);
    this.docArea.append(this.editor.el, this.backlinksEl, this.results.el, this.emptyState);
    const splitter = h('div.splitter');
    splitter.addEventListener('pointerdown', (e) => {
      splitter.setPointerCapture(e.pointerId);
      const move = (ev: PointerEvent) => document.documentElement.style.setProperty('--side-w', `${Math.max(160, Math.min(520, ev.clientX))}px`);
      splitter.addEventListener('pointermove', move);
      splitter.addEventListener('pointerup', () => splitter.removeEventListener('pointermove', move), { once: true });
    });
    const main = h('div.main', {}, this.sidebar.el, splitter, h('div.scrim', { onclick: () => this.toggleSidebar(false) }), scroll, this.help.el);
    this.root.append(h('div.app', {}, top, main, this.mobile.el), this.statusBar, this.mindmap.el);
    this.installImagePreview();
    document.head.append(this.customCss);

    this.searchInput.addEventListener('input', () => {
      this.runSearch(true);
      this.paintSearchTips();
    });
    this.searchInput.addEventListener('focus', () => this.paintSearchTips());
    this.searchInput.addEventListener('blur', () => setTimeout(() => (this.searchTips.hidden = true), 150));
    this.searchInput.addEventListener('keydown', (e) => {
      if (e.isComposing) return;
      const combo = comboOf(e);
      if (combo === 'Escape') this.closeSearch();
      else if (combo === 'Enter') this.runSearch(false);
      else if (combo === 'Mod+Enter') this.globalSearch(this.searchInput.value);
      else if (combo === 'Shift+Enter') this.flatSearch(this.searchInput.value);
      else if (combo === 'Tab') this.editor.exec('goto-first');
      else return;
      e.preventDefault();
    });
  }

  /** Hovering an image chip (inline images off) shows the picture. */
  private installImagePreview() {
    let img: HTMLImageElement | null = null;
    let timer = 0;
    const hide = () => {
      clearTimeout(timer);
      img?.remove();
      img = null;
    };
    document.addEventListener('mouseover', (e) => {
      const a = (e.target as HTMLElement).closest?.<HTMLAnchorElement>('.o-image[href]');
      if (!a || a.closest('.o-editing')) return;
      hide();
      timer = window.setTimeout(() => {
        const r = a.getBoundingClientRect();
        img = h('img.img-preview', { src: a.href, alt: '' });
        img.style.left = `${Math.min(r.left, innerWidth - 330)}px`;
        img.style.top = `${r.bottom + 6 + 250 > innerHeight ? r.top - 250 : r.bottom + 6}px`;
        img.addEventListener('error', hide);
        document.body.append(img);
      }, 300);
      a.addEventListener('mouseleave', hide, { once: true });
    });
  }

  private paintSearchTips() {
    const q = this.searchInput.value.trim();
    this.searchTips.hidden = !q || !this.ws.pref('show_search_tips', true) || document.activeElement !== this.searchInput;
    if (this.searchTips.hidden) return;
    const tip = (label: string, key: string, run: () => void) =>
      h('div.tip', { onmousedown: (e: Event) => (e.preventDefault(), run()) }, h('span', {}, label), h('span.k', {}, key));
    this.searchTips.replaceChildren(
      tip('Flat search', 'Shift+Enter', () => this.flatSearch(this.searchInput.value)),
      tip('Search in all documents', 'Ctrl+Enter', () => this.globalSearch(this.searchInput.value)),
      h('div.tip-hint', {}, 'Operators: is:completed, -is:completed, #tag, "exact phrase", edited:4d, created:1w, has:date, due:today, parent:text…'));
  }

  private toggleSidebar(force?: boolean) {
    const app = this.root.firstElementChild as HTMLElement;
    if (matchMedia('(max-width: 760px)').matches) app.classList.toggle('side-open', force);
    else this.sidebar.el.hidden = force === undefined ? !this.sidebar.el.hidden : !force;
  }

  private sidebarShown(): boolean {
    const app = this.root.firstElementChild as HTMLElement;
    return matchMedia('(max-width: 760px)').matches ? app.classList.contains('side-open') : !this.sidebar.el.hidden;
  }

  /** Pane shortcut: open the sidebar on that pane, or close it when it already shows it. */
  private togglePane(p: PaneName) {
    if (this.sidebarShown() && this.sidebar.active === p) this.toggleSidebar(false);
    else {
      this.sidebar.show(p);
      this.toggleSidebar(true);
    }
  }

  private paintSync(s: SyncState) {
    const text: Record<SyncState, string> = {
      synced: 'Synced', saving: 'Saving…', 'saved-locally': 'Saved locally', offline: 'Offline',
      error: "Can't sync", 'local-only': 'Local only',
    };
    this.syncLabel.textContent = text[s];
    this.syncLabel.classList.toggle('error', s === 'error');
    this.syncLabel.title = this.sync.lastError || '';
  }

  // ---- routing -------------------------------------------------------------------

  go(r: Route, push = true, apply = true) {
    const hash = formatHash(r);
    if (location.hash !== hash) {
      if (push) history.pushState(null, '', hash);
      else history.replaceState(null, '', hash);
    }
    if (apply) this.applyRoute(r);
    else this.route = r;
    if (r.doc && !r.view) this.rememberZoom(r.doc, r.zoom);
  }

  private applyRoute(r: Route) {
    const docChanged = r.doc !== this.route.doc;
    // phones: the file pane is a drawer over the document; opening something closes it
    if ((docChanged || r.view !== this.route.view) && matchMedia('(max-width: 760px)').matches) this.toggleSidebar(false);
    const missing = !!r.doc && !this.ws.get(r.doc);
    if (missing) r = { doc: null, zoom: ROOT, q: '', view: r.view === 'global' ? 'global' : undefined, sort: r.sort };
    this.route = r;
    if (r.view === 'global') {
      this.editor.el.hidden = true;
      this.backlinksEl.hidden = true;
      this.emptyState.hidden = true;
      this.searchBox.hidden = true;
      this.results.show({ mode: 'global', q: r.q, sort: r.sort ?? '', doc: r.doc, zoom: r.zoom });
      document.title = 'Search everywhere — Outliner';
      return;
    }
    if (!r.doc) {
      this.results.hide();
      this.editor.el.hidden = true;
      this.backlinksEl.hidden = true;
      this.emptyState.hidden = false;
      this.emptyState.replaceChildren(
        missing ? h('div.empty-title', {}, 'Cannot open document') : h('div.empty-title', {}, 'No document is open'),
        missing ? h('div', {}, 'It may have been deleted, or the link is wrong.') : '',
        h('div', {}, h('a', { onclick: () => this.goAnywhere() }, 'Find a document (Ctrl+O)')),
        h('div', {}, h('a', { onclick: () => this.sidebar.newFile('doc', null) }, 'Create a new document')),
        h('div', {}, 'or open one from the file pane.'),
      );
      this.sidebar.render();
      this.paintStatus();
      return;
    }
    localStorage.setItem(LAST_DOC, r.doc);
    this.emptyState.hidden = true;
    const session = this.sync.doc(r.doc);
    if (docChanged || this.editor.docSession !== session) {
      this.applyDocPrefs();
      this.editor.setDoc(session, r.zoom);
      this.watchDoc();
      this.sidebar.render();
      if (this.ws.pref('auto_focus_first_item', false) && !r.view) setTimeout(() => this.editor.exec('goto-first'));
    } else if (r.zoom !== this.editor.view.zoom) {
      this.editor.setZoom(r.zoom);
      if (this.mindmap.active) this.applyDocPrefs();
    }
    if (r.view === 'flat') {
      this.editor.el.hidden = true;
      this.backlinksEl.hidden = true;
      this.results.show({ mode: 'flat', q: r.q, sort: r.sort ?? '', doc: r.doc, zoom: r.zoom });
      document.title = `Search: ${r.q} — Outliner`;
      return;
    }
    this.results.hide();
    this.editor.el.hidden = false;
    if (r.q !== this.searchInput.value) {
      this.searchInput.value = r.q;
      this.searchBox.hidden = !r.q;
      this.runSearch(false);
    }
    this.paintBacklinks();
    this.paintStatus();
    document.title = `${this.ws.title(r.doc)} — Outliner`;
  }

  /** Re-paint the doc-dependent panels (backlinks, word count) a moment after edits. */
  private watchDoc() {
    this.unsubDoc?.();
    this.unsubDoc = this.editor.docSession.onChange(() => {
      clearTimeout(this.docTimer);
      this.docTimer = window.setTimeout(() => {
        this.paintBacklinks();
        this.paintStatus();
        if (this.results.active) this.results.render();
        if (this.mindmap.active) this.mindmap.render();
      }, 400);
    });
  }

  private rememberZoom(doc: string, zoom: NodeId) {
    const m = JSON.parse(localStorage.getItem(ZOOMS) ?? '{}') as Record<string, string>;
    if (zoom === ROOT) delete m[doc];
    else m[doc] = zoom;
    localStorage.setItem(ZOOMS, JSON.stringify(m));
  }

  /** Last zoom in a document (per device), if the item still exists. */
  private rememberedZoom(doc: string): NodeId {
    const z = (JSON.parse(localStorage.getItem(ZOOMS) ?? '{}') as Record<string, string>)[doc];
    if (!z) return ROOT;
    const t = this.sync.doc(doc).live;
    return t.get(z) && t.isAttached(z) ? z : ROOT;
  }

  private itemUrl(doc: string, zoom: NodeId): string {
    return `${location.origin}${location.pathname}${formatHash({ doc, zoom, q: '' })}`;
  }

  // ---- documents in scope --------------------------------------------------------

  /** Documents that are loaded and synced: archived ones are left alone unless open. */
  private liveDocs() {
    return this.ws.allDocs().filter((d) => !d.archived || d.id === this.route.doc);
  }

  private trees() {
    return this.liveDocs().map((d) => ({ doc: d.id, tree: this.sync.doc(d.id).live }));
  }

  private docShowChecked(doc: string): boolean {
    const v = this.ws.docPref<string>(doc, 'checked', 'global');
    return v === 'global' ? this.ws.pref('show_checked', true) : v === 'show';
  }

  private docNotes(doc: string): 'show' | 'teaser' | 'hide' {
    const v = this.ws.docPref<string>(doc, 'notes', 'global');
    return (v === 'global' ? this.ws.pref('show_notes', 'show') : v) as 'show' | 'teaser' | 'hide';
  }

  // ---- search --------------------------------------------------------------------

  /** where the caret was when the search box opened (Escape puts it back) */
  private preSearchSel: Sel | null = null;

  openSearch(q?: string) {
    if (this.searchBox.hidden || document.activeElement !== this.searchInput) {
      const sel = this.editor.currentSel();
      this.preSearchSel = this.editor.el.contains(document.activeElement) && sel.k !== 'none' ? sel : null;
    }
    if (this.route.view) this.go({ ...this.route, view: undefined, sort: undefined, q: q ?? this.route.q });
    this.searchBox.hidden = false;
    if (q !== undefined) {
      this.searchInput.value = q;
      this.runSearch(false);
    }
    this.searchInput.focus();
    this.searchInput.select();
  }

  closeSearch() {
    this.searchInput.value = '';
    this.searchBox.hidden = true;
    this.searchTips.hidden = true;
    this.runSearch(false);
    const sel = this.preSearchSel;
    this.preSearchSel = null;
    const ids = sel?.k === 'text' ? [sel.id] : sel?.k === 'block' ? sel.ids : [];
    if (sel && ids.length && ids.every((id) => this.editor.tree.get(id) && this.editor.tree.isAttached(id))) this.editor.applySel(sel);
    else this.editor.el.focus();
  }

  private toggleSearchTerm(tag: string) {
    const terms = this.searchInput.value.split(/\s+/).filter(Boolean);
    const has = terms.includes(tag);
    this.openSearch(has ? terms.filter((t) => t !== tag).join(' ') : [...terms, tag].join(' '));
    if (has && !this.searchInput.value) this.closeSearch();
  }

  private runSearch(typing: boolean) {
    const q = this.searchInput.value.trim();
    if (!this.route.doc) return;
    if (!q) {
      setRenderOptions({ ...this.renderOpts() });
      this.editor.el.classList.remove('o-searching');
      this.editor.setFilter(null);
      this.editor.refreshText();
    } else {
      const m = compileQuery(q, Date.now(), { typing });
      const res = searchTree(this.editor.tree, m, this.editor.view.zoom, { showChecked: this.editor.view.showChecked });
      setRenderOptions({ ...this.renderOpts(), highlight: m.terms });
      this.editor.el.classList.add('o-searching');
      this.editor.setFilter(res.visible);
      this.editor.refreshText();
    }
    if (!typing && q !== this.route.q) this.go({ ...this.route, q }, false, false);
  }

  private flatSearch(q: string) {
    if (!this.route.doc || !q.trim()) return;
    this.searchTips.hidden = true;
    this.go({ ...this.route, q: q.trim(), view: 'flat' });
  }

  private globalSearch(q: string) {
    this.searchTips.hidden = true;
    this.go({ ...this.route, q: q.trim(), view: 'global' });
  }

  /** Open doc, clear search, put caret on item (zooming out if needed). */
  revealItem(doc: string, id: NodeId) {
    this.searchInput.value = '';
    this.searchBox.hidden = true;
    const t = this.sync.doc(doc).live;
    const zoom = this.route.doc === doc && t.ancestors(id).includes(this.route.zoom) ? this.route.zoom : ROOT;
    this.go({ doc, zoom, q: '' });
    this.runSearch(false);
    this.editor.applySel({ k: 'text', id, f: 'c', s: 0, e: 0 });
  }

  /** Items of all documents for `[[` links (current document first). */
  private linkCandidates() {
    const docs = [...this.liveDocs()];
    const cur = this.route.doc;
    docs.sort((a, b) => Number(b.id === cur) - Number(a.id === cur));
    const out: { doc: string; id: NodeId; text: string; path: string }[] = [];
    for (const d of docs) {
      const t = this.sync.doc(d.id).live;
      for (const n of t.nodes.values()) {
        const text = String(n.props.c ?? '');
        if (n.id === ROOT || !text.trim() || !t.isAttached(n.id)) continue;
        const crumb = breadcrumb(t, n.id);
        out.push({ doc: d.id, id: n.id, text, path: crumb ? `${d.title} › ${crumb}` : d.title });
      }
    }
    return out;
  }

  private tagCounts() {
    const count = (trees: Tree[]) => {
      const m = new Map<string, [string, number]>();
      for (const t of trees) {
        for (const n of t.nodes.values()) {
          if (!t.isAttached(n.id)) continue;
          for (const src of [n.props.c, n.props.n]) {
            if (typeof src !== 'string' || !src) continue;
            for (const tag of extractTags(src)) {
              const k = tag.toLowerCase();
              const cur = m.get(k);
              m.set(k, [cur?.[0] ?? tag, (cur?.[1] ?? 0) + 1]);
            }
          }
        }
      }
      return [...m.values()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
    };
    const doc = this.route.doc ? count([this.sync.doc(this.route.doc).live]) : [];
    const all = count(this.trees().map((d) => d.tree));
    return { doc, all };
  }

  private renameTag(tag: string, to: string | null) {
    let n = 0;
    for (const { tree, doc } of this.trees()) {
      const s = this.sync.doc(doc);
      const tx = new Tx(tree);
      for (const node of tree.nodes.values()) {
        if (!tree.isAttached(node.id)) continue;
        const patch: Record<string, string> = {};
        for (const f of ['c', 'n'] as const) {
          const v = node.props[f];
          if (typeof v !== 'string' || !v) continue;
          const next = replaceTag(v, tag, to);
          if (next !== v) patch[f] = next;
        }
        if (Object.keys(patch).length) {
          tx.set(node.id, patch);
          n++;
        }
      }
      if (tx.ops.length) s.commit(tx, { before: null, after: null });
    }
    toast(`${to ? `Renamed ${tag} to ${to}` : `Removed ${tag}`} in ${n} item${n === 1 ? '' : 's'}`);
    this.sidebar.render();
  }

  // ---- backlinks / references ------------------------------------------------------

  private paintBacklinks() {
    const doc = this.route.doc;
    const on = this.ws.pref('show_backlinks', false);
    const refs = on && doc ? findBacklinks(this.trees(), doc, this.editor.view.zoom) : [];
    this.backlinksEl.hidden = !refs.length || this.editor.el.hidden;
    if (!refs.length) return;
    this.backlinksEl.replaceChildren(h('div.backlinks-head', {}, 'This item is referenced in:'),
      ...refs.map((r) => this.refRow(r.doc, r.id)));
  }

  private refRow(doc: string, id: NodeId): HTMLElement {
    const t = this.sync.doc(doc).live;
    const text = h('div.res-text');
    text.innerHTML = renderText(String(t.get(id)?.props.c ?? '')) || '&nbsp;';
    const crumb = breadcrumb(t, id);
    return h('div.res-item', { onclick: (e: MouseEvent) => !(e.target as HTMLElement).closest('a') && this.revealItem(doc, id) },
      h('div.res-crumb', {}, crumb ? `${this.ws.title(doc)} › ${crumb}` : this.ws.title(doc)), text);
  }

  private showReferences(id: NodeId) {
    const doc = this.route.doc;
    if (!doc) return;
    const refs = findBacklinks(this.trees(), doc, id);
    const body = refs.length
      ? h('div.refs', {}, ...refs.map((r) => this.refRow(r.doc, r.id)))
      : h('p', {}, 'No other item links to this one yet. Link to it with [[ in any item, or "Get link".');
    const close = modal('References', body, [{ label: 'Close', primary: true }]);
    body.addEventListener('click', (e) => (e.target as HTMLElement).closest('.res-item') && close());
  }

  private getLinkDialog(id: NodeId) {
    const doc = this.route.doc;
    if (!doc) return;
    const input = h('input', { value: this.itemUrl(doc, id), readOnly: true });
    modal('Link to this item', h('div', {}, h('label.field', {}, 'Item URL', input)), [
      { label: 'Copy', run: () => (void navigator.clipboard.writeText(input.value).then(() => toast('Link copied to your clipboard!')), false) },
      { label: 'Done', primary: true },
    ]);
    input.select();
  }

  // ---- commands --------------------------------------------------------------------

  private onGlobalKey(e: KeyboardEvent) {
    if (e.defaultPrevented || e.isComposing) return;
    const combo = comboOf(e);
    if (combo === 'Escape' && this.help.el.contains(document.activeElement)) return this.help.toggle(false);
    const cmd = this.editor.keymap.resolve(combo);
    if (!cmd) return;
    const inEditor = this.editor.el.contains(document.activeElement);
    // editor commands from outside the editor (e.g. nothing focused)
    if (!inEditor && !APP_COMMANDS_ANYWHERE.includes(cmd)) return;
    if ((document.activeElement instanceof HTMLInputElement || document.activeElement instanceof HTMLTextAreaElement) &&
      !['go-anywhere', 'goto-item', 'help', 'inbox-capture'].includes(cmd)) return;
    if (this.editor.exec(cmd)) e.preventDefault();
  }

  appCommand(id: string): boolean {
    const doc = this.route.doc;
    switch (id) {
      case 'help':
        this.help.toggle();
        return true;
      case 'settings':
        this.settingsDialog();
        return true;
      case 'toggle-file-pane':
        this.togglePane('files');
        return true;
      case 'toggle-bookmark-pane':
        this.togglePane('bookmarks');
        return true;
      case 'toggle-tag-pane':
        this.togglePane('tags');
        return true;
      case 'save':
        this.sync.flushSaves();
        void this.sync.syncNow();
        return true;
      case 'search':
        this.openSearch();
        return true;
      case 'search-replace':
        if (doc) searchReplaceDialog(this.editor, this.searchInput.value.trim());
        return true;
      case 'go-anywhere':
        this.goAnywhere();
        return true;
      case 'goto-item':
        this.itemFinder();
        return true;
      case 'back':
        history.back();
        return true;
      case 'forward':
        history.forward();
        return true;
      case 'bookmark':
        this.bookmarkCurrent();
        return true;
      case 'toggle-show-checked':
        this.ws.setPref('show_checked', !this.ws.pref('show_checked', true));
        return true;
      case 'toggle-show-notes': {
        const cur = this.ws.pref<string>('show_notes', 'show');
        this.ws.setPref('show_notes', cur === 'hide' ? 'show' : 'hide');
        return true;
      }
      case 'toggle-doc-checked':
        if (doc) this.ws.setDocPref(doc, 'checked', this.docShowChecked(doc) ? 'hide' : 'show');
        return true;
      case 'toggle-doc-notes':
        if (doc) this.ws.setDocPref(doc, 'notes', this.docNotes(doc) === 'hide' ? 'show' : 'hide');
        return true;
      case 'toggle-spellcheck':
        this.ws.setPref('spellcheck', !this.ws.pref('spellcheck', false));
        toast(this.ws.pref('spellcheck', false) ? 'Spell checker on' : 'Spell checker off');
        return true;
      case 'copy-link': {
        const target = this.editor.targets()[0];
        if (target && doc) {
          void navigator.clipboard.writeText(this.itemUrl(doc, target));
          toast('Link copied to your clipboard!');
        }
        return true;
      }
      case 'get-link': {
        const target = this.editor.targets()[0] ?? this.editor.view.zoom;
        this.getLinkDialog(target);
        return true;
      }
      case 'show-references':
        this.showReferences(this.editor.targets()[0] ?? this.editor.view.zoom);
        return true;
      case 'add-date':
        return this.popups.addDate();
      case 'insert-hash':
        document.execCommand('insertText', false, '#');
        return true;
      case 'insert-at':
        document.execCommand('insertText', false, '@');
        return true;
      case 'move-to':
        this.moveToDialog();
        return true;
      case 'insert-template':
        this.templateDialog();
        return true;
      case 'export':
        if (doc) this.exportItems(this.editor.targets().length ? this.editor.targets() : [this.editor.view.zoom]);
        return true;
      case 'inbox-capture':
        captureDialog(this.ws, this.sync);
        return true;
      case 'goto-inbox': {
        const r = inboxRoute(this.ws, this.sync);
        if (r) this.go({ doc: r.doc, zoom: r.zoom, q: '' });
        return true;
      }
      case 'version-history':
        if (doc) this.versionHistory(doc);
        return true;
      case 'download-backup':
        this.downloadBackup();
        return true;
    }
    return false;
  }

  private mainMenu(e: MouseEvent) {
    const doc = this.route.doc;
    const items: MenuItem[] = [
      { label: 'Shortcuts & formatting', key: 'Ctrl+/', run: () => this.help.toggle(true) },
      { label: 'Settings', key: 'Ctrl+,', run: () => this.settingsDialog() },
      { label: '', sep: true },
      { label: 'Capture to inbox', key: 'Ctrl+Shift+I', run: () => this.appCommand('inbox-capture') },
      { label: 'Go to inbox', run: () => this.appCommand('goto-inbox') },
      { label: 'Search everywhere', run: () => this.globalSearch('') },
      { label: '', sep: true },
    ];
    if (doc) items.push({ label: 'Version history of this document', run: () => this.versionHistory(doc) });
    items.push(
      { label: 'Download backup', run: () => this.downloadBackup() },
      { label: 'Import OPML / text…', run: () => this.importFiles(null) },
      { label: 'Import from WorkFlowy…', run: () => modal('Import from WorkFlowy', h('div', {},
        h('p', {}, 'In WorkFlowy, open the list you want, choose Export → OPML and save the file. Then import it here; each file becomes a document.')),
      [{ label: 'Cancel' }, { label: 'Choose OPML file…', primary: true, run: () => this.importFiles(null) }]) },
      { label: 'Print', run: () => window.print() },
      { label: '', sep: true },
      { label: 'Sync status', run: () => this.syncDialog() },
      { label: 'Tutorial', run: () => tutorial(() => this.ws.setPref('finished_onboarding', true)) },
      { label: "What's new", run: () => whatsNew() },
      { label: 'About', run: () => this.aboutDialog() },
    );
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    showMenu(r.right - 240, r.bottom + 4, items);
  }

  private viewOptions(e: MouseEvent) {
    const doc = this.route.doc;
    if (!doc) return;
    const opt = (key: string, choices: [string, string][], globalLabel?: string): MenuItem[] => {
      const cur = this.ws.docPref<string>(doc, key, choices[0]![0]);
      return choices.map(([v, l]) => ({ label: v === 'global' && globalLabel ? `${l} (${globalLabel})` : l, checked: cur === v, run: () => this.ws.setDocPref(doc, key, v === choices[0]![0] ? null : v) }));
    };
    const gChecked = this.ws.pref('show_checked', true) ? 'show' : 'hide';
    const gNotes = this.ws.pref<string>('show_notes', 'show');
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    showMenu(r.right - 220, r.bottom + 4, [
      { label: 'Checked items', sub: opt('checked', [['global', 'Global'], ['show', 'Show'], ['hide', 'Hide']], gChecked) },
      { label: 'Notes', sub: opt('notes', [['global', 'Global'], ['show', 'Show'], ['teaser', '1st line'], ['hide', 'Hide']], gNotes) },
      { label: 'Layout', sub: opt('layout', [['list', 'List'], ['article', 'Article'], ['mindmap', 'Mind map (read-only)']]) },
      { label: 'Text direction', sub: opt('dir', [['global', 'Global'], ['ltr', 'Left-to-right'], ['rtl', 'Right-to-left']]) },
      { label: '', sep: true },
      {
        label: 'Lock document (read-only)',
        checked: this.ws.docPref(doc, 'locked', false),
        run: () => this.ws.setDocPref(doc, 'locked', this.ws.docPref(doc, 'locked', false) ? null : true),
      },
    ]);
  }

  private goAnywhere() {
    finder('Search any document or bookmark to jump to…', (q) => {
      const docs = this.ws.allDocs().map((d) => ({ label: d.title, sub: d.archived ? `${d.path || 'Document'} (archived)` : d.path, run: () => this.go({ doc: d.id, zoom: this.rememberedZoom(d.id), q: '' }) }));
      const bms = this.ws.bookmarks().map((b) => ({ label: `★ ${b.title}`, sub: 'Bookmark', run: () => this.openBookmark(b) }));
      return fuzzyFilter(q, [...docs, ...bms], (x) => x.label).map((x) => x.item);
    });
  }

  private allItems() {
    const all: { doc: string; id: NodeId; t: Tree; text: string }[] = [];
    for (const d of this.liveDocs()) {
      const t = this.sync.doc(d.id).live;
      for (const n of t.nodes.values()) {
        if (n.id === ROOT || !t.isAttached(n.id)) continue;
        const text = String(n.props.c ?? '');
        if (text) all.push({ doc: d.id, id: n.id, t, text });
      }
    }
    return all;
  }

  private itemFinder() {
    const all = this.allItems();
    finder('Jump to any item…', (q) =>
      (q ? fuzzyFilter(q, all, (x) => x.text).map((m) => m.item) : all.slice(0, 50)).slice(0, 50).map((x) => ({
        label: x.text,
        sub: `${this.ws.title(x.doc)} › ${breadcrumb(x.t, x.id)}`,
        run: () => this.revealItem(x.doc, x.id),
      })),
    );
  }

  /** Insert template: a copy of a chosen item (with its subtree) goes after the current item. */
  private templateDialog() {
    const target = this.editor.targets()[0];
    if (!target || !this.route.doc) return;
    const all = this.allItems();
    finder('Insert template: pick an item to copy here…', (q) =>
      (q ? fuzzyFilter(q, all, (x) => x.text).map((m) => m.item) : all.slice(0, 50)).slice(0, 50).map((x) => ({
        label: x.text,
        sub: `${this.ws.title(x.doc)} › ${breadcrumb(x.t, x.id)}`,
        run: () => {
          const item = toPortable(x.t, x.id);
          this.editor.run((tx) => {
            const t = tx.tree;
            const zoom = this.editor.view.zoom;
            const empty = target !== zoom && !t.get(target)!.props.c && !t.get(target)!.children.length;
            const parent = target === zoom ? zoom : t.get(target)!.parent!;
            const after = target === zoom ? t.get(zoom)!.children.at(-1) ?? null : target;
            const [nid] = insertPortable(tx, parent, after, [item]);
            if (empty) tx.move(target, null, null);
            return { sel: { k: 'text', id: nid!, f: 'c', s: 0, e: 0 } };
          });
        },
      })),
    );
  }

  private moveToDialog() {
    const ids = this.editor.targets().filter((id) => id !== this.editor.view.zoom);
    const srcDoc = this.route.doc;
    if (!ids.length || !srcDoc) return;
    const src = this.sync.doc(srcDoc);
    const targets: { doc: string; id: NodeId; label: string; sub: string }[] = [];
    for (const d of this.liveDocs()) {
      const t = this.sync.doc(d.id).live;
      targets.push({ doc: d.id, id: ROOT, label: d.title, sub: 'Document' });
      for (const n of t.nodes.values()) {
        if (n.id === ROOT || !t.isAttached(n.id) || !n.props.c) continue;
        if (d.id === srcDoc && ids.some((x) => x === n.id || t.isAncestor(x, n.id))) continue;
        targets.push({ doc: d.id, id: n.id, label: String(n.props.c), sub: `${d.title} › ${breadcrumb(t, n.id)}` });
      }
    }
    const bottom = this.ws.pref('move_item_sent_to_position', 'bottom') === 'bottom';
    finder('Move to…', (q) =>
      fuzzyFilter(q, targets, (x) => x.label).slice(0, 50).map(({ item: x }) => ({
        label: x.label,
        sub: x.sub,
        run: () => {
          if (x.doc === srcDoc) {
            this.editor.run((tx) => {
              const kids = tx.tree.get(x.id)!.children;
              let after = bottom ? kids[kids.length - 1] ?? null : null;
              for (const id of ids) {
                tx.move(id, x.id, after);
                after = id;
              }
              return { sel: { k: 'none' } };
            });
          } else {
            this.copyInto(x.doc, x.id, ids.map((id) => toPortable(src.live, id)), bottom);
            this.editor.exec('delete-left');
          }
          toast(`Moved to ${x.label}`);
        },
      })),
    );
  }

  private copyInto(doc: string, parent: NodeId, items: ReturnType<typeof toPortable>[], bottom: boolean) {
    const dst = this.sync.doc(doc);
    const tx = new Tx(dst.live);
    const kids = dst.live.get(parent)!.children;
    insertPortable(tx, parent, bottom ? kids[kids.length - 1] ?? null : null, items);
    dst.commit(tx, { before: null, after: null });
  }

  /** Items dragged onto the file pane: into a document, or (doc null) each becomes a new document. */
  private dropItems(doc: string | null, ids: NodeId[], copy: boolean) {
    const srcDoc = this.route.doc;
    if (!srcDoc || !ids.length) return;
    const t = this.editor.tree;
    const items = ids.map((id) => toPortable(t, id));
    const bottom = this.ws.pref('move_item_sent_to_position', 'bottom') === 'bottom';
    if (doc === srcDoc) {
      if (copy) this.copyInto(doc, ROOT, items, bottom);
      else this.editor.run((tx) => {
        let after = bottom ? tx.tree.get(ROOT)!.children.filter((c) => !ids.includes(c)).at(-1) ?? null : null;
        for (const id of ids) {
          tx.move(id, ROOT, after);
          after = id;
        }
        return { sel: { k: 'none' } };
      });
      return;
    }
    if (doc) {
      this.copyInto(doc, ROOT, items, bottom);
      toast(`${copy ? 'Copied' : 'Moved'} to ${this.ws.title(doc)}`);
    } else {
      const folder = this.ws.parentOf(srcDoc);
      for (const it of items) {
        const title = inlineToPlain(String(it.props.c ?? '')) || 'Untitled';
        const id = this.ws.create('doc', folder, title);
        this.copyInto(id, ROOT, it.children, true);
      }
      toast(items.length === 1 ? 'Created a document from the item' : `Created ${items.length} documents`);
    }
    if (!copy) this.editor.run((tx) => {
      for (const id of ids) tx.move(id, null, null);
      return { sel: { k: 'none' } };
    });
  }

  /** A document dragged from the file pane onto an item becomes a child item of it (the document is deleted). */
  private installDocDrop() {
    const el = this.editor.el;
    const rowOf = (e: DragEvent) => (e.target as HTMLElement).closest<HTMLElement>('.o-node, .o-title');
    el.addEventListener('dragover', (e) => {
      if (!e.dataTransfer?.types.includes(DOC_DRAG_TYPE)) return;
      const row = rowOf(e);
      if (!row) return;
      e.preventDefault();
      el.querySelectorAll('.drop-target').forEach((x) => x.classList.remove('drop-target'));
      row.querySelector(':scope > .o-row')?.classList.add('drop-target');
    });
    el.addEventListener('dragleave', () => el.querySelectorAll('.drop-target').forEach((x) => x.classList.remove('drop-target')));
    el.addEventListener('drop', (e) => {
      const doc = e.dataTransfer?.getData(DOC_DRAG_TYPE);
      el.querySelectorAll('.drop-target').forEach((x) => x.classList.remove('drop-target'));
      const row = rowOf(e);
      if (!doc || !row) return;
      e.preventDefault();
      if (doc === this.route.doc) return toast('A document can’t be moved into itself');
      const target = row.dataset.id!;
      const title = this.ws.title(doc);
      modal('Move document into item', h('p', {}, `Move "${title}" into this item? The document becomes an item here and is removed from the file pane.`), [
        { label: 'Cancel' },
        {
          label: 'Move',
          primary: true,
          run: () => {
            const src = this.sync.doc(doc).live;
            const item = { props: { c: title }, children: src.get(ROOT)!.children.map((c) => toPortable(src, c)) };
            this.editor.run((tx) => {
              const kids = tx.tree.get(target)!.children;
              const [nid] = insertPortable(tx, target, kids.at(-1) ?? null, [item]);
              if (target !== ROOT) tx.collapse(target, false);
              return { sel: { k: 'text', id: nid!, f: 'c', s: 0, e: 0 } };
            });
            this.ws.remove(doc);
          },
        },
      ]);
    });
  }

  private copyDoc(id: string) {
    const src = this.sync.doc(id).live;
    const copy = this.ws.create('doc', this.ws.parentOf(id), `${this.ws.title(id)} (copy)`);
    this.ws.move(copy, this.ws.parentOf(id), this.ws.indexOf(id) + 1);
    this.copyInto(copy, ROOT, src.get(ROOT)!.children.map((c) => toPortable(src, c)), true);
    const s = this.sync.doc(copy);
    const n = src.get(ROOT)!.props.n;
    if (typeof n === 'string' && n) {
      const tx = new Tx(s.live);
      tx.set(ROOT, { n });
      s.commit(tx, { before: null, after: null }, { undoable: false });
    }
    this.go({ doc: copy, zoom: ROOT, q: '' });
    toast('Copy created');
  }

  /** Items whose text has line breaks become one item per line (the first keeps the children). */
  private cleanNewlines(id: string) {
    const s = this.sync.doc(id);
    const tx = new Tx(s.live);
    let n = 0;
    const walk = (p: NodeId) => {
      for (const c of [...(tx.tree.get(p)?.children ?? [])]) {
        walk(c);
        const text = tx.tree.get(c)!.props.c;
        if (typeof text !== 'string' || !text.includes('\n')) continue;
        const lines = text.split('\n').map((l) => l.trim()).filter(Boolean);
        tx.set(c, { c: lines[0] ?? '' });
        let after = c;
        for (const l of lines.slice(1)) after = tx.create(p, after, { c: l });
        n++;
      }
    };
    walk(ROOT);
    if (tx.ops.length) s.commit(tx, { before: null, after: null });
    toast(n ? `Split ${n} multi-line item${n === 1 ? '' : 's'}` : 'No items with line breaks');
  }

  private versionHistory(doc: string) {
    void versionHistory({
      sync: this.sync,
      doc,
      title: this.ws.title(doc),
      session: this.sync.doc(doc),
      exportItem: (tree, id, title) => exportDialog(tree, id, title, this.exportOpts(doc)),
      showAfter: this.ws.pref('show_version_history_after', true),
      setShowAfter: (v) => this.ws.setPref('show_version_history_after', v),
    });
  }

  private exportOpts(doc: string) {
    const v = this.editor.view;
    const searching = doc === this.route.doc && v.filter;
    return {
      showChecked: this.docShowChecked(doc),
      notes: this.docNotes(doc),
      include: searching ? (id: NodeId) => !!v.filter?.has(id) : undefined,
      visibleOnly: this.ws.pref('export_visible_only', false),
      onVisibleOnly: (b: boolean) => this.ws.setPref('export_visible_only', b),
      source: this.itemUrl(doc, ROOT),
      ownerName: this.sync.config ? new URL(this.sync.config.server).host : undefined,
    };
  }

  private exportItems(ids: NodeId[]) {
    const doc = this.route.doc;
    if (!doc) return;
    const t = this.editor.tree;
    const title = ids.length === 1 && ids[0] !== ROOT ? inlineToPlain(String(t.get(ids[0]!)?.props.c ?? '')) || this.ws.title(doc) : this.ws.title(doc);
    exportDialog(t, ids, title, this.exportOpts(doc));
  }

  private importFiles(parent: string | null) {
    importOpmlFile((title, build) => {
      const id = this.ws.create('doc', parent, title);
      const s = this.sync.doc(id);
      const tx = new Tx(s.live);
      build(tx);
      s.commit(tx, { before: null, after: null }, { undoable: false });
      this.go({ doc: id, zoom: ROOT, q: '' });
    });
  }

  private downloadBackup() {
    downloadBlob(backupZip(this.ws, this.sync), `outliner-backup-${new Date().toISOString().slice(0, 10)}.zip`);
  }

  private openBookmark(b: Bookmark) {
    this.go({ doc: b.doc, zoom: b.zoom ?? ROOT, q: b.q ?? '', view: b.global ? 'global' : b.flat ? 'flat' : undefined, sort: (b.sort as SortMode) || undefined });
  }

  private bookmarkCurrent() {
    const r = this.route;
    const doc = r.doc;
    if (r.view === 'global') {
      this.ws.addBookmark({ title: `Search: ${r.q}`, doc: doc ?? '', q: r.q, global: true, sort: r.sort || undefined });
    } else {
      if (!doc) return;
      const z = this.editor.view.zoom;
      const title = z === ROOT ? this.ws.title(doc) : inlineToPlain(String(this.editor.tree.get(z)?.props.c ?? '')) || 'Untitled';
      this.ws.addBookmark({
        title: r.q ? `${title} (${r.q})` : title, doc, zoom: z === ROOT ? undefined : z, q: r.q || undefined,
        flat: r.view === 'flat' || undefined, sort: r.sort || undefined,
      });
    }
    this.sidebar.show('bookmarks');
    toast('Bookmarked');
  }

  private openLink(url: string) {
    const u = parseItemUrl(url);
    if (u && (url.startsWith('#') || url.startsWith(location.origin))) {
      this.go({ doc: u.doc, zoom: u.zoom, q: '' });
      return;
    }
    window.open(url, '_blank', 'noopener');
  }

  // ---- prefs / dialogs -------------------------------------------------------------

  private renderOpts() {
    const fmt = (k: string) => this.ws.pref<string>(k, '') || undefined;
    return {
      hour12: this.ws.pref('time_format', '12h') === '12h',
      markOverdue: this.ws.pref('highlight_overdue', false),
      inlineImages: this.ws.pref('display_images_inline', false),
      dateFormat: fmt('custom_date_display_format'),
      dateFormatNoTime: fmt('custom_date_display_format_no_time'),
      isInternal: (url: string) => url.startsWith('#d=') || url.startsWith(location.origin),
      linkTitle: (url: string) => {
        const u = parseItemUrl(url);
        if (!u || !(url.startsWith('#') || url.startsWith(location.origin)) || !this.ws.get(u.doc)) return undefined;
        if (u.zoom === ROOT) return this.ws.title(u.doc);
        const c = this.sync.doc(u.doc).live.get(u.zoom)?.props.c;
        return typeof c === 'string' && c ? inlineToPlain(c) : undefined;
      },
    };
  }

  /** Per-document view options on top of the global ones. */
  private applyDocPrefs() {
    const doc = this.route.doc;
    const root = document.documentElement;
    const dir = doc ? this.ws.docPref<string>(doc, 'dir', 'global') : 'global';
    const rtl = dir === 'global' ? this.ws.pref('right_to_left', false) : dir === 'rtl';
    this.editor.el.dir = rtl ? 'rtl' : 'ltr';
    root.classList.toggle('rtl', rtl);
    const layout = doc ? this.ws.docPref<string>(doc, 'layout', 'list') : 'list';
    this.editor.el.classList.toggle('o-article', layout === 'article');
    if (!doc) {
      this.mindmap.hide();
      return;
    }
    this.editor.setReadOnly(this.ws.docPref(doc, 'locked', false));
    if (layout === 'mindmap' && !this.route.view) {
      this.mindmap.show({
        tree: this.sync.doc(doc).live, zoom: this.editor.view.zoom, title: this.ws.title(doc), showChecked: this.docShowChecked(doc),
        onBack: () => this.ws.setDocPref(doc, 'layout', null),
        onZoom: (zoom) => this.go({ ...this.route, zoom }),
      });
    } else this.mindmap.hide();
    const showChecked = this.docShowChecked(doc);
    const before = this.editor.prefs.showChecked;
    this.editor.applyPrefs({ showNotes: this.docNotes(doc), showChecked });
    if (before !== showChecked && this.searchInput.value) this.runSearch(false);
  }

  private applyPrefs() {
    const p = <T>(k: string, d: T) => this.ws.pref(k, d);
    const root = document.documentElement;
    let theme = p('theme', 'auto');
    if (theme === 'auto') theme = matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    if (theme === 'dark' && p('use_true_black', false)) theme = 'black';
    root.dataset.theme = theme;
    root.dataset.density = p('density', 'cozy');
    root.dataset.size = p('font_size', 'medium');
    root.dataset.font = p<string>('dyslexia_font', 'none') === 'opendyslexic' ? 'dyslexic' : p('font', 'default');
    root.dataset.eafont = p('eastern_asian_font', 'default');
    root.classList.toggle('tag-bg', p('tag_bg', false));
    root.classList.toggle('doc-border', p('doc_border', false));
    this.customCss.textContent = p('custom_css', '');
    this.docArea.classList.toggle('wide', !p('center_doc', true));
    setRenderOptions({ ...this.renderOpts(), highlight: undefined });
    this.editor.keymap = new Keymap({ overrides: p('shortcut_override', {}), workflowy: p('workflowy_shortcuts', false) });
    this.editor.applyPrefs({
      bulletZoom: p('use_bullet_to_zoom', false),
      unindentInPlace: p('unindent_in_place', false),
      encodeTimezone: p('show_timezone', false),
      spellcheck: p('spellcheck', false),
      highlightCurrent: p('current_highlight', false),
    });
    this.editor.checkOptions = { recurring: p<string>('recurrence_check_behavior', 'new_instance') === 'update' ? 'update' : 'new_instance' };
    const before = this.editor.prefs.showChecked;
    this.applyDocPrefs();
    if (this.route.doc && !this.editor.el.hidden) {
      if (this.searchInput.value && before === this.editor.prefs.showChecked) this.runSearch(false);
      else this.editor.refreshText();
    }
    this.mobile.render();
    if (!this.help.el.hidden) this.help.render();
    this.paintBacklinks();
    this.paintStatus();
    if (this.results.active) this.results.render();
  }

  /** Word count status bar (setting "Show word count"). */
  private paintStatus() {
    const on = this.ws.pref('word_count', false) && !!this.route.doc && !this.route.view;
    this.statusBar.hidden = !on;
    if (!on) return;
    const t = this.editor.tree;
    let items = 0;
    let wItems = 0;
    let wNotes = 0;
    const words = (s: unknown) => (typeof s === 'string' ? (inlineToPlain(s).match(/[\p{L}\p{N}][\p{L}\p{N}'’-]*/gu)?.length ?? 0) : 0);
    const walk = (id: NodeId) => {
      for (const c of t.get(id)?.children ?? []) {
        const n = t.get(c)!;
        items++;
        wItems += words(n.props.c);
        wNotes += words(n.props.n);
        walk(c);
      }
    };
    walk(this.editor.view.zoom);
    this.statusBar.textContent = `${wItems + wNotes} words · ${wItems} in items · ${wNotes} in notes · ${items} items`;
  }

  private settingsDialog() {
    settingsDialog({
      ws: this.ws,
      keymap: () => this.editor.keymap,
      server: () => this.serverSettings(),
      inboxLabel: () => {
        const t = inboxTarget(this.ws, this.sync);
        if (!t) return 'not set';
        const item = t.id === ROOT ? '' : ` › ${inlineToPlain(String(this.sync.doc(t.doc).live.get(t.id)?.props.c ?? '')).slice(0, 40)}`;
        return `${this.ws.title(t.doc)}${item}`;
      },
      pickInbox: () => {
        const targets = this.liveDocs().flatMap((d) => {
          const t = this.sync.doc(d.id).live;
          return [{ label: d.title, sub: 'Document', loc: d.id },
            ...[...t.nodes.values()].filter((n) => n.id !== ROOT && n.props.c && t.isAttached(n.id))
              .map((n) => ({ label: String(n.props.c), sub: `${d.title} › ${breadcrumb(t, n.id)}`, loc: `${d.id}/${n.id}` }))];
        });
        finder('Choose your inbox…', (q) => fuzzyFilter(q, targets, (x) => x.label).slice(0, 50).map(({ item }) => ({
          label: item.label, sub: item.sub, run: () => (this.ws.setPref('inbox_location', item.loc), toast('Inbox set')),
        })));
      },
      downloadBackup: () => this.downloadBackup(),
      importFiles: () => this.importFiles(null),
      serverExtras: () => serverExtras(this.sync, (d) => this.ws.title(d)),
    });
  }

  /** Upload pasted / dropped files to the sync server; one markdown link per file. */
  private uploadFiles(files: File[]): Promise<string[]> | null {
    if (!this.sync.config) {
      toast('Connect a sync server (Settings → Sync & data) to attach files');
      return null;
    }
    const done = toast(files.length > 1 ? `Uploading ${files.length} files…` : `Uploading ${files[0]!.name || 'file'}…`, 120_000);
    return Promise.all(files.map(async (f) => {
      const up = await this.sync.upload(f);
      const url = this.sync.serverUrl(up.url).replace(/\(/g, '%28').replace(/\)/g, '%29');
      const name = escapeInline(up.name);
      return up.type.startsWith('image/') ? `![${name}](${url})` : `[${name}](${url})`;
    })).finally(done).then((links) => (toast(links.length > 1 ? `${links.length} files uploaded` : 'File uploaded'), links), (e: Error) => {
      toast(`Upload failed: ${e.message}`);
      return [];
    });
  }

  private serverSettings(): HTMLElement {
    const c = this.sync.config;
    if (c) {
      return h('div', {}, h('div.field', {}, `Connected to ${c.server}`),
        h('button.btn', { onclick: () => void this.sync.disconnect().then(() => toast('Disconnected')) }, 'Disconnect'));
    }
    const url = h('input', { placeholder: 'https://my-server.example', value: location.protocol.startsWith('http') && location.hostname !== 'localhost' && location.port !== '5173' ? location.origin : '' });
    const pw = h('input', { type: 'password', placeholder: 'Password' });
    const status = h('div', { style: 'font-size:13px;color:var(--muted)' });
    return h('div', {},
      h('label.field', {}, 'Sync server URL', url),
      h('label.field', {}, 'Password', pw),
      h('button.btn.primary', {
        onclick: () => {
          status.textContent = 'Connecting…';
          this.dropUntouchedWelcome();
          this.sync.connect(url.value.trim(), pw.value).then(
            () => {
              status.textContent = 'Connected.';
              this.sidebar.render();
            },
            (e: Error) => (status.textContent = e.message),
          );
        },
      }, 'Connect'),
      status,
    );
  }

  private syncDialog() {
    const st = this.sync.state;
    const rows = [...this.sync.sessions].map(([id, s]) =>
      h('tr', {}, h('td', {}, id === 'workspace' ? 'Workspace (files, settings)' : this.ws.title(id)),
        h('td', {}, this.sync.paused(id) && !s.dirty ? 'Archived (not synced)' : s.dirty ? (st === 'error' || st === 'offline' ? 'Not synced' : 'Pending') : `v${s.version}`)));
    const head = h('div', {},
      h('p', {}, this.sync.config ? `Server: ${this.sync.config.server} — ${this.syncLabel.textContent}` : 'Local only: no sync server connected (Settings → Sync & data).'),
      this.sync.lastError ? h('p.error-text', {}, `Last error: ${this.sync.lastError}`) : null);
    modal('Sync status', h('div', {}, head, h('table.help-table', {}, ...rows)), [
      { label: 'Close' },
      { label: 'Sync now', primary: true, run: () => void this.sync.syncNow() },
    ]);
  }

  private aboutDialog() {
    modal('About Outliner', h('div', {},
      h('p', {}, 'A self-hosted, keyboard-first outliner. Your documents live on this device and on your own sync server.'),
      h('p', {}, 'Its behaviour follows Dynalist, re-implemented from observation (no Dynalist code or assets).'),
      h('p.hint', {}, `Sync: ${this.sync.config?.server ?? 'local only'}`)), [{ label: 'Okay', primary: true }]);
  }

  private createWelcome() {
    const id = this.ws.create('doc', null, 'Welcome');
    const s = this.sync.doc(id);
    const tx = new Tx(s.live);
    let after: string | null = null;
    const add = (parent: string, c: string, props: Record<string, unknown> = {}) => {
      const nid = tx.create(parent, parent === ROOT ? after : tx.tree.get(parent)!.children.at(-1) ?? null, { c, ...props });
      if (parent === ROOT) after = nid;
      return nid;
    };
    const basics = add(ROOT, 'Basics', { h: 2 });
    add(basics, 'Enter for a new item, Tab / Shift+Tab to indent');
    add(basics, 'Ctrl+] zooms in, Ctrl+[ zooms out, Ctrl+. collapses');
    add(basics, 'Shift+Enter adds a note', { n: 'Like this one.' });
    add(basics, 'Ctrl+Enter completes an item', { cb: true });
    const fmt = add(ROOT, 'Formatting', { h: 2 });
    add(fmt, '**bold**, __italic__, ~~strike~~, ==highlight==, `code`');
    add(fmt, 'Tags like #idea and dates like !(2026-12-31)');
    add(ROOT, 'Press Ctrl+/ for all shortcuts');
    s.commit(tx, { before: null, after: null }, { undoable: false });
    localStorage.setItem(WELCOME, JSON.stringify({ id, canonical: s.live.canonical() }));
  }

  /** Before joining a server, drop the local sample doc if the user never touched it. */
  dropUntouchedWelcome() {
    const raw = localStorage.getItem(WELCOME);
    if (!raw) return;
    localStorage.removeItem(WELCOME);
    const { id, canonical } = JSON.parse(raw) as { id: string; canonical: string };
    const s = this.sync.sessions.get(id);
    if (s && s.live.canonical() === canonical && this.ws.get(id)) this.ws.remove(id);
  }
}

// ---- helpers -------------------------------------------------------------------------

const SORTS = new Set(['title-asc', 'title-desc', 'date-new', 'date-old', 'unchecked-first', 'checked-first', 'edited-new', 'edited-old', 'created-new', 'created-old']);

function parseHash(): Route {
  const p = new URLSearchParams(location.hash.slice(1));
  const s = p.get('s') ?? '';
  return {
    doc: p.get('d'),
    zoom: p.get('z') ?? ROOT,
    q: p.get('q') ?? '',
    view: p.get('g') ? 'global' : p.get('f') ? 'flat' : undefined,
    sort: SORTS.has(s) ? (s as SortMode) : undefined,
  };
}

function formatHash(r: Route): string {
  const p = new URLSearchParams();
  if (r.doc) p.set('d', r.doc);
  if (r.doc && r.zoom !== ROOT) p.set('z', r.zoom);
  if (r.q) p.set('q', r.q);
  if (r.view === 'flat') p.set('f', '1');
  if (r.view === 'global') p.set('g', '1');
  if (r.view && r.sort) p.set('s', r.sort);
  const s = p.toString();
  return s ? `#${s}` : '#';
}

const app = new App();
void app.start();

// offline app shell for the browser build (needs a secure context: https or localhost;
// Capacitor bundles its assets itself)
if ('serviceWorker' in navigator && import.meta.env.PROD && !Capacitor.isNativePlatform()) {
  void navigator.serviceWorker.register('./sw.js').catch(() => {});
}
