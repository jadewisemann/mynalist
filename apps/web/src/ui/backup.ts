// "Download backup" (docs/03 §6): every document as plain text and OPML, in its folder structure.

import { ROOT, exportOpml, exportText } from '@outliner/core';
import type { SyncClient } from '../store/sync.ts';
import type { FileEntry, Workspace } from '../store/workspace.ts';
import { zip } from './zip.ts';

const safe = (s: string) => s.replace(/[\\/:*?"<>|\u0000-\u001f]+/g, '_').replace(/^\.+/, '_').slice(0, 120) || 'Untitled';

export function backupZip(ws: Workspace, sync: SyncClient): Blob {
  const files: { name: string; data: string }[] = [];
  const used = new Set<string>();
  const unique = (name: string) => {
    let n = name;
    for (let i = 2; used.has(n.toLowerCase()); i++) n = `${name} (${i})`;
    used.add(n.toLowerCase());
    return n;
  };
  const walk = (es: FileEntry[], dir: string) => {
    for (const e of es) {
      if (e.kind === 'folder') {
        walk(e.children, `${dir}${unique(`${dir}${safe(e.title)}`).slice(dir.length)}/`);
        continue;
      }
      const t = sync.doc(e.id).live;
      const base = unique(`${dir}${safe(e.title)}`);
      const ids = t.get(ROOT)!.children;
      files.push({ name: `${base}.txt`, data: `${e.title}\n${exportText(t, ids, { indent: 'spaces' })}` });
      files.push({ name: `${base}.opml`, data: exportOpml(t, ids, { title: e.title }) });
    }
  };
  walk(ws.files(), '');
  return zip(files);
}

export function downloadBlob(blob: Blob, name: string) {
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}
