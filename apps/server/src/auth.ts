// Authentication: one master token from the environment (scripts, admin) plus one revocable
// token per signed-in device, the login rate limiter and client-IP detection behind a proxy.
//
// Device tokens are random 256-bit values; only their SHA-256 is stored. When the password in
// the environment changes, every device token is revoked at startup, so a leaked device token
// or an old password stops working without touching OUTLINER_TOKEN.

import type http from 'node:http';
import { createHash, randomBytes, scryptSync, timingSafeEqual } from 'node:crypto';
import type { DatabaseSync, StatementSync } from 'node:sqlite';

const sha = (s: string) => createHash('sha256').update(s, 'utf8').digest();

/** Constant-time string comparison (hashing equalizes lengths). */
export function safeEqual(a: string, b: string): boolean {
  return timingSafeEqual(sha(a), sha(b));
}

export interface Device {
  id: string;
  name: string;
  created: number;
  lastSeen: number;
  lastIp: string;
}

/** Who a request is authenticated as: the master token, or one device. */
export type Principal = { kind: 'master' } | { kind: 'device'; id: string };

const SEEN_EVERY = 60_000;
const PAIR_TTL = 5 * 60_000;
const PAIR_ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ'; // Crockford base32: no I, L, O, U

/** Canonical form of a typed pairing code: case, spaces, dashes and look-alike letters don't matter. */
export function normalizePairCode(raw: string): string {
  return raw.toUpperCase().replace(/[^0-9A-Z]/g, '').replace(/O/g, '0').replace(/[IL]/g, '1');
}

export class Auth {
  private readonly q: Record<string, StatementSync>;
  private readonly master: string;
  /** device id -> last time last_seen was written (throttles writes) */
  private readonly seen = new Map<string, number>();
  /** one-time pairing codes (in memory: a restart simply invalidates them) */
  private readonly pairs = new Map<string, number>();
  /** called with the id of every revoked device (to drop its live connections) */
  onRevoke: (id: string) => void = () => {};

  constructor(db: DatabaseSync, opts: { token: string; password?: string }) {
    this.master = opts.token;
    db.exec(`
      CREATE TABLE IF NOT EXISTS devices (
        id        TEXT PRIMARY KEY,
        hash      BLOB NOT NULL UNIQUE,
        name      TEXT NOT NULL,
        created   INTEGER NOT NULL,
        last_seen INTEGER NOT NULL,
        last_ip   TEXT NOT NULL DEFAULT ''
      );
      CREATE TABLE IF NOT EXISTS auth_meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
      );
    `);
    const p = (sql: string) => db.prepare(sql);
    this.q = {
      insert: p('INSERT INTO devices (id, hash, name, created, last_seen, last_ip) VALUES (?, ?, ?, ?, ?, ?)'),
      byHash: p('SELECT id FROM devices WHERE hash = ?'),
      list: p('SELECT id, name, created, last_seen AS lastSeen, last_ip AS lastIp FROM devices ORDER BY last_seen DESC'),
      seen: p('UPDATE devices SET last_seen = ?, last_ip = ? WHERE id = ?'),
      del: p('DELETE FROM devices WHERE id = ?'),
      ids: p('SELECT id FROM devices'),
      delAll: p('DELETE FROM devices'),
      metaGet: p('SELECT value FROM auth_meta WHERE key = ?'),
      metaSet: p('INSERT INTO auth_meta (key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value'),
    };
    if (opts.password) this.checkPasswordChange(opts.password);
  }

  /** Revoke every device token when the configured password differs from the one last seen. */
  private checkPasswordChange(password: string) {
    const row = this.q.metaGet!.get('password') as { value: string } | undefined;
    if (row) {
      const [salt, hash] = row.value.split(':');
      if (salt && hash && timingSafeEqual(scryptSync(password, Buffer.from(salt, 'hex'), 32), Buffer.from(hash, 'hex'))) return;
      const n = this.revokeAll();
      if (n) console.log(`[outliner] password changed: signed out ${n} device(s)`);
    }
    const salt = randomBytes(16);
    this.q.metaSet!.run('password', `${salt.toString('hex')}:${scryptSync(password, salt, 32).toString('hex')}`);
  }

  /** Issue a new device token. */
  issue(name: string, ip: string): { id: string; token: string } {
    const id = randomBytes(9).toString('base64url');
    const token = randomBytes(32).toString('base64url');
    const now = Date.now();
    this.q.insert!.run(id, sha(token), name.slice(0, 80) || 'device', now, now, ip);
    this.seen.set(id, now);
    return { id, token };
  }

