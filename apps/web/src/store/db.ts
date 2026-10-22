// Minimal IndexedDB key-value store.

const DB_NAME = 'outliner';
const STORE = 'kv';

let dbp: Promise<IDBDatabase> | null = null;

function open(): Promise<IDBDatabase> {
  dbp ??= new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
  return dbp;
}

function run<T>(mode: IDBTransactionMode, fn: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  return open().then(
    (db) =>
      new Promise<T>((resolve, reject) => {
        const tx = db.transaction(STORE, mode);
        const req = fn(tx.objectStore(STORE));
        tx.oncomplete = () => resolve(req.result);
        tx.onerror = () => reject(tx.error);
      }),
  );
}

export const kvGet = <T>(key: string) => run<T | undefined>('readonly', (s) => s.get(key));
export const kvSet = (key: string, value: unknown) => run('readwrite', (s) => s.put(value, key)).then(() => {});
export const kvDel = (key: string) => run('readwrite', (s) => s.delete(key)).then(() => {});
export const kvKeys = () => run<IDBValidKey[]>('readonly', (s) => s.getAllKeys()).then((k) => k.map(String));
