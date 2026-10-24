// Clipboard adapters over core paste/export.
import { parseClipboard, portableToHtml as toHtml, type Portable } from '@outliner/core';

export function parseClipboardData(dt: DataTransfer): Portable[] {
  const r = parseClipboard({
    'text/plain': dt.getData('text/plain') || undefined,
    'text/html': dt.getData('text/html') || undefined,
  });
  return r?.items ?? [];
}

/**
 * Plain-text flavor of copied items, as Dynalist writes it: one line per item (note on the next
 * line), no indentation, trailing newline. Structure travels in the HTML flavor.
 */
export function portableToPlain(items: Portable[]): string {
  let out = '';
  const walk = (list: Portable[]) => {
    for (const it of list) {
      out += `${(it.props.c as string | undefined) ?? ''}\n`;
      if (it.props.n) out += `${it.props.n as string}\n`;
      walk(it.children);
    }
  };
  walk(items);
  return out;
}
export const portableToHtml = (items: Portable[]) => toHtml(items);
