// Environment -> ServerConfig.

import { dirname, join } from 'node:path';
import type { ServerConfig } from './server.ts';

const DAY = 24 * 3600 * 1000;

function num(env: NodeJS.ProcessEnv, key: string, def: number): number {
  const raw = env[key];
  if (raw === undefined || raw === '') return def;
  const n = Number(raw);
  if (!Number.isFinite(n) || n < 0) throw new Error(`${key} must be a non-negative number, got "${raw}"`);
  return n;
}

export interface Env {
  config: ServerConfig;
  port: number;
  host: string;
}

export function loadEnv(env: NodeJS.ProcessEnv = process.env): Env {
  const token = env.OUTLINER_TOKEN;
  if (!token) throw new Error('OUTLINER_TOKEN is required');
  if (token.length < 16) console.warn('[outliner] warning: OUTLINER_TOKEN is shorter than 16 characters');
  const origins = (env.OUTLINER_CORS_ORIGINS ?? '*').trim();
  const db = env.OUTLINER_DB || 'outliner.db';
  const files = env.OUTLINER_FILES_DIR ?? (db === ':memory:' ? '' : join(dirname(db), 'files'));
  return {
    port: num(env, 'SERVER_PORT', num(env, 'PORT', num(env, 'OUTLINER_PORT', 8080))),
    host: env.OUTLINER_HOST || '0.0.0.0',
    config: {
      token,
      password: env.OUTLINER_PASSWORD || undefined,
      corsOrigins: origins === '*' || origins === '' ? '*' : origins.split(',').map((s) => s.trim()).filter(Boolean),
      staticDir: env.OUTLINER_STATIC || undefined,
      maxBody: num(env, 'OUTLINER_MAX_BODY_MB', 20) * 1024 * 1024,
      filesDir: files || undefined,
      maxUpload: num(env, 'OUTLINER_MAX_UPLOAD_MB', 50) * 1024 * 1024,
      publicUrl: env.OUTLINER_PUBLIC_URL || undefined,
      pruneIntervalMs: 3600 * 1000,
      heartbeatMs: 30_000,
      store: {
        path: db,
        snapshotEvery: Math.max(1, num(env, 'OUTLINER_SNAPSHOT_EVERY', 200)),
        retainVersions: num(env, 'OUTLINER_RETAIN_VERSIONS', 10_000),
        retainMs: num(env, 'OUTLINER_RETAIN_DAYS', 30) * DAY,
        purgeDetachedMs: num(env, 'OUTLINER_PURGE_DETACHED_DAYS', 30) * DAY,
        cacheSize: Math.max(1, num(env, 'OUTLINER_CACHE_DOCS', 64)),
      },
    },
  };
}
