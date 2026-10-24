import { afterEach, describe, expect, it } from 'vitest';
import { mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { backupFiles } from '../src/backup.ts';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DocSession, ROOT, Tx } from '@outliner/core';
import { cleanName } from '../src/extras.ts';
import { NO_SEL, api, launch, syncSession, type TestServer } from './helpers.ts';

let srv: TestServer;
const withFiles = (over: Parameters<typeof launch>[0] = {}) => {
  const d = mkdtempSync(join(tmpdir(), 'outliner-server-'));
  return launch({ filesDir: join(d, 'files'), ...over }, {}, d);
};
afterEach(async () => srv?.dispose());

/** A workspace listing docs D1 (live) and D2 (archived), with dated items in both. */
async function seed(s: TestServer) {
  const ws = new DocSession();
  const tx = new Tx(ws.live);
  tx.create(ROOT, null, { title: 'files' }, 'files');
  tx.create('files', null, { kind: 'doc', title: 'Plans' }, 'D1');
  tx.create('files', 'D1', { kind: 'doc', title: 'Old', archived: true }, 'D2');
  ws.commit(tx, NO_SEL, { undoable: false });
  await syncSession(s, ws, 'workspace');
  for (const [doc, items] of [
    ['D1', [['a', 'Dentist !(2026-10-24 15:30)'], ['b', 'Secret <script>x</script> **bold**'], ['c', 'Done !(2026-10-01)', true]]],
    ['D2', [['z', 'Archived !(2026-10-25)']]],
  ] as const) {
    const d = new DocSession();
    const t = new Tx(d.live);
    let after: string | null = null;
    for (const [id, c, ck] of items as readonly (readonly [string, string, boolean?])[]) {
      t.create(ROOT, after, { c, ...(ck ? { ck: true } : {}) }, id);
      after = id;
    }
    t.create('b', null, { c: 'child', n: 'a note' }, 'b1');
    if (doc !== 'D1') t.move('b1', null, null);
    d.commit(t, NO_SEL, { undoable: false });
    await syncSession(s, d, doc);
  }
}

describe('uploads', () => {
  it('stores a file behind a capability URL, lists and deletes it', async () => {
    srv = await withFiles();
    const body = new TextEncoder().encode('hello file');
    const up = await api(srv, '/api/upload?name=' + encodeURIComponent('../notes 1.txt'), { method: 'POST', body, headers: { 'Content-Type': 'text/plain' } });
    expect(up.status).toBe(200);
    expect(up.data.name).toBe('notes 1.txt');
    expect(up.data.size).toBe(10);
    expect(up.data.url).toMatch(/^\/files\/[\w-]{22}\/notes%201\.txt$/);
    // no token needed to read it
    const got = await fetch(srv.url + up.data.url);
    expect(got.status).toBe(200);
    expect(await got.text()).toBe('hello file');
    expect(got.headers.get('content-security-policy')).toContain('sandbox');
    expect(got.headers.get('content-disposition')).toMatch(/^inline/);
    const html = await api(srv, '/api/upload?name=x.html', { method: 'POST', body: '<script>1</script>', headers: { 'Content-Type': 'text/html' } });
    expect((await fetch(srv.url + html.data.url)).headers.get('content-disposition')).toMatch(/^attachment/);
    // unauthenticated upload is refused
    expect((await api(srv, '/api/upload?name=a', { method: 'POST', body: 'x', token: null })).status).toBe(401);
    const list = await api(srv, '/api/uploads');
    expect(list.data.uploads.map((u: { name: string }) => u.name).sort()).toEqual(['notes 1.txt', 'x.html']);
    expect((await api(srv, `/api/upload/${up.data.key}`, { method: 'DELETE' })).status).toBe(200);
    expect((await fetch(srv.url + up.data.url)).status).toBe(404);
    expect((await api(srv, `/api/upload/${up.data.key}`, { method: 'DELETE' })).status).toBe(404);
    expect((await fetch(srv.url + '/files/nope/x')).status).toBe(404);
  });

  it('rejects files over the limit and reports disabled uploads', async () => {
    srv = await launch({ maxUpload: 1000 });
    expect((await api(srv, '/api/upload?name=a', { method: 'POST', body: 'x' })).status).toBe(403);
    expect((await api(srv, '/api/uploads')).data.enabled).toBe(false);
    await srv.dispose();
    srv = await withFiles({ maxUpload: 1000 });
    expect((await api(srv, '/api/upload?name=a', { method: 'POST', body: 'x'.repeat(2000) })).status).toBe(413);
    expect((await api(srv, '/api/uploads')).data.uploads).toEqual([]);
  });

  it('cleans file names', () => {
    expect(cleanName('C:\\x\\y\\a"b<c>.png')).toBe('abc.png');
    expect(cleanName('...')).toBe('file');
    expect(cleanName('.env')).toBe('env');
  });
});

