import { afterEach, describe, expect, it } from 'vitest';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import WebSocket from 'ws';
import { DocSession, ROOT, Tree, type SerializedTree } from '@outliner/core';
import { NO_SEL, PASSWORD, TOKEN, api, launch, randomEdit, rng, syncSession, type TestServer } from './helpers.ts';

let servers: TestServer[] = [];
afterEach(async () => {
  await Promise.all(servers.map((s) => s.dispose()));
  servers = [];
});
async function start(...args: Parameters<typeof launch>) {
  const s = await launch(...args);
  servers.push(s);
  return s;
}

const serverCanonical = async (srv: TestServer, doc: string) => {
  const r = await api(srv, `/api/doc/${doc}`);
  expect(r.status).toBe(200);
  return { version: r.data.version as number, canonical: Tree.fromJSON(r.data.tree as SerializedTree).canonical() };
};

describe('auth', () => {
  it('rejects requests without or with a wrong token', async () => {
    const srv = await start();
    expect((await api(srv, '/api/docs', { token: null })).status).toBe(401);
    expect((await api(srv, '/api/docs', { token: 'nope' })).status).toBe(401);
    expect((await api(srv, '/api/doc/x', { token: 'nope' })).status).toBe(401);
    const r = await api(srv, '/api/sync', { method: 'POST', token: 'nope', json: { docs: [] } });
    expect(r.status).toBe(401);
    expect((await api(srv, '/api/docs')).status).toBe(200);
  });

  it('logs in with the password only', async () => {
    const srv = await start();
    expect((await api(srv, '/api/login', { method: 'POST', token: null, json: { password: 'wrong' } })).status).toBe(401);
    expect((await api(srv, '/api/login', { method: 'POST', token: null, json: {} })).status).toBe(400);
    const ok = await api(srv, '/api/login', { method: 'POST', token: null, json: { password: PASSWORD } });
    expect(ok.status).toBe(200);
    expect(ok.data).toEqual({ token: TOKEN });
  });

  it('rate-limits repeated login failures', async () => {
    const srv = await start();
    for (let i = 0; i < 10; i++) {
      expect((await api(srv, '/api/login', { method: 'POST', token: null, json: { password: 'x' } })).status).toBe(401);
    }
    expect((await api(srv, '/api/login', { method: 'POST', token: null, json: { password: PASSWORD } })).status).toBe(429);
  });

  it('disables login when no password is configured', async () => {
    const srv = await start({ password: undefined });
    expect((await api(srv, '/api/login', { method: 'POST', token: null, json: { password: '' } })).status).toBe(403);
  });

  it('rejects websocket connections with a bad token', async () => {
    const srv = await start();
    const ws = new WebSocket(`${srv.url.replace('http', 'ws')}/api/ws?token=bad`);
    const status = await new Promise<number>((ok) => {
      ws.on('unexpected-response', (_req, res) => ok(res.statusCode ?? 0));
      ws.on('open', () => ok(101));
      ws.on('error', () => {});
    });
    expect(status).toBe(401);
  });
});

