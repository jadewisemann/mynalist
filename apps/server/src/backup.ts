// Consistent copy of the database (VACUUM INTO, safe while the server runs), keeping the newest N.
//   node apps/server/src/backup.ts            uses OUTLINER_DB, OUTLINER_BACKUP_DIR, OUTLINER_BACKUP_KEEP
// Restore: stop the service, copy a backup over OUTLINER_DB (delete the -wal/-shm files), start it.
// Uploaded files are immutable: they are copied once into <backup dir>/files (copy them back into OUTLINER_FILES_DIR).

import { DatabaseSync } from 'node:sqlite';
import { copyFileSync, existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';

export function backup(db: string, dir: string, keep: number, now = new Date()): string {
  mkdirSync(dir, { recursive: true });
  const stamp = now.toISOString().replace(/[-:]/g, '').replace('T', '-').slice(0, 15);
  const dest = join(dir, `outliner-${stamp}.db`);
  rmSync(dest, { force: true });
  const src = new DatabaseSync(db, { readOnly: true });
  try {
    src.prepare('VACUUM INTO ?').run(dest);
  } finally {
    src.close();
  }
  const old = readdirSync(dir).filter((f) => /^outliner-\d{8}-\d{6}\.db$/.test(f)).sort().reverse().slice(Math.max(1, keep));
  for (const f of old) rmSync(join(dir, f));
  return dest;
}

/** Copy uploads not yet in the backup; returns how many were copied. */
export function backupFiles(filesDir: string, dir: string): number {
  if (!existsSync(filesDir)) return 0;
  const dest = join(dir, 'files');
  mkdirSync(dest, { recursive: true });
  let n = 0;
  for (const f of readdirSync(filesDir)) {
    if (f.startsWith('.') || existsSync(join(dest, f))) continue;
    copyFileSync(join(filesDir, f), join(dest, f));
    n++;
  }
  return n;
}

if (import.meta.main) {
  const db = resolve(process.env.OUTLINER_DB || 'outliner.db');
  const dir = resolve(process.env.OUTLINER_BACKUP_DIR || join(dirname(db), 'backups'));
  const keep = Number(process.env.OUTLINER_BACKUP_KEEP || 14);
  console.log(`[outliner] backup → ${backup(db, dir, keep)}`);
  const files = process.env.OUTLINER_FILES_DIR ?? join(dirname(db), 'files');
  if (files) console.log(`[outliner] ${backupFiles(resolve(files), dir)} new uploaded files copied`);
}
