// HTTP + WebSocket front end for the sync protocol (see docs/ARCHITECTURE.md).

import http from 'node:http';
import type { AddressInfo, Socket } from 'node:net';
import { createHash, timingSafeEqual } from 'node:crypto';
import { createReadStream, statSync } from 'node:fs';
import { extname, join, resolve, sep } from 'node:path';
import { promisify } from 'node:util';
import { gzip as gzipCb } from 'node:zlib';
import { WebSocketServer, WebSocket } from 'ws';
import { Store, type DocVersion, type StoreOptions } from './store.ts';
import { BadRequest, isDocId, parseSyncBody } from './validate.ts';
import { Extras, calendarFeed, calendarKey, isKey, sharePage } from './extras.ts';

const gzip = promisify(gzipCb);

export interface ServerConfig {
  /** bearer token every API call / WS connection must present */
  token: string;
  /** password accepted by POST /api/login; login disabled when unset */
  password?: string;
  /** allowed CORS origins, or '*' */
  corsOrigins: '*' | string[];
  /** directory with the web build to serve at / (optional) */
  staticDir?: string;
  /** max request body in bytes */
  maxBody: number;
  /** where uploaded files are stored; uploads disabled when unset */
  filesDir?: string;
  /** max upload size in bytes */
  maxUpload?: number;
  /** public origin for links in the calendar feed (default: from the request's Host header) */
  publicUrl?: string;
  /** how often to prune the op log (ms); 0 = only at startup */
  pruneIntervalMs: number;
  /** WS heartbeat interval (ms) */
  heartbeatMs: number;
  store: StoreOptions;
}

export interface RunningServer {
  server: http.Server;
  store: Store;
  port: number;
  close(): Promise<void>;
}

class HttpError extends Error {
  readonly status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

const sha = (s: string) => createHash('sha256').update(s, 'utf8').digest();

/** Constant-time string comparison (hashing equalizes lengths). */
export function safeEqual(a: string, b: string): boolean {
  return timingSafeEqual(sha(a), sha(b));
}

const MIME: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.webmanifest': 'application/manifest+json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.txt': 'text/plain; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
  '.wasm': 'application/wasm',
};

