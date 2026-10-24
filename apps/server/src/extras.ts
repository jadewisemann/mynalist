// Uploaded files, public share links and the calendar feed: the parts of the server that are reachable
// without the bearer token (capability URLs), plus the authenticated endpoints that manage them.

import type http from 'node:http';
import { createHmac, randomBytes } from 'node:crypto';
import { createReadStream, createWriteStream, mkdirSync, renameSync, rmSync, statSync } from 'node:fs';
import { join } from 'node:path';
import type { DatabaseSync, StatementSync } from 'node:sqlite';
import { ROOT, Tree, buildCalendar, escapeHtml, renderInline, type NodeId } from '@outliner/core';
import type { Store } from './store.ts';

export interface Upload {
  key: string;
  name: string;
  type: string;
  size: number;
  created: number;
}

export interface Share {
  token: string;
  doc: string;
  zoom: string | null;
  created: number;
}

const newKey = () => randomBytes(16).toString('base64url');

export const isKey = (s: string) => /^[A-Za-z0-9_-]{16,64}$/.test(s);

/** A file name safe for a URL segment and a Content-Disposition header. */
export function cleanName(raw: string): string {
  const base = raw.split(/[\\/]/).pop() ?? '';
  const name = base.replace(/[\u0000-\u001f\u007f"<>|*?:]/g, '').replace(/^\.+/, '').trim().slice(-120);
  return name || 'file';
}

const cleanType = (t: string | undefined) => {
  const m = /^\s*([\w.+-]+\/[\w.+-]+)/.exec(t ?? '');
  return m ? m[1]!.toLowerCase() : 'application/octet-stream';
};

/** Types shown in the browser; everything else is downloaded. */
const INLINE = /^(image\/(png|jpeg|gif|webp|avif|bmp|svg\+xml)|application\/pdf|text\/plain|audio\/[\w.+-]+|video\/[\w.+-]+)$/;

export class Extras {
  private readonly q: Record<string, StatementSync>;
  readonly filesDir: string | undefined;
  constructor(db: DatabaseSync, filesDir: string | undefined) {
    this.filesDir = filesDir;
    db.exec(`
      CREATE TABLE IF NOT EXISTS uploads (
        key     TEXT PRIMARY KEY,
        name    TEXT NOT NULL,
        type    TEXT NOT NULL,
        size    INTEGER NOT NULL,
        created INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS shares (
        token   TEXT PRIMARY KEY,
        doc     TEXT NOT NULL,
        zoom    TEXT,
        created INTEGER NOT NULL
      );
    `);
    const p = (sql: string) => db.prepare(sql);
    this.q = {
      upInsert: p('INSERT INTO uploads (key, name, type, size, created) VALUES (?, ?, ?, ?, ?)'),
      upGet: p('SELECT key, name, type, size, created FROM uploads WHERE key = ?'),
      upList: p('SELECT key, name, type, size, created FROM uploads ORDER BY created DESC'),
      upDelete: p('DELETE FROM uploads WHERE key = ?'),
      shInsert: p('INSERT INTO shares (token, doc, zoom, created) VALUES (?, ?, ?, ?)'),
      shGet: p('SELECT token, doc, zoom, created FROM shares WHERE token = ?'),
      shList: p('SELECT token, doc, zoom, created FROM shares ORDER BY created DESC'),
      shDelete: p('DELETE FROM shares WHERE token = ?'),
    };
    if (filesDir) mkdirSync(filesDir, { recursive: true });
  }

  // ---- uploads ------------------------------------------------------------

  /** Stream a request body into the files directory. Rejects with 'too large' past `max` bytes. */
  receive(req: http.IncomingMessage, rawName: string, max: number): Promise<Upload> {
    const dir = this.filesDir;
    if (!dir) return Promise.reject(new Error('uploads disabled'));
    const key = newKey();
    const tmp = join(dir, `.${key}.part`);
    const name = cleanName(rawName);
    const type = cleanType(req.headers['content-type']);
    return new Promise((ok, fail) => {
      let size = 0;
      let failed = false;
      const out = createWriteStream(tmp, { flags: 'wx' });
      const abort = (err: Error) => {
        if (failed) return;
        failed = true;
        req.unpipe(out);
        req.resume();
        out.destroy();
        rmSync(tmp, { force: true });
        fail(err);
      };
      req.on('data', (c: Buffer) => {
        size += c.length;
        if (size > max) abort(new Error('too large'));
      });
      req.on('error', abort);
      out.on('error', abort);
      out.on('finish', () => {
        if (failed) return;
        renameSync(tmp, join(dir, key));
        const up: Upload = { key, name, type, size, created: Date.now() };
        this.q.upInsert!.run(key, name, type, size, up.created);
        ok(up);
      });
      req.pipe(out);
    });
  }

  uploads(): Upload[] {
    return this.q.upList!.all() as unknown as Upload[];
  }

  deleteUpload(key: string): boolean {
    if (!this.filesDir || !this.q.upGet!.get(key)) return false;
    this.q.upDelete!.run(key);
    rmSync(join(this.filesDir, key), { force: true });
    return true;
  }

  /** GET /files/<key>/<name>: no auth, the key is the capability. */
  serveFile(req: http.IncomingMessage, res: http.ServerResponse, key: string): boolean {
    if (!this.filesDir || !isKey(key)) return false;
    const up = this.q.upGet!.get(key) as Upload | undefined;
    const file = join(this.filesDir, key);
    const st = up && statSync(file, { throwIfNoEntry: false });
    if (!up || !st?.isFile()) return false;
    const inline = INLINE.test(up.type);
    res.setHeader('Content-Type', up.type === 'text/plain' ? 'text/plain; charset=utf-8' : up.type);
    res.setHeader('Content-Length', st.size);
    res.setHeader('Cache-Control', 'private, max-age=31536000, immutable');
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('Referrer-Policy', 'no-referrer');
    // uploaded content never runs script in our origin (the PDF viewer refuses a sandboxed document)
    if (up.type !== 'application/pdf') res.setHeader('Content-Security-Policy', "sandbox; default-src 'none'; img-src data:; style-src 'unsafe-inline'; media-src 'self'");
    res.setHeader('Content-Disposition', `${inline ? 'inline' : 'attachment'}; filename*=UTF-8''${encodeURIComponent(up.name)}`);
    res.writeHead(200);
    if (req.method === 'HEAD') res.end();
    else createReadStream(file).on('error', () => res.destroy()).pipe(res);
    return true;
  }

  // ---- shares -------------------------------------------------------------

  share(doc: string, zoom: string | null): Share {
    const s: Share = { token: newKey(), doc, zoom, created: Date.now() };
    this.q.shInsert!.run(s.token, s.doc, s.zoom, s.created);
    return s;
  }

  shares(): Share[] {
    return this.q.shList!.all() as unknown as Share[];
  }

  getShare(token: string): Share | undefined {
    return isKey(token) ? (this.q.shGet!.get(token) as Share | undefined) : undefined;
  }

  deleteShare(token: string): boolean {
    return Number(this.q.shDelete!.run(token).changes) > 0;
  }
}

// ---- workspace -------------------------------------------------------------

export const WS_DOC = 'workspace';

/** Documents listed in the workspace file tree (folder path, title, archived). */
export function workspaceDocs(store: Store): { id: string; title: string; archived: boolean }[] {
  const t = Tree.fromJSON(store.getDoc(WS_DOC).tree);
  const out: { id: string; title: string; archived: boolean }[] = [];
  const walk = (id: NodeId, archived: boolean) => {
    for (const c of t.get(id)?.children ?? []) {
      const n = t.get(c)!;
      const arch = archived || !!n.props.archived;
      if ((n.props.kind ?? 'doc') === 'doc') out.push({ id: c, title: String(n.props.title || 'Untitled'), archived: arch });
      else walk(c, arch);
    }
  };
  walk('files', false);
  return out;
}

// ---- calendar ----------------------------------------------------------------

export const calendarKey = (token: string) => createHmac('sha256', token).update('calendar').digest('base64url').slice(0, 32);

export function calendarFeed(store: Store, origin: string): string {
  const docs = workspaceDocs(store)
    .filter((d) => !d.archived)
    .map((d) => ({ id: d.id, title: d.title, tree: Tree.fromJSON(store.getDoc(d.id).tree) }));
  return buildCalendar(docs, { itemUrl: (d, i) => `${origin}/#d=${encodeURIComponent(d)}&z=${encodeURIComponent(i)}` });
}

// ---- public share page ---------------------------------------------------------

const SHARE_CSS = `
:root{color-scheme:light dark;--fg:#1d2329;--muted:#7a8590;--bg:#fff;--line:#e3e7ea;--accent:#3a7bd5}
@media (prefers-color-scheme:dark){:root{--fg:#d9dee3;--muted:#8a949e;--bg:#1e2124;--line:#33383d;--accent:#6aa2f0}}
body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.55 system-ui,-apple-system,"Segoe UI",sans-serif}
main{max-width:760px;margin:0 auto;padding:32px 20px 64px}
h1{font-size:1.6em;margin:0 0 4px}
.meta{color:var(--muted);font-size:.85em;margin-bottom:20px}
.top-note{color:var(--muted);white-space:pre-wrap;margin-bottom:12px}
ul{list-style:none;margin:0;padding-left:22px}
main>ul{padding-left:0}
li{position:relative}
summary{list-style:none;cursor:pointer}
summary::-webkit-details-marker{display:none}
.row{display:block;padding:2px 0 2px 18px;position:relative}
.row::before{content:"";position:absolute;left:4px;top:.72em;width:6px;height:6px;border-radius:50%;background:var(--fg);opacity:.7}
details:not([open])>summary>.row::before{box-shadow:0 0 0 4px color-mix(in srgb,var(--fg) 20%,transparent)}
details>ul{border-left:1px solid var(--line);margin-left:6px;padding-left:16px}
.ck>.row,.ck>details>summary>.row{text-decoration:line-through;opacity:.55}
.note{color:var(--muted);font-size:.9em;white-space:pre-wrap;padding-left:18px}
.h1{font-size:1.5em;font-weight:600}.h2{font-size:1.3em;font-weight:600}.h3{font-size:1.12em;font-weight:600}
code{font:.9em ui-monospace,monospace;background:color-mix(in srgb,var(--fg) 8%,transparent);padding:0 3px;border-radius:3px}
pre{white-space:pre-wrap}
mark{background:#fff3a3;color:#000}
a{color:var(--accent)}
time{color:var(--accent);white-space:nowrap}
img{max-width:100%}
footer{margin-top:40px;color:var(--muted);font-size:.8em}
`;

export function sharePage(store: Store, s: Share, opts: { collapsed?: boolean } = {}): string | null {
  const t = Tree.fromJSON(store.getDoc(s.doc).tree);
  const top: NodeId = s.zoom && t.get(s.zoom) && t.isAttached(s.zoom) ? s.zoom : ROOT;
  const rootNode = t.get(top);
  if (!rootNode) return null;
  const docTitle = workspaceDocs(store).find((d) => d.id === s.doc)?.title ?? String(t.get(ROOT)?.props.c ?? 'Untitled');
  // clean HTML writes dates literally as !(…): show them as plain <time> text
  const r = (src: unknown) => renderInline(typeof src === 'string' ? src : '', { clean: true }).replace(/!\(([^()<]*)\)/g, '<time>$1</time>');
  const item = (id: NodeId): string => {
    const n = t.get(id)!;
    const row = `<span class="row${typeof n.props.h === 'number' ? ` h${n.props.h}` : ''}">${r(n.props.c) || '&nbsp;'}</span>`;
    const note = typeof n.props.n === 'string' && n.props.n ? `<div class="note">${r(n.props.n)}</div>` : '';
    const attr = n.props.ck ? ' class="ck"' : '';
    if (!n.children.length) return `<li${attr}>${row}${note}</li>`;
    const open = opts.collapsed || n.collapsed ? '' : ' open';
    return `<li${attr}><details${open}><summary>${row}</summary>${note}<ul>${n.children.map(item).join('')}</ul></details></li>`;
  };
  const title = top === ROOT ? docTitle : (renderInline(String(rootNode.props.c ?? ''), { clean: true }) || 'Untitled');
  const plainTitle = top === ROOT ? docTitle : String(rootNode.props.c ?? '');
  const topNote = typeof rootNode.props.n === 'string' && rootNode.props.n ? `<div class="top-note">${r(rootNode.props.n)}</div>` : '';
  return `<!DOCTYPE html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="robots" content="noindex,nofollow"><title>${escapeHtml(plainTitle)}</title><style>${SHARE_CSS}</style></head>
<body><main><h1>${title}</h1>${top === ROOT ? '' : `<div class="meta">${escapeHtml(docTitle)}</div>`}${topNote}
<ul>${rootNode.children.map(item).join('')}</ul>
<footer>Read-only view · shared ${escapeHtml(new Date(s.created).toISOString().slice(0, 10))}</footer></main></body></html>
`;
}
