import { Capacitor } from '@capacitor/core';
import {
  ROOT, Tx, compileQuery, extractTags, fuzzyFilter, searchTree, type NodeId, type Tree,
} from '@outliner/core';
import { Editor } from './editor/editor.ts';
import { COMMANDS, Keymap, comboOf, prettyCombo } from './editor/keymap.ts';
import { setRenderOptions } from './editor/render.ts';
import { exportDialog, importOpmlFile } from './ui/exporter.ts';
import { SyncClient, type SyncState } from './store/sync.ts';
import { Workspace } from './store/workspace.ts';
import { h, icon, toast } from './ui/dom.ts';
import { finder, modal } from './ui/overlay.ts';
import { Sidebar } from './ui/sidebar.ts';
import { installDrag } from './editor/drag.ts';
import { itemMenu } from './ui/itemmenu.ts';
import { Popups } from './ui/popups.ts';
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
}

const LAST_DOC = 'outliner:lastDoc';
const WELCOME = 'outliner:welcome';

class App {
  sync = new SyncClient();
  ws!: Workspace;
  editor!: Editor;
  sidebar!: Sidebar;
  popups!: Popups;
  route: Route = { doc: null, zoom: ROOT, q: '' };
  private root = document.getElementById('app')!;
  private docArea = h('div.doc-wrap');
  private emptyState = h('div.empty-state');
  private searchInput = h('input', { placeholder: 'Search… (is:completed, #tag, edited:3d)' });
  private searchBox = h('div.search-box', { hidden: true }, icon('search'), this.searchInput,
    h('button.icon-btn', { title: 'Close search', onclick: () => this.closeSearch() }, icon('close')));
  private syncLabel = h('span.sync-status', { onclick: () => this.syncDialog() });

  async start() {
    await this.sync.init();
    this.ws = new Workspace(this.sync);
    this.ws.ensureStructure();
    this.sync.onState = (s) => this.paintSync(s);
    this.sync.onRemoteDoc = () => this.sidebar?.render();
    (window as unknown as { __qa: () => unknown }).__qa = () => qaSnapshot(this.editor);

    this.editor = new Editor({
      onZoom: (zoom) => this.go({ ...this.route, zoom }, true, false),
      onTitleEdit: (title) => this.route.doc && this.ws.syncTitleFromDoc(this.route.doc, title),
      onAppCommand: (id) => this.appCommand(id),
      openLink: (url) => this.openLink(url),
      onTagClick: (tag) => this.toggleSearchTerm(tag),
      onTextInput: (ctx) => this.popups.onTextInput(ctx),
      onDateClick: (id, s, e, el) => this.popups.openDateChip(id, s, e, el),
      onItemMenu: (id, x, y) => itemMenu(this.editor, id, x, y, {
        bookmark: () => {
          const doc = this.route.doc;
          if (doc) this.ws.addBookmark({ title: String(this.editor.tree.get(id)?.props.c ?? '') || 'Untitled', doc, zoom: id });
          toast('Bookmarked');
        },
        exportItem: () => this.route.doc && exportDialog(this.editor.tree, id, String(this.editor.tree.get(id)?.props.c ?? '') || this.ws.title(this.route.doc)),
        setInbox: () => {
          if (this.route.doc) this.ws.setPref('inbox_location', `${this.route.doc}/${id}`);
          toast('Inbox set');
        },
      }),
    });
    this.popups = new Popups(this.editor, {
      tags: (global) => this.tagCounts()[global ? 'all' : 'doc'],
      globalTags: () => this.ws.pref('global_tag_autocomplete', false),
      linkCandidates: () => this.linkCandidates(),
      itemUrl: (doc, id) => formatHash({ doc, zoom: id, q: '' }),
      startDay: () => ({ sat: 6, sun: 0, mon: 1 } as Record<string, number>)[String(this.ws.pref('start_day', 'sun'))] ?? 0,
      hour12: () => this.ws.pref('time_format', '12h') === '12h',
    });
    installDrag(this.editor, { insideAtTop: () => this.ws.pref<string>('move_item_sent_to_position', 'bottom') === 'top' });
    this.sidebar = new Sidebar(this.ws, {
      currentDoc: () => this.route.doc,
      openDoc: (doc, zoom, q) => this.go({ doc, zoom: zoom ?? ROOT, q: q ?? '' }),
      exportDoc: (id) => exportDialog(this.sync.doc(id).live, ROOT, this.ws.title(id)),
      importOpml: (parent) => importOpmlFile((title, build) => {
        const id = this.ws.create('doc', parent, title);
        const s = this.sync.doc(id);
        const tx = new Tx(s.live);
        build(tx);
        s.commit(tx, { before: null, after: null }, { undoable: false });
        this.go({ doc: id, zoom: ROOT, q: '' });
      }),
      tags: () => this.tagCounts(),
      searchTag: (tag, global) => (global ? this.globalSearch(tag) : this.openSearch(tag)),
      bookmarkCurrent: () => this.bookmarkCurrent(),
    });
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
    window.addEventListener('beforeunload', () => this.sync.flushSaves());

    const initial = parseHash();
    if (!initial.doc) initial.doc = localStorage.getItem(LAST_DOC) ?? this.ws.allDocs()[0]?.id ?? null;
    this.applyRoute(initial);
    this.go(initial, false);
  }

