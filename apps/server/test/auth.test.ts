import { afterEach, describe, expect, it } from 'vitest';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import WebSocket from 'ws';
import { LoginLimiter } from '../src/auth.ts';
import { PASSWORD, TOKEN, api, launch, type TestServer } from './helpers.ts';

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

const login = async (srv: TestServer, name?: string, headers: Record<string, string> = {}) =>
  api(srv, '/api/login', { method: 'POST', token: null, headers, json: { password: PASSWORD, name } });

const wsOpen = (srv: TestServer, protocols?: string[], url = '/api/ws') =>
  new Promise<{ status: number; ws: WebSocket }>((ok) => {
    const ws = new WebSocket(srv.url.replace('http', 'ws') + url, protocols);
    ws.on('unexpected-response', (_req, res) => ok({ status: res.statusCode ?? 0, ws }));
    ws.on('open', () => ok({ status: 101, ws }));
    ws.on('error', () => {});
  });
const bearerProto = (t: string) => ['mynalist', `bearer.${Buffer.from(t).toString('base64url')}`];

describe('device tokens', () => {
  it('issues one token per login and lists the devices', async () => {
    const srv = await start();
    const a = await login(srv, 'Phone');
    const b = await login(srv, 'Laptop');
    expect(a.data.token).not.toBe(b.data.token);
    const list = await api(srv, '/api/devices', { token: a.data.token });
    expect(list.status).toBe(200);
    expect(list.data.current).toBe(a.data.device);
    expect(list.data.devices.map((d: { name: string }) => d.name).sort()).toEqual(['Laptop', 'Phone']);
    // the token itself is never listed
    expect(JSON.stringify(list.data)).not.toContain(a.data.token);
  });

  it('revokes a single device, the others, or itself (logout)', async () => {
    const srv = await start();
    const a = (await login(srv, 'a')).data;
    const b = (await login(srv, 'b')).data;
    const c = (await login(srv, 'c')).data;
    expect((await api(srv, `/api/devices/${b.device}`, { method: 'DELETE', token: a.token })).status).toBe(200);
    expect((await api(srv, '/api/docs', { token: b.token })).status).toBe(401);
    expect((await api(srv, `/api/devices/${b.device}`, { method: 'DELETE', token: a.token })).status).toBe(404);

    const d = (await login(srv, 'd')).data;
    const others = await api(srv, '/api/devices', { method: 'DELETE', token: a.token });
    expect(others.data).toEqual({ revoked: 2 });
    expect((await api(srv, '/api/docs', { token: c.token })).status).toBe(401);
    expect((await api(srv, '/api/docs', { token: d.token })).status).toBe(401);
    expect((await api(srv, '/api/docs', { token: a.token })).status).toBe(200);

    expect((await api(srv, '/api/logout', { method: 'POST', token: a.token })).status).toBe(200);
    expect((await api(srv, '/api/docs', { token: a.token })).status).toBe(401);
    // the master token still works and sees no devices
    expect((await api(srv, '/api/devices')).data).toEqual({ current: null, devices: [] });
  });

  it('lets only the master token mint device tokens', async () => {
    const srv = await start();
    const m = await api(srv, '/api/devices', { method: 'POST', json: { name: 'migrated' } });
    expect(m.status).toBe(200);
    expect((await api(srv, '/api/docs', { token: m.data.token })).status).toBe(200);
    expect((await api(srv, '/api/devices', { method: 'POST', token: m.data.token, json: {} })).status).toBe(403);
  });

  it('signs out every device when the password changes, and keeps them across restarts otherwise', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'outliner-auth-'));
    const s1 = await launch({}, {}, dir);
    const t = (await login(s1)).data.token;
    await s1.close();

    const s2 = await launch({}, {}, dir); // same password
    expect((await api(s2, '/api/docs', { token: t })).status).toBe(200);
    await s2.close();

    const s3 = await launch({ password: 'a different password' }, {}, dir);
    servers.push(s3);
    expect((await api(s3, '/api/docs', { token: t })).status).toBe(401);
    expect((await api(s3, '/api/docs')).status).toBe(200); // master token unaffected
  });
});

