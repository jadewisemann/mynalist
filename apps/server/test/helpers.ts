import { randomBytes } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DocSession, ROOT, type Op, type SerializedTree } from '@outliner/core';
import { startServer, type RunningServer, type ServerConfig } from '../src/server.ts';

// generated per run: no credential-looking literals in the repo
export const TOKEN = randomBytes(16).toString('hex');
export const PASSWORD = randomBytes(12).toString('base64url');

export interface TestServer extends RunningServer {
  url: string;
  dir: string;
  cfg: ServerConfig;
  /** stop and delete the temp dir */
  dispose(): Promise<void>;
}

export function testConfig(dir: string, over: Partial<ServerConfig> = {}, store: Partial<ServerConfig['store']> = {}): ServerConfig {
  return {
    token: TOKEN,
    password: PASSWORD,
    corsOrigins: '*',
    maxBody: 256 * 1024,
    pruneIntervalMs: 0,
    heartbeatMs: 30_000,
    ...over,
    store: {
      path: join(dir, 'test.db'),
      snapshotEvery: 200,
      retainVersions: 10_000,
      retainMs: 30 * 24 * 3600 * 1000,
      purgeDetachedMs: 0,
      cacheSize: 16,
      ...store,
    },
  };
}

export async function launch(over: Partial<ServerConfig> = {}, store: Partial<ServerConfig['store']> = {}, dir?: string): Promise<TestServer> {
  const d = dir ?? mkdtempSync(join(tmpdir(), 'outliner-server-'));
  const cfg = testConfig(d, over, store);
  const srv = await startServer(cfg, 0, '127.0.0.1');
  return {
    ...srv,
    url: `http://127.0.0.1:${srv.port}`,
    dir: d,
    cfg,
    async dispose() {
      await srv.close();
      rmSync(d, { recursive: true, force: true });
    },
  };
}

export async function api(srv: { url: string }, path: string, init: RequestInit & { json?: unknown; token?: string | null } = {}) {
  const headers = new Headers(init.headers);
  const token = init.token === undefined ? TOKEN : init.token;
  if (token) headers.set('Authorization', `Bearer ${token}`);
  let body = init.body;
  if (init.json !== undefined) {
    headers.set('Content-Type', 'application/json');
    body = JSON.stringify(init.json);
  }
  const res = await fetch(srv.url + path, { ...init, headers, body });
  const text = await res.text();
  let data: any = undefined;
  try {
    data = text ? JSON.parse(text) : undefined;
  } catch {
    data = text;
  }
  return { status: res.status, data, headers: res.headers };
}

/** One sync round for one DocSession, exactly as a real client would do it. */
export async function syncSession(srv: { url: string }, s: DocSession, doc: string): Promise<'ok' | 'reset' | 'busy'> {
  const req = s.beginSync();
  if (!req) return 'busy';
  let r;
  try {
    r = await api(srv, '/api/sync', { method: 'POST', json: { docs: [{ doc, ...req }] } });
  } catch (err) {
    s.failSync();
    throw err;
  }
  if (r.status !== 200) {
    s.failSync();
    throw new Error(`sync failed: ${r.status} ${JSON.stringify(r.data)}`);
  }
  const res = r.data.docs[0];
  if (res.reset) {
    s.reset(res.reset.tree as SerializedTree, res.reset.version);
    return 'reset';
  }
  s.endSync({ version: res.version, remote: res.remote as Op[] });
  return 'ok';
}

export const NO_SEL = { before: null, after: null };

/** Small deterministic PRNG (mulberry32). */
export function rng(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

let idCounter = 0;

/** Perform one random edit on a session (create / edit / move / delete / undo / collapse). */
export function randomEdit(s: DocSession, rand: () => number, tag: string) {
  const t = s.live;
  const attached = [...t.nodes.values()].filter((n) => t.isAttached(n.id));
  const pick = () => attached[Math.floor(rand() * attached.length)]!;
  const items = attached.filter((n) => n.id !== ROOT);
  const ts = Date.now();
  const r = rand();
  if (r < 0.35 || items.length < 3) {
    const parent = pick();
    const after = parent.children.length && rand() < 0.7 ? parent.children[Math.floor(rand() * parent.children.length)]! : null;
    s.transact([{ t: 'create', id: `${tag}${++idCounter}`, parent: parent.id, after, props: { c: `${tag} ${idCounter}` }, ts }], NO_SEL);
  } else if (r < 0.55) {
    const n = items[Math.floor(rand() * items.length)]!;
    const props: Record<string, unknown> = { c: `${tag} edit ${Math.floor(rand() * 1000)}` };
    if (rand() < 0.3) props.ck = rand() < 0.5;
    s.transact([{ t: 'set', id: n.id, props, ts }], NO_SEL);
  } else if (r < 0.75) {
    const n = items[Math.floor(rand() * items.length)]!;
    const parent = pick();
    const after = parent.children.length && rand() < 0.5 ? parent.children[Math.floor(rand() * parent.children.length)]! : null;
    s.transact([{ t: 'move', id: n.id, parent: parent.id, after, ts }], NO_SEL);
  } else if (r < 0.85) {
    const n = items[Math.floor(rand() * items.length)]!;
    s.transact([{ t: 'move', id: n.id, parent: null, after: null, ts }], NO_SEL);
  } else if (r < 0.95) {
    s.undo();
  } else {
    const n = items[Math.floor(rand() * items.length)]!;
    s.transact([{ t: 'collapse', id: n.id, v: !n.collapsed }], NO_SEL);
  }
}