  // ---- shell ---------------------------------------------------------------------

  private buildShell() {
    const btn = (name: string, title: string, run: () => void) =>
      h('button.icon-btn', { title, onclick: run }, icon(name));
    const top = h('header.topbar', {},
      btn('side', 'Toggle sidebar', () => this.toggleSidebar()),
      btn('back', 'Back', () => history.back()),
      btn('fwd', 'Forward', () => history.forward()),
      h('span.spacer'),
      this.syncLabel,
      btn('search', 'Search (Ctrl+F)', () => this.openSearch()),
      btn('gear', 'Settings', () => this.settingsDialog()),
      btn('menu', 'Menu', () => this.helpDialog()),
    );
    const tools = h('div.doc-tools', {}, this.searchBox);
    const scroll = h('div.doc-scroll.o-scroll', {}, tools, this.docArea);
    this.docArea.append(this.editor.el, this.emptyState);
    const splitter = h('div.splitter');
    splitter.addEventListener('pointerdown', (e) => {
      splitter.setPointerCapture(e.pointerId);
      const move = (ev: PointerEvent) => document.documentElement.style.setProperty('--side-w', `${Math.max(160, Math.min(520, ev.clientX))}px`);
      splitter.addEventListener('pointermove', move);
      splitter.addEventListener('pointerup', () => splitter.removeEventListener('pointermove', move), { once: true });
    });
    const main = h('div.main', {}, this.sidebar.el, splitter, h('div.scrim', { onclick: () => this.toggleSidebar(false) }), scroll);
    this.root.append(h('div.app', {}, top, main, this.mobileBar()));

    this.searchInput.addEventListener('input', () => this.runSearch(true));
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

  private mobileBar(): HTMLElement {
    const b = (label: string, cmd: string) =>
      h('button', {
        onpointerdown: (e: Event) => e.preventDefault(),
        onclick: () => this.editor.exec(cmd),
      }, label);
    return h('nav.mobile-bar', {},
      b('⇤', 'unindent'), b('⇥', 'indent'), b('↑', 'swap-up'), b('↓', 'swap-down'),
      b('☑', 'toggle-checked'), b('✎', 'goto-note'), b('▸', 'collapse'), b('↶', 'undo'), b('↷', 'redo'),
      b('#', 'insert-hash'), b('◷', 'add-date'), b('⌕', 'search'));
  }

  private toggleSidebar(force?: boolean) {
    const app = this.root.firstElementChild as HTMLElement;
    if (matchMedia('(max-width: 760px)').matches) app.classList.toggle('side-open', force);
    else this.sidebar.el.hidden = force === undefined ? !this.sidebar.el.hidden : !force;
  }

  private paintSync(s: SyncState) {
    const text: Record<SyncState, string> = {
      synced: 'Synced', saving: 'Saving…', 'saved-locally': 'Saved locally', offline: 'Offline',
      error: "Can't sync", 'local-only': 'Local only',
    };
    this.syncLabel.textContent = text[s];
    this.syncLabel.classList.toggle('error', s === 'error');
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
  }

  private applyRoute(r: Route) {
    const docChanged = r.doc !== this.route.doc;
    if (r.doc && !this.ws.get(r.doc)) r = { doc: null, zoom: ROOT, q: '' };
    this.route = r;
    if (!r.doc) {
      this.editor.el.hidden = true;
      this.emptyState.hidden = false;
      this.emptyState.replaceChildren(
        h('div', {}, 'No document is open'),
        h('div', {}, h('a', { onclick: () => this.goAnywhere() }, 'Find a document (Ctrl+O)')),
        h('div', {}, h('a', { onclick: () => this.sidebar.newFile('doc', null) }, 'Create a new document')),
      );
      this.sidebar.render();
      return;
    }
    localStorage.setItem(LAST_DOC, r.doc);
    this.editor.el.hidden = false;
    this.emptyState.hidden = true;
    const session = this.sync.doc(r.doc);
    if (docChanged || this.editor.docSession !== session) {
      this.editor.setDoc(session, r.zoom);
      this.sidebar.render();
    } else if (r.zoom !== this.editor.view.zoom) {
      this.editor.setZoom(r.zoom);
    }
    if (r.q !== this.searchInput.value) {
      this.searchInput.value = r.q;
      this.searchBox.hidden = !r.q;
      this.runSearch(false);
    }
    document.title = `${this.ws.title(r.doc)} — Outliner`;
  }

  // ---- search --------------------------------------------------------------------

  openSearch(q?: string) {
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
    this.runSearch(false);
    this.editor.el.focus();
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
      this.editor.setFilter(null);
      this.editor.refreshText();
    } else {
      const m = compileQuery(q, Date.now(), { typing });
      const res = searchTree(this.editor.tree, m, this.editor.view.zoom, { showChecked: this.editor.view.showChecked });
      setRenderOptions({ ...this.renderOpts(), highlight: m.terms });
      this.editor.setFilter(res.visible);
      this.editor.refreshText();
    }
    if (!typing && q !== this.route.q) this.go({ ...this.route, q }, false, false);
  }

