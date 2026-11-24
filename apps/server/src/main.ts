import { loadEnv } from './config.ts';
import { startServer } from './server.ts';
import { scheduleBackups } from './backup.ts';
import { dirname, join, resolve } from 'node:path';

let env;
try {
  env = loadEnv();
} catch (err) {
  console.error(`[outliner] ${(err as Error).message}`);
  process.exit(1);
}

const srv = await startServer(env.config, env.port, env.host);
console.log(
  `[outliner] listening on http://${env.host}:${srv.port} (db: ${env.config.store.path}` +
    (env.config.staticDir ? `, static: ${env.config.staticDir}` : '') + ')',
);

// OUTLINER_BACKUP_HOURS > 0: back up from inside the server (used where there is no systemd timer, e.g. Homebrew).
const backupHours = Number(process.env.OUTLINER_BACKUP_HOURS || 0);
let stopBackups = () => {};
if (backupHours > 0 && env.config.store.path !== ':memory:') {
  const db = resolve(env.config.store.path);
  stopBackups = scheduleBackups({
    db,
    dir: resolve(process.env.OUTLINER_BACKUP_DIR || join(dirname(db), 'backups')),
    keep: Number(process.env.OUTLINER_BACKUP_KEEP || 14),
    filesDir: env.config.filesDir ? resolve(env.config.filesDir) : undefined,
    hours: backupHours,
  });
}

let closing = false;
const shutdown = (sig: string) => {
  if (closing) return;
  closing = true;
  console.log(`[outliner] ${sig}, shutting down`);
  stopBackups();
  srv.close().then(
    () => process.exit(0),
    (err: unknown) => {
      console.error(err);
      process.exit(1);
    },
  );
};
process.on('SIGINT', () => shutdown('SIGINT'));
process.on('SIGTERM', () => shutdown('SIGTERM'));
