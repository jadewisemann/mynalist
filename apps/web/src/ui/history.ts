// Version history (docs/03 §7): versions from the server's op log on the left, the outline of the
// selected one on the right (added items green; with "Show version after change" off, the version
// before it with removed items red). Restoring is an ordinary undoable edit.

import { ROOT, Tree, Tx, restoreTree, type DocSession, type NodeId, type SerializedTree } from '@outliner/core';
import type { SyncClient } from '../store/sync.ts';
import { renderText } from '../editor/render.ts';
import { h, toast } from './dom.ts';
import { modal } from './overlay.ts';

interface Point {
  version: number;
  ts: number;
  edits: number;
}

export interface HistoryHost {
  sync: SyncClient;
  doc: string;
  title: string;
  session: DocSession;
  /** export an item of a past version */
  exportItem(tree: Tree, id: NodeId, title: string): void;
  showAfter: boolean;
  setShowAfter(v: boolean): void;
}

const attachedIds = (t: Tree) => {
  const out = new Set<NodeId>();
  const walk = (id: NodeId) => {
    for (const c of t.get(id)?.children ?? []) {
      out.add(c);
      walk(c);
    }
  };
  walk(ROOT);
  return out;
};

export async function versionHistory(host: HistoryHost) {
  if (!host.sync.config) {
    modal('Version history', h('p', {}, 'Version history is kept by the sync server. Connect one in Settings → Sync to use it.'), [{ label: 'Okay', primary: true }]);
    return;
  }
  host.sync.flushSaves();
  await host.sync.syncNow();
  let points: Point[];
  try {
    points = (await host.sync.api<{ points: Point[] }>(`doc/${encodeURIComponent(host.doc)}/history`)).points;
  } catch (e) {
    modal('Version history', h('p', {}, `Could not load the history: ${(e as Error).message}`), [{ label: 'Okay', primary: true }]);
    return;
  }
  const cache = new Map<number, Tree>();
  const load = async (v: number): Promise<Tree> => {
    if (v <= 0) return new Tree();
    let t = cache.get(v);
    if (!t) {
      const r = await host.sync.api<{ tree: SerializedTree }>(`doc/${encodeURIComponent(host.doc)}/at/${v}`);
      t = Tree.fromJSON(r.tree);
      cache.set(v, t);
    }
    return t;
  };
  let selected = 0;
  let showAfter = host.showAfter;
  const list = h('div.hist-list');
  const view = h('div.hist-view');
  const after = h('input', { type: 'checkbox', checked: showAfter, onchange: () => {
    showAfter = after.checked;
    host.setShowAfter(showAfter);
    void paint();
  } });
  const fmt = (ts: number) => new Date(ts).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });

  const paint = async () => {
    list.replaceChildren(...points.map((p, i) => h('div.hist-point', {
      class: i === selected ? 'active' : undefined,
      onclick: () => {
        selected = i;
        void paint();
      },
    }, h('div', {}, fmt(p.ts)), h('div.sub', {}, `${p.edits} change${p.edits === 1 ? '' : 's'} · v${p.version}`))));
    const p = points[selected];
    if (!p) {
      view.textContent = 'No history yet.';
      return;
    }
    view.textContent = 'Loading…';
    let cur: Tree;
    let prev: Tree;
    try {
      // the version before this burst of edits = the end of the previous (older) burst
      [cur, prev] = await Promise.all([load(p.version), load(points[selected + 1]?.version ?? 0)]);
    } catch {
      view.textContent = 'This version is no longer kept on the server.';
      return;
    }
    const shown = showAfter ? cur : prev;
    const other = attachedIds(showAfter ? prev : cur);
    const mark = showAfter ? 'added' : 'removed';
    const render = (id: NodeId): HTMLElement | null => {
      const n = shown.get(id)!;
      const text = h('span.hist-text');
      text.innerHTML = renderText(String(n.props.c ?? '')) || '&nbsp;';
      const changed = !other.has(id) ? mark : '';
      const prevNode = (showAfter ? prev : cur).get(id);
      const edited = !changed && prevNode && (prevNode.props.c !== n.props.c || prevNode.props.n !== n.props.n) ? 'edited' : '';
      return h('li', {},
        h('div.hist-item', {
          class: [changed, edited, n.props.ck ? 'checked' : ''].filter(Boolean).join(' ') || undefined,
          title: 'Export this item',
          onclick: () => host.exportItem(shown, id, String(n.props.c ?? '') || host.title),
        }, text),
        n.children.length ? h('ul', {}, ...n.children.map(render)) : null);
    };
    const titleEl = h('div.hist-title');
    titleEl.innerHTML = renderText(String(shown.get(ROOT)?.props.c ?? '')) || host.title;
    view.replaceChildren(titleEl, h('ul.hist-tree', {}, ...(shown.get(ROOT)?.children ?? []).map(render)));
  };

  const body = h('div.hist', {},
    h('div.hist-cols', {}, list, view),
    h('label.hist-toggle', {}, after, ' Show version after change'));
  modal(`Version history of "${host.title}"`, body, [
    { label: 'Close' },
    {
      label: 'Restore this version',
      primary: true,
      run: () => {
        const p = points[selected];
        const t = p && cache.get(p.version);
        if (!p || !t) return false;
        const target = showAfter ? t : cache.get(points[selected + 1]?.version ?? -1) ?? (points[selected + 1] ? null : new Tree());
        if (!target) return false;
        const tx = new Tx(host.session.live.clone());
        restoreTree(tx, target);
        if (tx.ops.length) host.session.transact(tx.ops, { before: null, after: null });
        toast(`Restored the version of ${fmt(showAfter ? p.ts : points[selected + 1]?.ts ?? p.ts)}. Undo with Ctrl+Z.`);
      },
    },
  ]);
  if (!points.length) view.textContent = 'No history yet.';
  else void paint();
}