  private flatSearch(q: string) {
    const doc = this.route.doc;
    if (!doc) return;
    const t = this.editor.tree;
    const m = compileQuery(q, Date.now());
    const res = searchTree(t, m, this.editor.view.zoom, { showChecked: this.editor.view.showChecked });
    finder(`Found ${res.matches.length} matches — type to narrow`, (sub) =>
      fuzzyFilter(sub, res.matches, (id) => String(t.get(id)?.props.c ?? '')).map(({ item }) => ({
        label: String(t.get(item)?.props.c ?? '') || '(empty)',
        sub: breadcrumb(t, item),
        run: () => this.revealItem(doc, item),
      })),
    );
  }

  private globalSearch(q: string) {
    const m = compileQuery(q, Date.now());
    const hits: { doc: string; id: NodeId; t: Tree }[] = [];
    for (const d of this.ws.allDocs()) {
      const t = this.sync.doc(d.id).live;
      for (const id of searchTree(t, m, ROOT, { showChecked: true }).matches) hits.push({ doc: d.id, id, t });
    }
    finder(`${hits.length} matches for "${q}" in all documents`, (sub) =>
      fuzzyFilter(sub, hits, (x) => String(x.t.get(x.id)?.props.c ?? '')).map(({ item: x }) => ({
        label: String(x.t.get(x.id)?.props.c ?? ''),
        sub: `${this.ws.title(x.doc)} › ${breadcrumb(x.t, x.id)}`,
        run: () => this.revealItem(x.doc, x.id),
      })),
    );
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
    const docs = [...this.ws.allDocs()];
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
    const all = count(this.ws.allDocs().map((d) => this.sync.doc(d.id).live));
    return { doc, all };
  }

  // ---- commands --------------------------------------------------------------------

  private onGlobalKey(e: KeyboardEvent) {
    if (e.defaultPrevented || e.isComposing) return;
    const combo = comboOf(e);
    const cmd = this.editor.keymap.resolve(combo);
    if (!cmd) return;
    const inEditor = this.editor.el.contains(document.activeElement);
    // editor commands from outside the editor (e.g. nothing focused)
    if (!inEditor && !['help', 'settings', 'toggle-file-pane', 'toggle-bookmark-pane', 'save', 'search', 'go-anywhere', 'goto-item', 'back', 'forward', 'undo', 'redo', 'bookmark'].includes(cmd)) return;
    if (document.activeElement instanceof HTMLInputElement && !['go-anywhere', 'goto-item', 'help'].includes(cmd)) return;
    if (this.editor.exec(cmd)) e.preventDefault();
  }