  /** Resolve a presented token; updates the device's last-seen time (at most once a minute). */
  verify(token: string, ip: string): Principal | null {
    if (!token) return null;
    if (safeEqual(token, this.master)) return { kind: 'master' };
    const row = this.q.byHash!.get(sha(token)) as { id: string } | undefined;
    if (!row) return null;
    const now = Date.now();
    if (now - (this.seen.get(row.id) ?? 0) > SEEN_EVERY) {
      this.seen.set(row.id, now);
      this.q.seen!.run(now, ip, row.id);
    }
    return { kind: 'device', id: row.id };
  }

  /**
   * A one-time code (50 bits, valid 5 minutes) that a signed-in device shows so another device can
   * sign in without the password. Redeeming it goes through the login rate limiter.
   */
  createPairCode(now = Date.now()): { code: string; expires: number } {
    for (const [c, exp] of this.pairs) if (exp < now) this.pairs.delete(c);
    if (this.pairs.size >= 5) this.pairs.delete(this.pairs.keys().next().value!);
    const bytes = randomBytes(10);
    const code = [...bytes].map((b) => PAIR_ALPHABET[b & 31]).join('');
    const expires = now + PAIR_TTL;
    this.pairs.set(code, expires);
    return { code, expires };
  }

  /** Use up a pairing code; true if it was valid. */
  redeemPairCode(raw: string, now = Date.now()): boolean {
    const code = normalizePairCode(raw);
    const exp = this.pairs.get(code);
    if (exp === undefined) return false;
    this.pairs.delete(code);
    return exp >= now;
  }

  list(): Device[] {
    return this.q.list!.all() as unknown as Device[];
  }

  revoke(id: string): boolean {
    const ok = Number(this.q.del!.run(id).changes) > 0;
    if (ok) {
      this.seen.delete(id);
      this.onRevoke(id);
    }
    return ok;
  }

  /** Revoke every device except `keep`; returns how many were revoked. */
  revokeAll(keep?: string): number {
    let n = 0;
    for (const { id } of this.q.ids!.all() as { id: string }[]) if (id !== keep && this.revoke(id)) n++;
    return n;
  }
}

// ---- client IP ---------------------------------------------------------------

const LOOPBACK = /^(127\.|::1$|::ffff:127\.)/;

/**
 * The client's IP. Behind a reverse proxy on the same host (Tailscale Funnel / serve, Caddy, nginx)
 * every connection comes from loopback, so the proxy's X-Forwarded-For is used instead — but only
 * when the TCP peer is loopback (a remote client cannot spoof it) and only its last entry (the one
 * the proxy itself added; anything before it came from the client).
 */
export function clientIp(req: http.IncomingMessage, trustProxy: boolean): string {
  const peer = req.socket.remoteAddress ?? '?';
  if (!trustProxy || !LOOPBACK.test(peer)) return peer;
  const xff = req.headers['x-forwarded-for'];
  const raw = Array.isArray(xff) ? xff.join(',') : xff;
  const last = raw?.split(',').map((s) => s.trim()).filter(Boolean).pop();
  return last && /^[0-9a-fA-F:.]{2,45}$/.test(last) ? last : peer;
}

// ---- login rate limit --------------------------------------------------------

/**
 * Failed logins are counted per client IP (10 per 15 min) and, as a backstop against guessing
 * from many addresses, in total (100 per 15 min). Either limit only blocks *logins*: devices
 * that are already signed in keep working, so the owner is never locked out of their data.
 */
export class LoginLimiter {
  private readonly perIp = new Map<string, { n: number; until: number }>();
  private global = { n: 0, until: 0 };

  private readonly window: number;
  private readonly maxPerIp: number;
  private readonly maxGlobal: number;

  constructor(window = 15 * 60 * 1000, maxPerIp = 10, maxGlobal = 100) {
    this.window = window;
    this.maxPerIp = maxPerIp;
    this.maxGlobal = maxGlobal;
  }

  blocked(ip: string, now = Date.now()): boolean {
    if (this.global.until < now) this.global = { n: 0, until: 0 };
    if (this.global.n >= this.maxGlobal) return true;
    const f = this.perIp.get(ip);
    if (f && f.until < now) this.perIp.delete(ip);
    return !!f && f.until >= now && f.n >= this.maxPerIp;
  }

  fail(ip: string, now = Date.now()) {
    if (this.perIp.size > 1000) for (const [k, v] of this.perIp) if (v.until < now) this.perIp.delete(k);
    const cur = this.perIp.get(ip) ?? { n: 0, until: now + this.window };
    cur.n++;
    this.perIp.set(ip, cur);
    if (this.global.n === 0 || this.global.until < now) this.global = { n: 0, until: now + this.window };
    this.global.n++;
  }

  succeed(ip: string) {
    this.perIp.delete(ip);
  }
}
