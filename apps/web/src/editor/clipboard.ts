// Clipboard adapters over core paste/export.
import { parseClipboard, portableToHtml as toHtml, portableToOpml, type Portable } from '@outliner/core';

export function parseClipboardData(dt: DataTransfer): Portable[] {
  const r = parseClipboard({
    'text/plain': dt.getData('text/plain') || undefined,
    'text/html': dt.getData('text/html') || undefined,
    'text/opml': dt.getData('text/opml') || undefined,
  });
  return r?.items ?? [];
}

/**
 * Plain-text flavor of copied items, as Dynalist writes it: one line per item (note on the next
 * line), no indentation, children of numbered items prefixed `N. `, trailing newline. Structure
 * travels in the HTML and OPML flavors.
 */
export function portableToPlain(items: Portable[], numbered = false): string {
  let out = '';
  const walk = (list: Portable[], num: boolean) => {
    list.forEach((it, i) => {
      out += `${num ? `${i + 1}. ` : ''}${(it.props.c as string | undefined) ?? ''}\n`;
      if (it.props.n) out += `${it.props.n as string}\n`;
      walk(it.children, !!it.props.num);
    });
  };
  walk(items, numbered);
  return out;
}
export const portableToHtml = (items: Portable[]) => toHtml(items);
export const portableToOpmlText = (items: Portable[]) => portableToOpml(items);
