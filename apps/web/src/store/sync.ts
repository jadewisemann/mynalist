// Owns every DocSession on this client: local persistence, server sync, push.

import { DocSession, type DocSnapshot, type Op, type SerializedTree } from '@outliner/core';
import { kvGet, kvKeys, kvSet } from './db.ts';

export type SyncState = 'synced' | 'saving' | 'saved-locally' | 'offline' | 'error' | 'local-only';

interface Config {
  server: string;
  token: string;
}

interface SyncDocResult {
  doc: string;
  version?: number;
  remote?: Op[];
  reset?: { version: number; tree: SerializedTree };
}

const DOC_KEY = (id: string) => `doc:${id}`;
const CONFIG_KEY = 'config';

export class SyncClient {
  readonly sessions = new Map<string, DocSession>();
  private saveTimers = new Map<string, number>();
  private syncTimer = 0;
  private backoff = 0;
  private running = false;
  private again = false;
  private ws: WebSocket | null = null;
  private wsPing = 0;
  config: Config | null = null;
  state: SyncState = 'local-only';
  onState: (s: SyncState) => void = () => {};
  /** Called when a document not yet loaded appears on the server. */
  onRemoteDoc: (doc: string) => void = () => {};

  async init(): Promise<void> {
    this.config = (await kvGet<Config>(CONFIG_KEY)) ?? null;
    const keys = await kvKeys();
    for (const k of keys) {
      if (!k.startsWith('doc:')) continue;
      const snap = await kvGet<DocSnapshot>(k);
      if (snap) this.attach(k.slice(4), new DocSession(snap));
    }
    this.setState(this.config ? 'saved-locally' : 'local-only');
    if (this.config) {
      this.connectPush();
      this.scheduleSync(0);
    }
    setInterval(() => this.config && this.scheduleSync(0), 20_000);
    window.addEventListener('online', () => this.scheduleSync(0));
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible') this.scheduleSync(0);
      else this.flushSaves();
    });
  }

  async connect(server: string, password: string): Promise<void> {
    const base = server.replace(/\/+$/, '');
    const res = await fetch(`${base}/api/login`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ password }),
    });
    if (!res.ok) throw new Error(res.status === 401 ? 'Wrong password' : `Login failed (${res.status})`);
    const { token } = (await res.json()) as { token: string };
    this.config = { server: base, token };
    await kvSet(CONFIG_KEY, this.config);
    this.connectPush();
    await this.pullDocList();
    this.scheduleSync(0);
  }

  async disconnect() {
    this.config = null;
    await kvSet(CONFIG_KEY, null);
    this.ws?.close();
    this.setState('local-only');
  }

  /** Get (or create empty) session for a doc id. */
  doc(id: string): DocSession {
    let s = this.sessions.get(id);
    if (!s) {
      s = new DocSession();
      this.attach(id, s);
      this.save(id);
    }
    return s;
  }

  private attach(id: string, s: DocSession) {
    this.sessions.set(id, s);
    s.onChange((kind) => {
      this.save(id);
      if (kind !== 'remote') this.scheduleSync(1000);
    });
  }

  private save(id: string) {
    if (this.saveTimers.has(id)) return;
    this.saveTimers.set(
      id,
      window.setTimeout(() => this.saveNow(id), 300),
    );
  }

  private saveNow(id: string) {
    clearTimeout(this.saveTimers.get(id));
    this.saveTimers.delete(id);
    const s = this.sessions.get(id);
    if (s) void kvSet(DOC_KEY(id), s.snapshot());
  }

  flushSaves() {
    for (const id of [...this.saveTimers.keys()]) this.saveNow(id);
  }

  get dirty(): boolean {
    for (const s of this.sessions.values()) if (s.dirty) return true;
    return false;
  }

  private setState(s: SyncState) {
    if (this.state === s) return;
    this.state = s;
    this.onState(s);
  }

  scheduleSync(delay: number) {
    if (!this.config) {
      this.setState('local-only');
      return;
    }
    if (this.dirty && this.state === 'synced') this.setState('saving');
    clearTimeout(this.syncTimer);
    this.syncTimer = window.setTimeout(() => void this.syncNow(), Math.max(delay, this.backoff));
  }

  private async api<T>(path: string, body?: unknown): Promise<T> {
    const c = this.config!;
    const res = await fetch(`${c.server}/api/${path}`, {
      method: body === undefined ? 'GET' : 'POST',
      headers: { authorization: `Bearer ${c.token}`, 'content-type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (res.status === 401) {
      this.setState('error');
      throw new Error('unauthorized');
    }
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return (await res.json()) as T;
  }

  /** Make sure every document on the server has a local session. */
  async pullDocList() {
    const list = await this.api<{ doc: string; version: number }[]>('docs');
    for (const { doc, version } of list) {
      const s = this.sessions.get(doc);
      if (!s) {
        this.doc(doc);
        this.onRemoteDoc(doc);
      } else if (version > s.version) {
        this.again = true;
      }
    }
  }

  async syncNow(): Promise<void> {
    if (!this.config) return;
    if (this.running) {
      this.again = true;
      return;
    }
    this.running = true;
    const batch: { id: string; s: DocSession; req: { baseVersion: number; ops: Op[] } }[] = [];
    try {
      for (const [id, s] of this.sessions) {
        const req = s.beginSync();
        if (req) batch.push({ id, s, req });
      }
      if (this.dirty) this.setState('saving');
      const res = await this.api<{ docs: SyncDocResult[] }>('sync', {
        docs: batch.map((b) => ({ doc: b.id, baseVersion: b.req.baseVersion, ops: b.req.ops })),
      });
      const byDoc = new Map(res.docs.map((d) => [d.doc, d]));
      for (const b of batch) {
        const r = byDoc.get(b.id);
        if (!r) {
          b.s.failSync();
          continue;
        }
        if (r.reset) {
          b.s.failSync();
          b.s.reset(r.reset.tree, r.reset.version);
        } else {
          b.s.endSync({ version: r.version!, remote: r.remote ?? [] });
        }
        b.s.purge();
        this.save(b.id);
      }
      this.backoff = 0;
      this.setState(this.dirty ? 'saving' : 'synced');
    } catch (e) {
      for (const b of batch) b.s.failSync();
      this.backoff = Math.min(this.backoff ? this.backoff * 2 : 2000, 300_000);
      this.setState(navigator.onLine ? 'error' : 'offline');
      console.warn('sync failed', e);
      this.scheduleSync(this.backoff);
    } finally {
      this.running = false;
    }
    if (this.again || this.dirty) {
      this.again = false;
      this.scheduleSync(this.dirty ? 1000 : 0);
    }
  }

  private connectPush() {
    if (!this.config) return;
    this.ws?.close();
    const url = this.config.server.replace(/^http/, 'ws') + `/api/ws?token=${encodeURIComponent(this.config.token)}`;
    let ws: WebSocket;
    try {
      ws = new WebSocket(url);
    } catch {
      return;
    }
    this.ws = ws;
    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(String(ev.data)) as { doc?: string; version?: number };
        if (!msg.doc) return;
        const s = this.sessions.get(msg.doc);
        if (!s) {
          void this.pullDocList().then(() => this.scheduleSync(0));
        } else if ((msg.version ?? 0) > s.version) {
          this.scheduleSync(0);
        }
      } catch {
        /* ignore */
      }
    };
    ws.onopen = () => {
      clearInterval(this.wsPing);
      this.wsPing = window.setInterval(() => ws.readyState === 1 && ws.send('{"ping":1}'), 30_000);
      this.scheduleSync(0);
    };
    ws.onclose = () => {
      clearInterval(this.wsPing);
      if (this.ws === ws && this.config) setTimeout(() => this.ws === ws && this.connectPush(), 5000);
    };
  }
}