export async function startServer(cfg: ServerConfig, port = 0, host = '127.0.0.1'): Promise<RunningServer> {
  const store = new Store(cfg.store);
  const extras = new Extras(store.db, cfg.filesDir);
  const calKey = calendarKey(cfg.token);
  const staticRoot = cfg.staticDir ? resolve(cfg.staticDir) : undefined;

  // ---- auth ---------------------------------------------------------------

  const authorized = (req: http.IncomingMessage, url: URL, allowQuery: boolean): boolean => {
    const h = req.headers.authorization;
    if (h?.startsWith('Bearer ')) return safeEqual(h.slice(7).trim(), cfg.token);
    if (allowQuery) {
      const t = url.searchParams.get('token');
      if (t) return safeEqual(t, cfg.token);
    }
    return false;
  };

  const failures = new Map<string, { n: number; until: number }>();
  const LOGIN_WINDOW = 15 * 60 * 1000;
  const LOGIN_MAX_FAILS = 10;

  // ---- helpers ------------------------------------------------------------

  const cors = (req: http.IncomingMessage, res: http.ServerResponse) => {
    const origin = req.headers.origin;
    if (cfg.corsOrigins === '*') {
      res.setHeader('Access-Control-Allow-Origin', '*');
    } else {
      res.setHeader('Vary', 'Origin');
      if (origin && cfg.corsOrigins.includes(origin)) res.setHeader('Access-Control-Allow-Origin', origin);
    }
  };

  const sendJson = async (req: http.IncomingMessage, res: http.ServerResponse, status: number, body: unknown) => {
    let data: Buffer = Buffer.from(JSON.stringify(body));
    res.setHeader('Content-Type', 'application/json; charset=utf-8');
    res.setHeader('Cache-Control', 'no-store');
    res.setHeader('X-Content-Type-Options', 'nosniff');
    if (data.length > 1024 && /\bgzip\b/.test(String(req.headers['accept-encoding'] ?? ''))) {
      data = await gzip(data);
      res.setHeader('Content-Encoding', 'gzip');
      res.setHeader('Vary', [res.getHeader('Vary'), 'Accept-Encoding'].filter(Boolean).join(', '));
    }
    res.setHeader('Content-Length', data.length);
    res.writeHead(status);
    res.end(data);
  };

  const readJson = (req: http.IncomingMessage): Promise<unknown> =>
    new Promise((resolveBody, reject) => {
      const declared = Number(req.headers['content-length'] ?? 0);
      if (declared > cfg.maxBody) {
        reject(new HttpError(413, 'request body too large'));
        req.resume();
        return;
      }
      const chunks: Buffer[] = [];
      let size = 0;
      let failed = false;
      req.on('data', (c: Buffer) => {
        if (failed) return;
        size += c.length;
        if (size > cfg.maxBody) {
          failed = true;
          reject(new HttpError(413, 'request body too large'));
          return;
        }
        chunks.push(c);
      });
      req.on('end', () => {
        if (failed) return;
        try {
          resolveBody(JSON.parse(Buffer.concat(chunks).toString('utf8')));
        } catch {
          reject(new HttpError(400, 'invalid JSON'));
        }
      });
      req.on('error', (e) => {
        if (!failed) reject(e);
        failed = true;
      });
    });

  // ---- websocket ----------------------------------------------------------

  const wss = new WebSocketServer({ noServer: true, maxPayload: 64 * 1024 });
  const alive = new WeakMap<WebSocket, boolean>();

  wss.on('connection', (ws) => {
    alive.set(ws, true);
    ws.on('pong', () => alive.set(ws, true));
    ws.on('message', (raw) => {
      alive.set(ws, true);
      try {
        const msg = JSON.parse(String(raw)) as { ping?: unknown };
        if (msg && msg.ping !== undefined) ws.send('{"pong":1}');
      } catch {
        // ignore garbage
      }
    });
    ws.on('error', () => ws.terminate());
  });

  const heartbeat = setInterval(() => {
    for (const ws of wss.clients) {
      if (alive.get(ws) === false) {
        ws.terminate();
        continue;
      }
      alive.set(ws, false);
      ws.ping();
    }
  }, cfg.heartbeatMs);
  heartbeat.unref();

  const broadcast = (changed: DocVersion[]) => {
    if (!changed.length) return;
    const msgs = changed.map((c) => JSON.stringify({ doc: c.doc, version: c.version }));
    for (const ws of wss.clients) {
      if (ws.readyState !== WebSocket.OPEN) continue;
      for (const m of msgs) ws.send(m);
    }
  };

  // ---- static -------------------------------------------------------------

  const serveStatic = (req: http.IncomingMessage, res: http.ServerResponse, pathname: string): boolean => {
    if (!staticRoot || (req.method !== 'GET' && req.method !== 'HEAD')) return false;
    let rel: string;
    try {
      rel = decodeURIComponent(pathname);
    } catch {
      throw new HttpError(400, 'bad path');
    }
    if (rel.includes('\0')) throw new HttpError(400, 'bad path');
    let file = resolve(join(staticRoot, rel));
    if (file !== staticRoot && !file.startsWith(staticRoot + sep)) throw new HttpError(403, 'forbidden');
    let st = statSync(file, { throwIfNoEntry: false });
    if (st?.isDirectory()) {
      file = join(file, 'index.html');
      st = statSync(file, { throwIfNoEntry: false });
    }
    if (!st?.isFile()) {
      // SPA fallback for extension-less routes
      if (extname(rel)) return false;
      file = join(staticRoot, 'index.html');
      st = statSync(file, { throwIfNoEntry: false });
      if (!st?.isFile()) return false;
    }
    const ext = extname(file).toLowerCase();
    res.setHeader('Content-Type', MIME[ext] ?? 'application/octet-stream');
    res.setHeader('Content-Length', st.size);
    res.setHeader(
      'Cache-Control',
      rel.startsWith('/assets/') ? 'public, max-age=31536000, immutable' : 'no-cache',
    );
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('Referrer-Policy', 'no-referrer');
    res.setHeader('Content-Security-Policy', "frame-ancestors 'none'");
    res.writeHead(200);
    if (req.method === 'HEAD') res.end();
    else createReadStream(file).on('error', () => res.destroy()).pipe(res);
    return true;
  };

  // ---- routes -------------------------------------------------------------

  const sendPage = (res: http.ServerResponse, body: string, type: string, csp?: string) => {
    const data = Buffer.from(body);
    res.setHeader('Content-Type', type);
    res.setHeader('Content-Length', data.length);
    res.setHeader('Cache-Control', 'no-store');
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('Referrer-Policy', 'no-referrer');
    if (csp) res.setHeader('Content-Security-Policy', csp);
    res.writeHead(200);
    res.end(data);
  };

  const origin = (req: http.IncomingMessage) => {
    if (cfg.publicUrl) return cfg.publicUrl.replace(/\/+$/, '');
    const proto = String(req.headers['x-forwarded-proto'] ?? '').split(',')[0]!.trim() || 'http';
    const host = String(req.headers['x-forwarded-host'] ?? req.headers.host ?? 'localhost').split(',')[0]!.trim();
    return `${/^https?$/.test(proto) ? proto : 'http'}://${host.replace(/[^\w.:[\]-]/g, '')}`;
  };

  const handle = async (req: http.IncomingMessage, res: http.ServerResponse) => {
    const url = new URL(req.url ?? '/', 'http://x');
    const path = url.pathname;
    cors(req, res);
    if (req.method === 'OPTIONS') {
      res.setHeader('Access-Control-Allow-Methods', 'GET, POST, DELETE, OPTIONS');
      res.setHeader('Access-Control-Allow-Headers', 'Authorization, Content-Type');
      res.setHeader('Access-Control-Max-Age', '86400');
      res.writeHead(204);
      res.end();
      return;
    }

    if (!path.startsWith('/api/')) {
      if (req.method === 'GET' || req.method === 'HEAD') {
        // capability URLs: no bearer token, the random key is the secret
        const f = /^\/files\/([^/]+)(?:\/[^/]*)?$/.exec(path);
        if (f) {
          if (extras.serveFile(req, res, f[1]!)) return;
          throw new HttpError(404, 'not found');
        }
        const s = /^\/s\/([^/]+)$/.exec(path);
        if (s) {
          const share = extras.getShare(s[1]!);
          const html = share && sharePage(store, share, { collapsed: url.searchParams.has('collapsed') });
          if (!html) throw new HttpError(404, 'not found');
          return sendPage(res, html, 'text/html; charset=utf-8',
            "default-src 'none'; style-src 'unsafe-inline'; img-src * data:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'");
        }
        const c = /^\/cal\/([^/]+)\.ics$/.exec(path);
        if (c) {
          if (!safeEqual(c[1]!, calKey)) throw new HttpError(404, 'not found');
          return sendPage(res, calendarFeed(store, origin(req)), 'text/calendar; charset=utf-8');
        }
      }
      if (serveStatic(req, res, path)) return;
      throw new HttpError(404, 'not found');
    }

    if (path === '/api/health' && req.method === 'GET') {
      return sendJson(req, res, 200, { ok: true });
    }

    if (path === '/api/login') {
      if (req.method !== 'POST') throw new HttpError(405, 'method not allowed');
      if (!cfg.password) throw new HttpError(403, 'login disabled (OUTLINER_PASSWORD not set)');
      const ip = req.socket.remoteAddress ?? '?';
      const now = Date.now();
      const f = failures.get(ip);
      if (f && f.until < now) failures.delete(ip);
      if (f && f.until >= now && f.n >= LOGIN_MAX_FAILS) throw new HttpError(429, 'too many attempts');
      const body = await readJson(req);
      const pw = (body as { password?: unknown } | null)?.password;
      if (typeof pw !== 'string') throw new HttpError(400, 'expected {password}');
      if (!safeEqual(pw, cfg.password)) {
        const cur = failures.get(ip) ?? { n: 0, until: now + LOGIN_WINDOW };
        cur.n++;
        failures.set(ip, cur);
        throw new HttpError(401, 'wrong password');
      }
      failures.delete(ip);
      return sendJson(req, res, 200, { token: cfg.token });
    }

    if (!authorized(req, url, false)) {
      res.setHeader('WWW-Authenticate', 'Bearer');
      throw new HttpError(401, 'unauthorized');
    }

    if (path === '/api/sync') {
      if (req.method !== 'POST') throw new HttpError(405, 'method not allowed');
      const entries = parseSyncBody(await readJson(req));
      const { results, changed } = store.sync(entries);
      broadcast(changed);
      return sendJson(req, res, 200, { docs: results });
    }

    if (path === '/api/docs') {
      if (req.method !== 'GET') throw new HttpError(405, 'method not allowed');
      return sendJson(req, res, 200, store.listDocs());
    }

    if (path === '/api/backup') {
      if (req.method !== 'GET') throw new HttpError(405, 'method not allowed');
      const docs: Record<string, unknown> = {};
      for (const d of store.listDocs()) docs[d.doc] = store.getDoc(d.doc);
      res.setHeader('Content-Disposition', `attachment; filename="outliner-backup-${new Date().toISOString().slice(0, 10)}.json"`);
      return sendJson(req, res, 200, { format: 'outliner-backup', version: 1, created: Date.now(), docs });
    }

    if (path === '/api/upload') {
      if (req.method !== 'POST') throw new HttpError(405, 'method not allowed');
      if (!cfg.filesDir) throw new HttpError(403, 'uploads disabled');
      const max = cfg.maxUpload ?? 50 * 1024 * 1024;
      if (Number(req.headers['content-length'] ?? 0) > max) {
        req.resume();
        throw new HttpError(413, 'file too large');
      }
      try {
        const up = await extras.receive(req, url.searchParams.get('name') ?? 'file', max);
        return sendJson(req, res, 200, { ...up, url: `/files/${up.key}/${encodeURIComponent(up.name)}` });
      } catch (e) {
        if ((e as Error).message === 'too large') throw new HttpError(413, 'file too large');
        throw e;
      }
    }

    if (path === '/api/uploads') {
      if (req.method !== 'GET') throw new HttpError(405, 'method not allowed');
      return sendJson(req, res, 200, {
        enabled: !!cfg.filesDir,
        max: cfg.maxUpload ?? 50 * 1024 * 1024,
        uploads: extras.uploads().map((u) => ({ ...u, url: `/files/${u.key}/${encodeURIComponent(u.name)}` })),
      });
    }

    const del = /^\/api\/(upload|share)\/([^/]+)$/.exec(path);
    if (del) {
      if (req.method !== 'DELETE') throw new HttpError(405, 'method not allowed');
      if (!isKey(del[2]!)) throw new HttpError(400, 'invalid key');
      const ok = del[1] === 'upload' ? extras.deleteUpload(del[2]!) : extras.deleteShare(del[2]!);
      if (!ok) throw new HttpError(404, 'not found');
      return sendJson(req, res, 200, { ok: true });
    }

    if (path === '/api/share') {
      if (req.method !== 'POST') throw new HttpError(405, 'method not allowed');
      const body = (await readJson(req)) as { doc?: unknown; zoom?: unknown } | null;
      const doc = body?.doc;
      const zoom = body?.zoom ?? null;
      if (typeof doc !== 'string' || !isDocId(doc) || doc === 'workspace') throw new HttpError(400, 'expected {doc, zoom?}');
      if (zoom !== null && (typeof zoom !== 'string' || zoom.length > 64)) throw new HttpError(400, 'invalid zoom');
      const s = extras.share(doc, zoom as string | null);
      return sendJson(req, res, 200, { ...s, url: `/s/${s.token}` });
    }

    if (path === '/api/shares') {
      if (req.method !== 'GET') throw new HttpError(405, 'method not allowed');
      return sendJson(req, res, 200, { shares: extras.shares().map((s) => ({ ...s, url: `/s/${s.token}` })) });
    }

    if (path === '/api/calendar') {
      if (req.method !== 'GET') throw new HttpError(405, 'method not allowed');
      return sendJson(req, res, 200, { url: `/cal/${calKey}.ics` });
    }

    const m = /^\/api\/doc\/([^/]+)(?:\/(history)|\/at\/(\d+))?$/.exec(path);
    if (m) {
      if (req.method !== 'GET') throw new HttpError(405, 'method not allowed');
      let id: string;
      try {
        id = decodeURIComponent(m[1]!);
      } catch {
        throw new HttpError(400, 'invalid doc id');
      }
      if (!isDocId(id)) throw new HttpError(400, 'invalid doc id');
      if (m[2]) return sendJson(req, res, 200, { points: store.history(id) });
      if (m[3]) {
        const tree = store.treeAt(id, Number(m[3]));
        if (!tree) throw new HttpError(404, 'version not retained');
        return sendJson(req, res, 200, { version: Number(m[3]), tree });
      }
      return sendJson(req, res, 200, store.getDoc(id));
    }

    throw new HttpError(404, 'not found');
  };

  const server = http.createServer((req, res) => {
    handle(req, res).catch((err: unknown) => {
      let status = 500;
      let message = 'internal error';
      if (err instanceof HttpError) {
        status = err.status;
        message = err.message;
      } else if (err instanceof BadRequest) {
        status = 400;
        message = err.message;
      } else {
        console.error('[outliner] request failed', req.method, req.url, err);
      }
      if (res.headersSent) {
        res.destroy();
        return;
      }
      if (status === 413) res.setHeader('Connection', 'close');
      sendJson(req, res, status, { error: message }).catch(() => res.destroy());
    });
  });

  server.on('upgrade', (req, socket: Socket, head) => {
    const url = new URL(req.url ?? '/', 'http://x');
    const reject = (status: string) => {
      socket.end(`HTTP/1.1 ${status}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n`);
    };
    if (url.pathname !== '/api/ws') return reject('404 Not Found');
    if (!authorized(req, url, true)) return reject('401 Unauthorized');
    wss.handleUpgrade(req, socket, head, (ws) => wss.emit('connection', ws, req));
  });

  // ---- op log retention ---------------------------------------------------

  const prune = () => {
    try {
      const n = store.prune();
      if (n) console.log(`[outliner] pruned ${n} op log rows`);
    } catch (err) {
      console.error('[outliner] prune failed', err);
    }
  };
  prune();
  const pruneTimer = cfg.pruneIntervalMs > 0 ? setInterval(prune, cfg.pruneIntervalMs) : undefined;
  pruneTimer?.unref();

  await new Promise<void>((ok, fail) => {
    server.once('error', fail);
    server.listen(port, host, () => {
      server.off('error', fail);
      ok();
    });
  });

  return {
    server,
    store,
    port: (server.address() as AddressInfo).port,
    async close() {
      clearInterval(heartbeat);
      if (pruneTimer) clearInterval(pruneTimer);
      for (const ws of wss.clients) ws.terminate();
      wss.close();
      await new Promise<void>((ok) => {
        server.close(() => ok());
        server.closeAllConnections();
      });
      store.close();
    },
  };
}