describe('pairing', () => {
  const redeem = (srv: TestServer, code: string) =>
    api(srv, '/api/login', { method: 'POST', token: null, json: { code, name: 'New phone' } });

  it('signs in a new device with a one-time code from a signed-in one, even without a password', async () => {
    const srv = await start({ password: undefined });
    const me = (await api(srv, '/api/devices', { method: 'POST', json: { name: 'laptop' } })).data;
    expect((await api(srv, '/api/pair', { method: 'POST', token: null })).status).toBe(401);
    const { code, expires } = (await api(srv, '/api/pair', { method: 'POST', token: me.token })).data;
    expect(code).toMatch(/^[0-9A-Z]{10}$/);
    expect(expires).toBeGreaterThan(Date.now());
    // typed sloppily: lower case, dashes and spaces are fine
    const typed = `${code.slice(0, 5).toLowerCase()} - ${code.slice(5)}`;
    const r = await redeem(srv, typed);
    expect(r.status).toBe(200);
    expect((await api(srv, '/api/docs', { token: r.data.token })).status).toBe(200);
    // single use
    expect((await redeem(srv, code)).status).toBe(401);
  });

  it('rate-limits wrong codes like wrong passwords', async () => {
    const srv = await start();
    for (let i = 0; i < 10; i++) expect((await redeem(srv, 'AAAAAAAAAA')).status).toBe(401);
    const { code } = (await api(srv, '/api/pair', { method: 'POST' })).data;
    expect((await redeem(srv, code)).status).toBe(429);
  });
});

describe('websocket auth', () => {
  it('accepts a device token as subprotocol and closes the socket when the device is revoked', async () => {
    const srv = await start();
    const a = (await login(srv, 'a')).data;
    const { status, ws } = await wsOpen(srv, bearerProto(a.token));
    expect(status).toBe(101);
    expect(ws.protocol).toBe('mynalist'); // the bearer.* protocol is never echoed back
    const closed = new Promise<number>((ok) => ws.on('close', (code) => ok(code)));
    await api(srv, `/api/devices/${a.device}`, { method: 'DELETE' });
    expect(await closed).toBe(4401);
  });

  it('no longer accepts the token in the query string', async () => {
    const srv = await start();
    expect((await wsOpen(srv, undefined, `/api/ws?token=${TOKEN}`)).status).toBe(401);
    expect((await wsOpen(srv, bearerProto('bad'))).status).toBe(401);
  });
});

describe('login rate limit', () => {
  it('counts failures per forwarded client IP behind a loopback proxy', async () => {
    const srv = await start();
    const as = (ip: string, password: string) =>
      api(srv, '/api/login', { method: 'POST', token: null, headers: { 'X-Forwarded-For': ip }, json: { password } });
    for (let i = 0; i < 10; i++) expect((await as('203.0.113.9', 'x')).status).toBe(401);
    expect((await as('203.0.113.9', PASSWORD)).status).toBe(429);
    // another client (e.g. the owner on another network) can still log in
    expect((await as('198.51.100.7', PASSWORD)).status).toBe(200);
    // the proxy's own (last) entry counts, not what the client put in front of it
    expect((await as('198.51.100.7, 203.0.113.9', PASSWORD)).status).toBe(429);
  });

  it('ignores X-Forwarded-For when proxy trust is off', async () => {
    const srv = await start({ trustProxy: false });
    for (let i = 0; i < 10; i++) {
      const r = await api(srv, '/api/login', { method: 'POST', token: null, headers: { 'X-Forwarded-For': `10.0.0.${i}` }, json: { password: 'x' } });
      expect(r.status).toBe(401);
    }
    expect((await login(srv)).status).toBe(429);
  });

  it('has a global backstop that expires', () => {
    const l = new LoginLimiter(1000, 10, 5);
    for (let i = 0; i < 5; i++) l.fail(`ip${i}`, 0);
    expect(l.blocked('fresh', 10)).toBe(true);
    expect(l.blocked('fresh', 2000)).toBe(false);
  });
});