describe('validation', () => {
  it('rejects malformed requests with 400 and keeps serving', async () => {
    const srv = await start();
    const bad: unknown[] = [
      null,
      { docs: 'x' },
      { docs: [{ doc: 'a b', baseVersion: 0, ops: [] }] },
      { docs: [{ doc: 'a', baseVersion: -1, ops: [] }] },
      { docs: [{ doc: 'a', baseVersion: 1.5, ops: [] }] },
      { docs: [{ doc: 'a', baseVersion: 0, ops: {} }] },
      { docs: [{ doc: 'a', baseVersion: 0, ops: [null] }] },
      { docs: [{ doc: 'a', baseVersion: 0, ops: [{ t: 'nuke', id: 'x' }] }] },
      { docs: [{ doc: 'a', baseVersion: 0, ops: [{ t: 'create', id: 'x', parent: 'root', props: {}, ts: 1 }] }] },
      { docs: [{ doc: 'a', baseVersion: 0, ops: [{ t: 'create', id: '', parent: 'root', after: null, props: {}, ts: 1 }] }] },
      { docs: [{ doc: 'a', baseVersion: 0, ops: [{ t: 'set', id: 'x', props: [], ts: 1 }] }] },
      { docs: [{ doc: 'a', baseVersion: 0, ops: [{ t: 'set', id: 'x', props: { c: 'a' }, ts: '1' }] }] },
      { docs: [{ doc: 'a', baseVersion: 0, ops: [{ t: 'move', id: 'x', parent: 5, after: null, ts: 1 }] }] },
      { docs: [{ doc: 'a', baseVersion: 0, ops: [{ t: 'collapse', id: 'x', v: 'yes' }] }] },
      { docs: [{ doc: 'a', baseVersion: 0, ops: [] }, { doc: 'a', baseVersion: 0, ops: [] }] },
    ];
    for (const json of bad) {
      const r = await api(srv, '/api/sync', { method: 'POST', json });
      expect(r.status, JSON.stringify(json)).toBe(400);
      expect(typeof r.data.error).toBe('string');
    }
    const proto = await api(srv, '/api/sync', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: '{"docs":[{"doc":"a","baseVersion":0,"ops":[{"t":"set","id":"root","props":{"__proto__":{"x":1}},"ts":1}]}]}',
    });
    expect(proto.status).toBe(400);
    const notJson = await api(srv, '/api/sync', { method: 'POST', body: '{nope', headers: { 'Content-Type': 'application/json' } });
    expect(notJson.status).toBe(400);
    expect((await api(srv, '/api/doc/bad%20id')).status).toBe(400);
    // nothing was created by the rejected batches
    expect((await api(srv, '/api/docs')).data).toEqual([]);
  });

  it('enforces the request size limit', async () => {
    const srv = await start({ maxBody: 1024 });
    const ops = Array.from({ length: 100 }, (_, i) => ({ t: 'create', id: `n${i}`, parent: 'root', after: null, props: { c: 'x' }, ts: 1 }));
    const r = await api(srv, '/api/sync', { method: 'POST', json: { docs: [{ doc: 'a', baseVersion: 0, ops }] } });
    expect(r.status).toBe(413);
  });

  it('answers CORS preflight and sets allow-origin', async () => {
    const srv = await start({ corsOrigins: ['https://localhost', 'capacitor://localhost'] });
    const pre = await fetch(`${srv.url}/api/sync`, {
      method: 'OPTIONS',
      headers: { Origin: 'capacitor://localhost', 'Access-Control-Request-Method': 'POST' },
    });
    expect(pre.status).toBe(204);
    expect(pre.headers.get('access-control-allow-origin')).toBe('capacitor://localhost');
    expect(pre.headers.get('access-control-allow-headers')).toMatch(/Authorization/);
    const other = await fetch(`${srv.url}/api/docs`, { headers: { Origin: 'https://evil.example', Authorization: `Bearer ${TOKEN}` } });
    expect(other.headers.get('access-control-allow-origin')).toBeNull();
    const star = await start();
    const r = await fetch(`${star.url}/api/docs`, { headers: { Origin: 'http://localhost:5173', Authorization: `Bearer ${TOKEN}` } });
    expect(r.headers.get('access-control-allow-origin')).toBe('*');
  });
});

