// Clipboard adapters over core paste/export.
import { parseClipboard, portableToHtml as toHtml, portableToText, type Portable } from '@outliner/core';

export function parseClipboardData(dt: DataTransfer): Portable[] {
  const r = parseClipboard({
    'text/plain': dt.getData('text/plain') || undefined,
    'text/html': dt.getData('text/html') || undefined,
  });
  return r?.items ?? [];
}

export const portableToPlain = (items: Portable[]) => portableToText(items, { indent: 'spaces' });
export const portableToHtml = (items: Portable[]) => toHtml(items);