  appCommand(id: string): boolean {
    switch (id) {
      case 'help':
        this.helpDialog();
        return true;
      case 'settings':
        this.settingsDialog();
        return true;
      case 'toggle-file-pane':
        this.sidebar.show('files');
        this.toggleSidebar();
        return true;
      case 'toggle-bookmark-pane':
        this.sidebar.show('bookmarks');
        return true;
      case 'toggle-tag-pane':
        this.sidebar.show('tags');
        return true;
      case 'save':
        this.sync.flushSaves();
        void this.sync.syncNow();
        return true;
      case 'search':
        this.openSearch();
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
      case 'copy-link': {
        const id = this.editor.targets()[0];
        if (id && this.route.doc) {
          void navigator.clipboard.writeText(`${location.origin}${location.pathname}${formatHash({ doc: this.route.doc, zoom: id, q: '' })}`);
          toast('Link copied to your clipboard!');
        }
        return true;
      }
      case 'add-date':
        return this.popups.addDate();
      case 'insert-hash':
        document.execCommand('insertText', false, '#');
        return true;
      case 'move-to':
        this.moveToDialog();
        return true;
      case 'export':
        if (this.route.doc) exportDialog(this.editor.tree, this.editor.targets()[0] ?? this.editor.view.zoom, this.ws.title(this.route.doc));
        return true;
    }
    return false;
  }

  private goAnywhere() {
    finder('Search any document or bookmark to jump to…', (q) => {
      const docs = this.ws.allDocs().map((d) => ({ label: d.title, sub: d.path, run: () => this.go({ doc: d.id, zoom: ROOT, q: '' }) }));
      const bms = this.ws.bookmarks().map((b) => ({ label: `★ ${b.title}`, sub: 'Bookmark', run: () => this.go({ doc: b.doc, zoom: b.zoom ?? ROOT, q: b.q ?? '' }) }));
      return fuzzyFilter(q, [...docs, ...bms], (x) => x.label).map((x) => x.item);
    });
  }

  private itemFinder() {
    const all: { doc: string; id: NodeId; t: Tree; text: string }[] = [];
    for (const d of this.ws.allDocs()) {
      const t = this.sync.doc(d.id).live;
      for (const n of t.nodes.values()) {
        if (n.id === ROOT || !t.isAttached(n.id)) continue;
        const text = String(n.props.c ?? '');
        if (text) all.push({ doc: d.id, id: n.id, t, text });
      }
    }
    finder('Jump to any item…', (q) =>
      (q ? fuzzyFilter(q, all, (x) => x.text).map((m) => m.item) : all.slice(0, 50)).slice(0, 50).map((x) => ({
        label: x.text,
        sub: `${this.ws.title(x.doc)} › ${breadcrumb(x.t, x.id)}`,
        run: () => this.revealItem(x.doc, x.id),
      })),
    );
  }

