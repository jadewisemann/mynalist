// Server-side extras: public read-only share links, the calendar feed URL and the uploaded-files manager.
// All of them need a sync server; without one they explain that instead.

import type { SyncClient } from '../store/sync.ts';
import { h, toast } from './dom.ts';
import { modal } from './overlay.ts';

interface ShareRow {
  token: string;
  doc: string;
  zoom: string | null;
  created: number;
  url: string;
}

interface UploadRow {
  key: string;
  name: string;
  type: string;
  size: number;
  created: number;
  url: string;
}

const copy = (text: string) => void navigator.clipboard.writeText(text).then(() => toast('Copied to your clipboard'));
const day = (ms: number) => new Date(ms).toLocaleDateString();
export const fmtSize = (n: number) => (n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(1)} KB` : `${(n / 1048576).toFixed(1)} MB`);

const needServer = () => h('p.hint', {}, 'Needs a sync server: connect one in Settings → Sync & data.');

/** "Share…" for a document (zoom null) or one item: create / copy / revoke public read-only links. */
export function shareDialog(sync: SyncClient, doc: string, zoom: string | null, title: string) {
  if (!sync.config) {
    modal('Share', h('div', {}, needServer()), [{ label: 'Close', primary: true }]);
    return;
  }
  const list = h('div.share-list');
  const collapsed = h('input', { type: 'checkbox' });
  const link = (s: ShareRow) => sync.serverUrl(s.url) + (collapsed.checked ? '?collapsed' : '');
  const paint = async () => {
    let rows: ShareRow[] = [];
    try {
      rows = (await sync.api<{ shares: ShareRow[] }>('shares')).shares.filter((s) => s.doc === doc && (s.zoom ?? null) === zoom);
    } catch (e) {
      list.replaceChildren(h('p.hint', {}, `Could not reach the server: ${(e as Error).message}`));
      return;
    }
    list.replaceChildren(...(rows.length ? rows.map((s) => {
      const input = h('input', { value: link(s), readOnly: true, onfocus: () => input.select() });
      return h('div.share-row', {}, input,
        h('button.btn', { onclick: () => copy(input.value) }, 'Copy'),
        h('button.btn', { onclick: () => window.open(input.value, '_blank', 'noopener') }, 'Open'),
        h('button.btn.danger', { onclick: async () => {
          await sync.api(`share/${s.token}`, undefined, 'DELETE');
          toast('Link revoked');
          void paint();
        } }, 'Revoke'),
        h('span.hint', {}, `created ${day(s.created)}`));
    }) : [h('p.hint', {}, 'Not shared yet.')]));
  };
  collapsed.addEventListener('change', () => void paint());
  const body = h('div.share', {},
    h('p', {}, `Anyone with the link can read “${title}” (live, read-only, no sign-in). Revoke the link to stop sharing.`),
    list,
    h('label.field.row', {}, collapsed, 'Links open with every item collapsed'),
    h('button.btn.primary', { onclick: async () => {
      try {
        const s = await sync.api<ShareRow>('share', { doc, zoom });
        await paint();
        copy(link(s));
      } catch (e) {
        toast(`Could not create a link: ${(e as Error).message}`);
      }
    } }, 'Create link'));
  void paint();
  modal('Share', body, [{ label: 'Done', primary: true }]);
}

/** Settings → Sync & data: calendar feed, shared links, uploaded files. */
export function serverExtras(sync: SyncClient, docTitle: (doc: string) => string): HTMLElement[] {
  const sec = (title: string, ...kids: (HTMLElement | string)[]) => h('section.set-sec', {}, h('h3', {}, title), ...kids);
  if (!sync.config) return [sec('Calendar, sharing and files', needServer())];

  const cal = h('div', {}, h('p.hint', {}, 'Loading…'));
  void sync.api<{ url: string }>('calendar').then(({ url }) => {
    const input = h('input', { value: sync.serverUrl(url), readOnly: true, onfocus: () => input.select() });
    cal.replaceChildren(
      h('p.hint', {}, 'Subscribe to this URL in Google Calendar (Other calendars → From URL), Apple Calendar or Outlook: every unchecked item with a date shows up as an event. Keep it private — anyone with the URL can read those items.'),
      h('div.share-row', {}, input, h('button.btn', { onclick: () => copy(input.value) }, 'Copy')));
  }, (e: Error) => cal.replaceChildren(h('p.hint', {}, `Unavailable: ${e.message}`)));

  const shares = h('div.share-list');
  const paintShares = () => void sync.api<{ shares: ShareRow[] }>('shares').then(({ shares: rows }) => {
    shares.replaceChildren(...(rows.length ? rows.map((s) => h('div.share-row', {},
      h('a', { href: sync.serverUrl(s.url), target: '_blank', rel: 'noopener' }, docTitle(s.doc) + (s.zoom ? ' › item' : '')),
      h('span.hint', {}, day(s.created)),
      h('button.btn', { onclick: () => copy(sync.serverUrl(s.url)) }, 'Copy'),
      h('button.btn.danger', { onclick: async () => (await sync.api(`share/${s.token}`, undefined, 'DELETE'), paintShares()) }, 'Revoke'))) : [h('p.hint', {}, 'No shared links. Use “Share…” in a document or item menu.')]));
  }, () => shares.replaceChildren(h('p.hint', {}, 'Unavailable')));
  paintShares();

  const files = h('div.share-list');
  const paintFiles = () => void sync.api<{ enabled: boolean; max: number; uploads: UploadRow[] }>('uploads').then((r) => {
    if (!r.enabled) return files.replaceChildren(h('p.hint', {}, 'Uploads are disabled on this server (set OUTLINER_FILES_DIR).'));
    const total = r.uploads.reduce((a, u) => a + u.size, 0);
    files.replaceChildren(
      h('p.hint', {}, `Paste or drop files into a document to attach them (up to ${fmtSize(r.max)} each). ${r.uploads.length} files, ${fmtSize(total)}.`),
      ...r.uploads.map((u) => h('div.share-row', {},
        h('a', { href: sync.serverUrl(u.url), target: '_blank', rel: 'noopener' }, u.name),
        h('span.hint', {}, `${fmtSize(u.size)} · ${day(u.created)}`),
        h('button.btn', { onclick: () => copy(sync.serverUrl(u.url)) }, 'Copy link'),
        h('button.btn.danger', { onclick: async () => {
          if (!confirm(`Delete “${u.name}”? Links to it stop working.`)) return;
          await sync.api(`upload/${u.key}`, undefined, 'DELETE');
          paintFiles();
        } }, 'Delete'))));
  }, () => files.replaceChildren(h('p.hint', {}, 'Unavailable')));
  paintFiles();

  return [sec('Calendar feed', cal), sec('Shared links', shares), sec('Uploaded files', files)];
}
