import { afterEach, describe, expect, it, vi } from 'vitest';
import { mkdtempSync, readdirSync, rmSync, utimesSync } from 'node:fs';
import { DatabaseSync } from 'node:sqlite';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { backup, lastBackupTime, scheduleBackups } from '../src/backup.ts';

const HOUR = 3600 * 1000;
let dirs: string[] = [];
afterEach(() => {
  vi.useRealTimers();
  for (const d of dirs) rmSync(d, { recursive: true, force: true });
  dirs = [];
});

function fixture() {
  const d = mkdtempSync(join(tmpdir(), 'outliner-backup-'));
  dirs.push(d);
  const db = join(d, 'outliner.db');
  const conn = new DatabaseSync(db);
  conn.exec('CREATE TABLE t (x); INSERT INTO t VALUES (1)');
  conn.close();
  return { db, dir: join(d, 'backups') };
}
const count = (dir: string) => readdirSync(dir).filter((f) => f.endsWith('.db')).length;

describe('scheduleBackups', () => {
  it('runs a minute after start when there is no backup yet, then every interval', () => {
    vi.useFakeTimers();
    const { db, dir } = fixture();
    const stop = scheduleBackups({ db, dir, keep: 14, hours: 24, log: () => {} });
    expect(lastBackupTime(dir)).toBe(0);
    vi.advanceTimersByTime(59_000);
    expect(lastBackupTime(dir)).toBe(0);
    vi.advanceTimersByTime(1_000);
    expect(count(dir)).toBe(1);
    // the next one is a day later (backups are named by wall-clock second, so move the clock with the timers)
    vi.advanceTimersByTime(24 * HOUR);
    expect(count(dir)).toBe(2);
    stop();
    vi.advanceTimersByTime(48 * HOUR);
    expect(count(dir)).toBe(2);
  });

  it('waits for the rest of the interval after a recent backup', () => {
    const { db, dir } = fixture();
    const made = backup(db, dir, 14, new Date('2026-01-01T00:00:00Z'));
    const t = Date.now() - 20 * HOUR;
    utimesSync(made, t / 1000, t / 1000);
    vi.useFakeTimers({ now: Date.now() });
    const stop = scheduleBackups({ db, dir, keep: 14, hours: 24, log: () => {} });
    vi.advanceTimersByTime(3.9 * HOUR);
    expect(count(dir)).toBe(1);
    vi.advanceTimersByTime(0.2 * HOUR);
    expect(count(dir)).toBe(2);
    stop();
  });
});