  private moveToDialog() {
    const ids = this.editor.targets().filter((id) => id !== this.editor.view.zoom);
    const srcDoc = this.route.doc;
    if (!ids.length || !srcDoc) return;
    const src = this.sync.doc(srcDoc);
    const targets: { doc: string; id: NodeId; label: string; sub: string }[] = [];
    for (const d of this.ws.allDocs()) {
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
            const dst = this.sync.doc(x.doc);
            const items = ids.map((id) => toPortableDeep(src.live, id));
            const tx = new Tx(dst.live);
            const kids = dst.live.get(x.id)!.children;
            insertItems(tx, x.id, bottom ? kids[kids.length - 1] ?? null : null, items);
            dst.commit(tx, { before: null, after: null });
            this.editor.exec('delete-left');
          }
          toast(`Moved to ${x.label}`);
        },
      })),
    );
  }

  private bookmarkCurrent() {
    const doc = this.route.doc;
    if (!doc) return;
    const z = this.editor.view.zoom;
    const title = z === ROOT ? this.ws.title(doc) : String(this.editor.tree.get(z)?.props.c ?? '') || 'Untitled';
    this.ws.addBookmark({ title: this.route.q ? `${title} (${this.route.q})` : title, doc, zoom: z === ROOT ? undefined : z, q: this.route.q || undefined });
    this.sidebar.show('bookmarks');
    toast('Bookmarked');
  }

  private openLink(url: string) {
    const m = /#d=([^&]+)(?:&z=([^&]+))?/.exec(url);
    if (m && (url.startsWith('#') || url.startsWith(location.origin))) {
      this.go({ doc: decodeURIComponent(m[1]!), zoom: m[2] ? decodeURIComponent(m[2]) : ROOT, q: '' });
      return;
    }
    window.open(url, '_blank', 'noopener');
  }

  // ---- prefs / dialogs -------------------------------------------------------------

  private renderOpts() {
    return {
      hour12: this.ws.pref('time_format', '12h') === '12h',
      markOverdue: this.ws.pref('highlight_overdue', false),
      inlineImages: this.ws.pref('display_images_inline', false),
      isInternal: (url: string) => url.startsWith('#d=') || url.startsWith(location.origin),
    };
  }

  private applyPrefs() {
    const p = <T>(k: string, d: T) => this.ws.pref(k, d);
    const root = document.documentElement;
    let theme = p('theme', 'auto');
    if (theme === 'auto') theme = matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    root.dataset.theme = theme;
    root.dataset.density = p('density', 'cozy');
    root.dataset.size = p('font_size', 'medium');
    this.docArea.classList.toggle('wide', !p('center_doc', true));
    setRenderOptions({ ...this.renderOpts(), highlight: undefined });
    this.editor.keymap = new Keymap({ workflowy: p('workflowy_shortcuts', false) });
    const showChecked = p('show_checked', true);
    const before = this.editor.prefs;
    this.editor.applyPrefs({
      bulletZoom: p('use_bullet_to_zoom', false),
      showNotes: p('show_notes', 'show'),
      showChecked,
    });
    this.editor.checkOptions = { recurring: p<string>('recurrence_check_behavior', 'new_instance') === 'update' ? 'update' : 'new_instance' };
    if (before.showChecked !== showChecked || this.searchInput.value) this.runSearch(false);
  }

  private settingsDialog() {
    const sel = (key: string, def: string, opts: [string, string][]) => {
      const s = h('select', { onchange: () => this.ws.setPref(key, s.value) },
        ...opts.map(([v, l]) => h('option', { value: v, selected: this.ws.pref(key, def) === v }, l)));
      return s;
    };
    const chk = (key: string, def: boolean) => {
      const c = h('input', { type: 'checkbox', checked: this.ws.pref(key, def), onchange: () => this.ws.setPref(key, c.checked) });
      return c;
    };
    const row = (label: string, input: HTMLElement) =>
      input instanceof HTMLInputElement && input.type === 'checkbox'
        ? h('label.field', { style: 'flex-direction:row;align-items:center;gap:8px' }, input, label)
        : h('label.field', {}, label, input);
    const body = h('div', {},
      row('Theme', sel('theme', 'auto', [['auto', 'Auto'], ['light', 'Light'], ['dark', 'Dark'], ['black', 'True black'], ['sepia', 'Sepia']])),
      row('Density', sel('density', 'cozy', [['compact', 'Compact'], ['cozy', 'Cozy'], ['comfortable', 'Comfortable']])),
      row('Font size', sel('font_size', 'medium', [['small', 'Small'], ['medium', 'Medium'], ['large', 'Large']])),
      row('Notes', sel('show_notes', 'show', [['show', 'Show'], ['teaser', 'First line'], ['hide', 'Hide']])),
      row('Time format', sel('time_format', '12h', [['12h', '12-hour'], ['24h', '24-hour']])),
      row('First day of week', sel('start_day', 'sun', [['sat', 'Saturday'], ['sun', 'Sunday'], ['mon', 'Monday']])),
      row('Recurring date', sel('recurrence_check_behavior', 'new_instance', [['new_instance', 'Next date appears in new item'], ['update', 'Existing date gets updated']])),
      row('Auto-complete tags from all docs', chk('global_tag_autocomplete', false)),
      row('Show checked items', chk('show_checked', true)),
      row('Narrow document layout', chk('center_doc', true)),
      row('Use bullet to zoom in', chk('use_bullet_to_zoom', false)),
      row('Highlight overdue dates', chk('highlight_overdue', false)),
      row('Display images inline', chk('display_images_inline', false)),
      row('WorkFlowy-style shortcuts', chk('workflowy_shortcuts', false)),
      h('hr'),
      this.serverSettings(),
    );
    modal('Settings', body, [{ label: 'Done', primary: true }]);
  }

  private serverSettings(): HTMLElement {
    const c = this.sync.config;
    if (c) {
      return h('div', {}, h('div.field', {}, `Sync server: ${c.server}`),
        h('button.btn', { onclick: () => void this.sync.disconnect().then(() => toast('Disconnected')) }, 'Disconnect'));
    }
    const url = h('input', { placeholder: 'http://100.x.y.z:8080', value: location.protocol.startsWith('http') && location.hostname !== 'localhost' && location.port !== '5173' ? location.origin : '' });
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
    const rows = [...this.sync.sessions].map(([id, s]) =>
      h('tr', {}, h('td', {}, id === 'workspace' ? 'Workspace' : this.ws.title(id)), h('td', {}, s.dirty ? 'Pending' : `v${s.version}`)));
    modal('Sync status', h('table.help-table', {}, ...rows), [{ label: 'Sync now', primary: true, run: () => void this.sync.syncNow() }]);
  }

  private helpDialog() {
    const groups = new Map<string, HTMLElement[]>();
    for (const c of COMMANDS) {
      const keys = this.editor.keymap.keysFor(c.id);
      if (!keys.length) continue;
      const list = groups.get(c.group) ?? [];
      list.push(h('tr', {}, h('td', {}, c.label), h('td', {}, keys.map(prettyCombo).join(' / '))));
      groups.set(c.group, list);
    }
    const fmt: [string, string][] = [
      ['**bold**', 'Bold'], ['__italic__', 'Italic'], ['~~strike~~', 'Strikethrough'], ['==highlight==', 'Highlight'],
      ['`code`', 'Inline code'], ['[text](url)', 'Link'], ['![alt](url)', 'Image'], ['$$E=mc^2$$', 'LaTeX'],
      ['#tag @tag', 'Tags'], ['!(2026-01-31 14:00)', 'Date'],
    ];
    const body = h('div', {},
      h('h3', {}, 'Formatting'),
      h('table.help-table', {}, ...fmt.map(([a, b]) => h('tr', {}, h('td', {}, b), h('td', {}, a)))),
      ...[...groups].flatMap(([g, rows]) => [h('h3', {}, g), h('table.help-table', {}, ...rows)]),
    );
    modal('Shortcuts & formatting', body, [{ label: 'Close', primary: true }]);
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

function parseHash(): Route {
  const p = new URLSearchParams(location.hash.slice(1));
  return { doc: p.get('d'), zoom: p.get('z') ?? ROOT, q: p.get('q') ?? '' };
}

function formatHash(r: Route): string {
  if (!r.doc) return '#';
  const p = new URLSearchParams({ d: r.doc });
  if (r.zoom !== ROOT) p.set('z', r.zoom);
  if (r.q) p.set('q', r.q);
  return '#' + p.toString();
}

function breadcrumb(t: Tree, id: NodeId): string {
  return t
    .ancestors(id)
    .filter((a) => a !== ROOT)
    .map((a) => String(t.get(a)?.props.c ?? '').slice(0, 40))
    .join(' › ');
}

interface Item {
  props: Record<string, unknown>;
  collapsed?: boolean;
  children: Item[];
}

function toPortableDeep(t: Tree, id: NodeId): Item {
  const n = t.get(id)!;
  return { props: { ...n.props }, collapsed: n.collapsed, children: n.children.map((c) => toPortableDeep(t, c)) };
}

function insertItems(tx: Tx, parent: NodeId, after: NodeId | null, items: Item[]) {
  let anchor = after;
  for (const it of items) {
    const id = tx.create(parent, anchor, it.props);
    insertItems(tx, id, null, it.children);
    if (it.collapsed && it.children.length) tx.collapse(id, true);
    anchor = id;
  }
}

void new App().start();

// offline app shell for the browser build (needs a secure context: https or localhost;
// Capacitor bundles its assets itself)
if ('serviceWorker' in navigator && import.meta.env.PROD && !Capacitor.isNativePlatform()) {
  void navigator.serviceWorker.register('./sw.js').catch(() => {});
}
