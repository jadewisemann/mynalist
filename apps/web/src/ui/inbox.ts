// Inbox (docs/01 §6.6, docs/03 §7): `inbox_location` = "<doc>" or "<doc>/<item>"; capture adds items at its
// start or end (`inbox_move_position`).

import { ROOT, Tx, insertPortable, parsePlainText, type NodeId } from '@outliner/core';
import type { SyncClient } from '../store/sync.ts';
import type { Workspace } from '../store/workspace.ts';
import { h, toast } from './dom.ts';
import { modal } from './overlay.ts';

export function inboxTarget(ws: Workspace, sync: SyncClient): { doc: string; id: NodeId } | null {
  const loc = ws.pref<string>('inbox_location', '');
  if (!loc) return null;
  const [doc, id = ROOT] = loc.split('/');
  if (!doc || !ws.get(doc)) return null;
  const t = sync.doc(doc).live;
  return t.get(id) && t.isAttached(id) ? { doc, id } : null;
}

const NO_INBOX = 'Set an inbox first: open an item’s menu and choose "Set as inbox".';

/** Add `text` (one item per line, indentation kept) to the inbox. */
export function addToInbox(ws: Workspace, sync: SyncClient, text: string): boolean {
  const target = inboxTarget(ws, sync);
  if (!target) {
    toast(NO_INBOX, 4000);
    return false;
  }
  const items = parsePlainText(text);
  if (!items.length) return false;
  const s = sync.doc(target.doc);
  const tx = new Tx(s.live);
  const kids = s.live.get(target.id)!.children;
  const top = ws.pref<string>('inbox_move_position', 'bottom') === 'top';
  insertPortable(tx, target.id, top ? null : kids[kids.length - 1] ?? null, items);
  s.commit(tx, { before: null, after: null });
  return true;
}

export function captureDialog(ws: Workspace, sync: SyncClient) {
  if (!inboxTarget(ws, sync)) {
    toast(NO_INBOX, 4000);
    return;
  }
  const area = h('textarea.capture', { placeholder: 'Capture into my inbox…' });
  const add = () => {
    if (!area.value.trim()) return;
    if (addToInbox(ws, sync, area.value)) toast('Successfully captured to your inbox.');
  };
  area.addEventListener('keydown', (e) => {
    // Ctrl/Cmd+Enter adds (Enter alone makes a new line = another item)
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      add();
      close();
    }
  });
  const close = modal('', h('div.capture-wrap', {}, area), [
    { label: 'Cancel' },
    { label: 'Add', primary: true, run: () => add() },
  ]);
}

/** The inbox location as a route. */
export function inboxRoute(ws: Workspace, sync: SyncClient): { doc: string; zoom: NodeId } | null {
  const t = inboxTarget(ws, sync);
  if (!t) {
    toast(NO_INBOX, 4000);
    return null;
  }
  return { doc: t.doc, zoom: t.id };
}