describe('sync', () => {
  it('creates unknown docs implicitly and lists them', async () => {
    const srv = await start();
    const empty = await api(srv, '/api/doc/nope');
    expect(empty.data.version).toBe(0);
    expect(Tree.fromJSON(empty.data.tree).canonical()).toBe(new Tree().canonical());

    // empty batch against unknown doc does not create it
    let r = await api(srv, '/api/sync', { method: 'POST', json: { docs: [{ doc: 'd1', baseVersion: 0, ops: [] }] } });
    expect(r.data.docs).toEqual([{ doc: 'd1', version: 0, remote: [] }]);
    expect((await api(srv, '/api/docs')).data).toEqual([]);

    const op = { t: 'create', id: 'a', parent: ROOT, after: null, props: { c: 'hello' }, ts: 1 };
    r = await api(srv, '/api/sync', { method: 'POST', json: { docs: [{ doc: 'd1', baseVersion: 0, ops: [op] }, { doc: 'workspace', baseVersion: 0, ops: [op] }] } });
    expect(r.data.docs).toEqual([{ doc: 'd1', version: 1, remote: [] }, { doc: 'workspace', version: 1, remote: [] }]);
    expect((await api(srv, '/api/docs')).data).toEqual([{ doc: 'd1', version: 1 }, { doc: 'workspace', version: 1 }]);

    // ops that Tree.apply drops do not create a version
    r = await api(srv, '/api/sync', { method: 'POST', json: { docs: [{ doc: 'd1', baseVersion: 1, ops: [op, { t: 'set', id: 'zz', props: { c: 1 }, ts: 1 }] }] } });
    expect(r.data.docs).toEqual([{ doc: 'd1', version: 1, remote: [] }]);

    // unknown doc with baseVersion > 0 -> reset to empty
    r = await api(srv, '/api/sync', { method: 'POST', json: { docs: [{ doc: 'ghost', baseVersion: 3, ops: [op] }] } });
    expect(r.data.docs[0].reset.version).toBe(0);
    // client ahead of server -> reset to current state, batch not applied
    r = await api(srv, '/api/sync', { method: 'POST', json: { docs: [{ doc: 'd1', baseVersion: 9, ops: [{ ...op, id: 'b' }] }] } });
    expect(r.data.docs[0].reset.version).toBe(1);
    expect(r.data.docs[0].reset.tree.nodes.map((n: { id: string }) => n.id).sort()).toEqual(['a', 'root']);
  });

  it('two clients with conflicting concurrent edits converge', async () => {
    const srv = await start();
    const doc = 'conflict';
    const A = new DocSession();
    const B = new DocSession();
    const ts = Date.now();
    A.transact([
      { t: 'create', id: 'p', parent: ROOT, after: null, props: { c: 'parent' }, ts },
      { t: 'create', id: 'q', parent: ROOT, after: 'p', props: { c: 'other' }, ts },
      { t: 'create', id: 'x', parent: 'p', after: null, props: { c: 'x' }, ts },
    ], NO_SEL);
    expect(await syncSession(srv, A, doc)).toBe('ok');
    expect(await syncSession(srv, B, doc)).toBe('ok');
    expect(B.live.canonical()).toBe(A.live.canonical());

    // Conflicts: same field edited on both; A deletes p while B adds a child to it;
    // A moves p under q while B moves q under p (cycle); both reorder x.
    A.transact([{ t: 'set', id: 'x', props: { c: 'from A', cb: true }, ts: ts + 1 }], NO_SEL);
    B.transact([{ t: 'set', id: 'x', props: { c: 'from B', h: 2 }, ts: ts + 1 }], NO_SEL);
    A.transact([{ t: 'move', id: 'p', parent: 'q', after: null, ts: ts + 2 }], NO_SEL);
    B.transact([{ t: 'move', id: 'q', parent: 'p', after: null, ts: ts + 2 }], NO_SEL);
    B.transact([{ t: 'create', id: 'y', parent: 'p', after: 'x', props: { c: 'y' }, ts: ts + 3 }], NO_SEL);
    A.transact([{ t: 'move', id: 'p', parent: null, after: null, ts: ts + 3 }], NO_SEL);
    A.transact([{ t: 'create', id: 'z', parent: ROOT, after: null, props: { c: 'z' }, ts: ts + 4 }], NO_SEL);

    // Both send concurrently against the same base version.
    const [ra, rb] = await Promise.all([syncSession(srv, A, doc), syncSession(srv, B, doc)]);
    expect([ra, rb]).toEqual(['ok', 'ok']);
    // Settle: each pulls what the other committed.
    await syncSession(srv, A, doc);
    await syncSession(srv, B, doc);
    const s = await serverCanonical(srv, doc);
    expect(A.live.canonical()).toBe(s.canonical);
    expect(B.live.canonical()).toBe(s.canonical);
    expect(A.version).toBe(s.version);
    expect(B.version).toBe(s.version);
    expect(A.dirty || B.dirty).toBe(false);
  });

  it.each([1234, 7, 42, 2026])('randomized concurrent editing converges (3 clients, seed %i)', async (seed) => {
    const srv = await start({}, { snapshotEvery: 7, cacheSize: 1 });
    const doc = 'fuzz';
    const rand = rng(seed);
    const clients = [new DocSession(), new DocSession(), new DocSession()];
    for (let round = 0; round < 40; round++) {
      for (const [i, c] of clients.entries()) {
        const n = Math.floor(rand() * 4);
        for (let k = 0; k < n; k++) randomEdit(c, rand, `c${i}r${round}-`);
      }
      // a random subset syncs concurrently
      const subset = clients.filter(() => rand() < 0.6);
      await Promise.all(subset.map((c) => syncSession(srv, c, doc)));
      // interleave another doc to exercise LRU eviction with cacheSize 1
      await api(srv, '/api/doc/other');
    }
    for (let i = 0; i < 2; i++) for (const c of clients) await syncSession(srv, c, doc);
    const s = await serverCanonical(srv, doc);
    expect(s.version).toBeGreaterThan(20);
    for (const c of clients) {
      expect(c.dirty).toBe(false);
      expect(c.live.canonical()).toBe(s.canonical);
    }
  });

  it('survives a restart: snapshot + op-log replay reproduces the same tree', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'outliner-restart-'));
    const first = await launch({}, { snapshotEvery: 5 }, dir);
    const A = new DocSession();
    const rand = rng(99);
    for (let i = 0; i < 23; i++) {
      randomEdit(A, rand, 'r');
      await syncSession(first, A, 'persist');
    }
    const before = await serverCanonical(first, 'persist');
    expect(before.canonical).toBe(A.live.canonical());
    await first.close();
    const second = await start({}, { snapshotEvery: 5 }, dir);
    const snaps = second.store.db.prepare('SELECT version FROM snapshots WHERE doc = ? ORDER BY version').all('persist') as { version: number }[];
    expect(snaps.length).toBeGreaterThan(0);
    expect(snaps.length).toBeLessThanOrEqual(2);
    const after = await serverCanonical(second, 'persist');
    expect(after).toEqual(before);
    // and editing continues from there
    randomEdit(A, rand, 'r2');
    expect(await syncSession(second, A, 'persist')).toBe('ok');
    expect((await serverCanonical(second, 'persist')).canonical).toBe(A.live.canonical());
  });

  it('replies reset when baseVersion predates the retained op log; client recovers', async () => {
    const srv = await start({}, { snapshotEvery: 5, retainVersions: 3, retainMs: 0 });
    const doc = 'old';
    const A = new DocSession();
    const B = new DocSession();
    const ts = Date.now();
    A.transact([{ t: 'create', id: 'a', parent: ROOT, after: null, props: { c: 'a' }, ts }], NO_SEL);
    await syncSession(srv, A, doc);
    await syncSession(srv, B, doc); // B at version 1
    for (let i = 0; i < 12; i++) {
      A.transact([{ t: 'set', id: 'a', props: { c: `a${i}` }, ts: ts + i }], NO_SEL);
      await syncSession(srv, A, doc);
    }
    expect(A.version).toBe(13);
    const removed = srv.store.prune();
    expect(removed).toBeGreaterThan(0);
    const oldest = srv.store.db.prepare('SELECT MIN(version) AS v FROM ops WHERE doc = ?').get(doc) as { v: number };
    expect(oldest.v).toBeGreaterThan(2);

    // B edits offline at version 1, then syncs -> reset; its edit is kept and re-sent.
    B.transact([{ t: 'create', id: 'b', parent: ROOT, after: 'a', props: { c: 'from B' }, ts: ts + 100 }], NO_SEL);
    expect(await syncSession(srv, B, doc)).toBe('reset');
    expect(B.version).toBe(13);
    expect(B.live.get('b')?.props.c).toBe('from B');
    expect(B.live.get('a')?.props.c).toBe('a11');
    expect(await syncSession(srv, B, doc)).toBe('ok');
    expect(B.version).toBe(14);
    await syncSession(srv, A, doc);
    const s = await serverCanonical(srv, doc);
    expect(A.live.canonical()).toBe(s.canonical);
    expect(B.live.canonical()).toBe(s.canonical);
    expect(B.dirty).toBe(false);

    // a client at a still-retained version gets incremental remote ops, not a reset
    const r = await api(srv, '/api/sync', { method: 'POST', json: { docs: [{ doc, baseVersion: 12, ops: [] }] } });
    expect(r.data.docs[0].version).toBe(14);
    expect(r.data.docs[0].remote.length).toBe(2);
  });
});