describe('shares', () => {
  it('renders a read-only page for a doc or a zoomed item, escaped, until revoked', async () => {
    srv = await launch();
    await seed(srv);
    const s = await api(srv, '/api/share', { method: 'POST', json: { doc: 'D1' } });
    expect(s.status).toBe(200);
    const page = await fetch(srv.url + s.data.url);
    expect(page.status).toBe(200);
    const html = await page.text();
    expect(html).toContain('<h1>Plans</h1>');
    expect(html).toContain('<strong>bold</strong>');
    expect(html).not.toContain('<script>');
    expect(html).toContain('Dentist <time>2026-10-24 15:30</time>');
    expect(html).toContain('<details open><summary>');
    expect(html).toContain('<div class="note">a note</div>');
    expect(html).toContain('class="ck"');
    expect(page.headers.get('content-security-policy')).toContain("default-src 'none'");
    expect(await (await fetch(srv.url + s.data.url + '?collapsed')).text()).toContain('<details><summary>');

    const z = await api(srv, '/api/share', { method: 'POST', json: { doc: 'D1', zoom: 'b' } });
    const zh = await (await fetch(srv.url + z.data.url)).text();
    expect(zh).toContain('child');
    expect(zh).not.toContain('Dentist');
    expect(zh).toContain('<div class="meta">Plans</div>');

    expect((await api(srv, '/api/shares')).data.shares.length).toBe(2);
    expect((await api(srv, '/api/share', { method: 'POST', json: { doc: 'workspace' } })).status).toBe(400);
    expect((await api(srv, '/api/share', { method: 'POST', json: { doc: 'D1' }, token: null })).status).toBe(401);
    expect((await api(srv, `/api/share/${s.data.token}`, { method: 'DELETE' })).status).toBe(200);
    expect((await fetch(srv.url + s.data.url)).status).toBe(404);
    expect((await fetch(srv.url + '/s/AAAAAAAAAAAAAAAAAAAAAA')).status).toBe(404);
  });
});

describe('calendar feed', () => {
  it('serves an ICS of dated unchecked items in live docs at a secret URL', async () => {
    srv = await launch();
    await seed(srv);
    const { data } = await api(srv, '/api/calendar');
    expect(data.url).toMatch(/^\/cal\/[\w-]{32}\.ics$/);
    const res = await fetch(srv.url + data.url, { headers: { 'X-Forwarded-Proto': 'https', 'X-Forwarded-Host': 'notes.example' } });
    expect(res.status).toBe(200);
    expect(res.headers.get('content-type')).toContain('text/calendar');
    const ics = await res.text();
    expect(ics.split('BEGIN:VEVENT').length - 1).toBe(1);
    expect(ics).toContain('SUMMARY:Dentist');
    expect(ics).toContain('URL:https://notes.example/#d=D1&z=a');
    expect((await fetch(srv.url + '/cal/' + 'x'.repeat(32) + '.ics')).status).toBe(404);
  });
});

describe('backupFiles', () => {
  it('copies only uploads not yet backed up', () => {
    const d = mkdtempSync(join(tmpdir(), 'outliner-bf-'));
    mkdirSync(join(d, 'files'));
    writeFileSync(join(d, 'files', 'k1'), 'a');
    writeFileSync(join(d, 'files', '.k2.part'), 'partial');
    expect(backupFiles(join(d, 'files'), join(d, 'bk'))).toBe(1);
    writeFileSync(join(d, 'files', 'k3'), 'c');
    expect(backupFiles(join(d, 'files'), join(d, 'bk'))).toBe(1);
    expect(readdirSync(join(d, 'bk', 'files')).sort()).toEqual(['k1', 'k3']);
    rmSync(d, { recursive: true });
  });
});