describe('websocket', () => {
  it('pushes {doc, version} after every accepted batch and answers pings', async () => {
    const srv = await start();
    const ws = new WebSocket(`${srv.url.replace('http', 'ws')}/api/ws?token=${TOKEN}`);
    const messages: unknown[] = [];
    let wake: (() => void) | null = null;
    ws.on('message', (m) => {
      messages.push(JSON.parse(String(m)));
      wake?.();
    });
    await new Promise<void>((ok, fail) => {
      ws.on('open', () => ok());
      ws.on('error', fail);
    });
    const waitFor = async (n: number) => {
      while (messages.length < n) await new Promise<void>((ok) => (wake = ok));
    };

    ws.send(JSON.stringify({ ping: 1 }));
    await waitFor(1);
    expect(messages[0]).toEqual({ pong: 1 });

    const A = new DocSession();
    A.transact([{ t: 'create', id: 'a', parent: ROOT, after: null, props: { c: 'a' }, ts: 1 }], NO_SEL);
    await syncSession(srv, A, 'pushed');
    await waitFor(2);
    expect(messages[1]).toEqual({ doc: 'pushed', version: 1 });

    // an empty sync (no new version) pushes nothing; the next accepted batch does
    await syncSession(srv, A, 'pushed');
    A.transact([{ t: 'set', id: 'a', props: { c: 'b' }, ts: 2 }], NO_SEL);
    await syncSession(srv, A, 'pushed');
    await waitFor(3);
    expect(messages).toEqual([{ pong: 1 }, { doc: 'pushed', version: 1 }, { doc: 'pushed', version: 2 }]);
    ws.close();
  });

  it('accepts the token via Authorization header too', async () => {
    const srv = await start();
    const ws = new WebSocket(`${srv.url.replace('http', 'ws')}/api/ws`, { headers: { Authorization: `Bearer ${TOKEN}` } });
    await new Promise<void>((ok, fail) => {
      ws.on('open', () => ok());
      ws.on('error', fail);
    });
    ws.close();
  });
});

describe('static', () => {
  it('serves the web build with SPA fallback and no traversal', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'outliner-static-'));
    mkdirSync(join(dir, 'assets'));
    writeFileSync(join(dir, 'index.html'), '<!doctype html><title>outliner</title>');
    writeFileSync(join(dir, 'assets', 'app-123.js'), 'console.log(1)');
    const srv = await start({ staticDir: dir });
    let r = await fetch(`${srv.url}/`);
    expect(r.status).toBe(200);
    expect(r.headers.get('content-type')).toMatch(/text\/html/);
    expect(await r.text()).toContain('outliner');
    r = await fetch(`${srv.url}/assets/app-123.js`);
    expect(r.headers.get('cache-control')).toMatch(/immutable/);
    expect(await r.text()).toBe('console.log(1)');
    r = await fetch(`${srv.url}/doc/abc`);
    expect(await r.text()).toContain('outliner');
    expect((await fetch(`${srv.url}/missing.js`)).status).toBe(404);
    expect((await fetch(`${srv.url}/..%2f..%2fetc%2fpasswd`)).status).toBe(403);
    expect((await fetch(`${srv.url}/api/nope`, { headers: { Authorization: `Bearer ${TOKEN}` } })).status).toBe(404);
  });
});
