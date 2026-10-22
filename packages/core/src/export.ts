// Export: plain text, HTML and OPML 2.0 (docs/03 §6), plus OPML import.
//
// Pipeline: tree → portable items (applying "visible only" filters) →
// serializer. The serializers also accept portable items directly, which is
// what the clipboard uses.

import type { NodeId, Tree } from './tree.ts';
import { escapeHtml, renderInline } from './inline.ts';
import { parseOpml, type OpmlDocument, type PortableItem } from './paste.ts';

export const APP_NAME = 'Outliner';
/** `<flavor>` written to (and recognized in) OPML heads */
export const OPML_FLAVOR = 'outliner';

export type NotesMode = 'show' | 'first' | 'hide';

export interface ExportScope {
  /** skip children of collapsed items, hidden checked items and hidden notes */
  visibleOnly?: boolean;
  /** with visibleOnly: false hides checked items and their subtrees (default true) */
  showChecked?: boolean;
  /** with visibleOnly: note display mode (default 'show') */
  notes?: NotesMode;
  /** with visibleOnly: extra filter (e.g. active search); false skips the item and its subtree */
  include?: (id: NodeId) => boolean;
}

/** Copy subtrees into portable items, applying the visible-only rules. */
export function portableFromTree(tree: Tree, ids: NodeId[], opts: ExportScope = {}): PortableItem[] {
  const vis = !!opts.visibleOnly;
  const notes = vis ? opts.notes ?? 'show' : 'show';
  const conv = (id: NodeId): PortableItem | null => {
    const n = tree.get(id);
    if (!n) return null;
    if (vis && opts.showChecked === false && n.props.ck) return null;
    if (vis && opts.include && !opts.include(id)) return null;
    const props = { ...n.props };
    if (typeof props.n === 'string') {
      if (notes === 'hide') delete props.n;
      else if (notes === 'first') props.n = props.n.split('\n')[0]!;
      if (!props.n) delete props.n;
    }
    const kids = vis && n.collapsed ? [] : n.children;
    const children: PortableItem[] = [];
    for (const c of kids) {
      const p = conv(c);
      if (p) children.push(p);
    }
    return { props, collapsed: n.collapsed && n.children.length > 0, children };
  };
  const out: PortableItem[] = [];
  for (const id of ids) {
    const p = conv(id);
    if (p) out.push(p);
  }
  return out;
}

/** True if the items share a parent whose children are numbered. */
function parentNumbered(tree: Tree, ids: NodeId[]): boolean {
  const parents = new Set(ids.map((id) => tree.get(id)?.parent));
  if (parents.size !== 1) return false;
  const [p] = [...parents];
  return !!(p && tree.get(p)?.props.num);
}

const str = (v: unknown): string => (typeof v === 'string' ? v : '');

// ---- plain text -----------------------------------------------------------------------

export type IndentStyle = 'spaces' | 'asterisks' | 'dashes' | 'none';

export interface TextExportOptions {
  /** default 'spaces' (4 per level); asterisks/dashes add a "* "/"- " prefix */
  indent?: IndentStyle;
  /** include notes (default true) */
  includeNotes?: boolean;
  /** the top-level items are children of a numbered parent */
  topNumbered?: boolean;
}

/**
 * One line per item; numbered children get "N. " prefixes; note lines (and
 * extra lines of multi-line content) follow, aligned with the item text.
 */
export function portableToText(items: PortableItem[], opts: TextExportOptions = {}): string {
  const style = opts.indent ?? 'spaces';
  const out: string[] = [];
  const walk = (list: PortableItem[], depth: number, numbered: boolean) => {
    list.forEach((it, i) => {
      const ind = style === 'none' ? '' : '    '.repeat(depth);
      const prefix = numbered ? `${i + 1}. ` : style === 'asterisks' ? '* ' : style === 'dashes' ? '- ' : '';
      const pad = ind + ' '.repeat(prefix.length);
      const lines = str(it.props.c).split('\n');
      out.push(ind + prefix + lines[0]);
      for (const l of lines.slice(1)) out.push(pad + l);
      const note = str(it.props.n);
      if (note && opts.includeNotes !== false) for (const l of note.split('\n')) out.push(pad + l);
      walk(it.children, depth + 1, !!it.props.num);
    });
  };
  walk(items, 0, !!opts.topNumbered);
  return out.join('\n');
}

// ---- HTML ----------------------------------------------------------------------------------

export interface HtmlExportOptions {
  /** include notes (default true) */
  includeNotes?: boolean;
  topNumbered?: boolean;
  /** wrap in a complete HTML document */
  document?: boolean;
  title?: string;
}

/**
 * Nested `<ul>`/`<ol>` (ol when the parent is numbered); headings as
 * `<hN>`, checked items wrapped in `<del>`, notes after `<br>`. Inline
 * formatting rendered as clean HTML.
 */
export function portableToHtml(items: PortableItem[], opts: HtmlExportOptions = {}): string {
  const list = (items: PortableItem[], numbered: boolean): string => {
    if (!items.length) return '';
    const tag = numbered ? 'ol' : 'ul';
    let s = `<${tag}>`;
    for (const it of items) {
      let body = renderInline(str(it.props.c), { clean: true });
      if (it.props.ck) body = `<del>${body}</del>`;
      const h = it.props.h as number | undefined;
      if (h && h >= 1 && h <= 3) body = `<h${h}>${body}</h${h}>`;
      const note = str(it.props.n);
      if (note && opts.includeNotes !== false) {
        let nb = renderInline(note, { clean: true });
        if (it.props.ck) nb = `<del>${nb}</del>`;
        body += `<br>${nb}`;
      }
      s += `<li>${body}${list(it.children, !!it.props.num)}</li>`;
    }
    return s + `</${tag}>`;
  };
  const html = list(items, !!opts.topNumbered);
  if (!opts.document) return html;
  return `<!DOCTYPE html>\n<html><head><meta charset="utf-8"><title>${escapeHtml(opts.title ?? '')}</title></head><body>\n${html}\n</body></html>\n`;
}

// ---- OPML ----------------------------------------------------------------------------------

export interface OpmlExportOptions {
  title?: string;
  /** source URL of the document */
  source?: string;
  ownerName?: string;
  ownerEmail?: string;
}

/** XML attribute escaping (newlines/tabs as character references). */
export function escapeXmlAttr(s: string): string {
  return s.replace(/[&<>"\n\r\t]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', '\n': '&#10;', '\r': '&#13;', '\t': '&#9;' })[c]!);
}

const xmlText = (s: string) => s.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]!);

/** OPML 2.0 with our attributes (text, _note, checkbox, complete, heading, colorLabel, listStyle, collapsed). */
export function portableToOpml(items: PortableItem[], opts: OpmlExportOptions = {}): string {
  const lines: string[] = [
    '<?xml version="1.0" encoding="utf-8"?>',
    '<opml version="2.0">',
    '  <head>',
    `    <title>${xmlText(opts.title ?? '')}</title>`,
    `    <flavor>${OPML_FLAVOR}</flavor>`,
  ];
  if (opts.source) lines.push(`    <source>${xmlText(opts.source)}</source>`);
  if (opts.ownerName) lines.push(`    <ownerName>${xmlText(opts.ownerName)}</ownerName>`);
  if (opts.ownerEmail) lines.push(`    <ownerEmail>${xmlText(opts.ownerEmail)}</ownerEmail>`);
  lines.push('  </head>', '  <body>');
  const walk = (list: PortableItem[], depth: number) => {
    const ind = '  '.repeat(depth + 2);
    for (const it of list) {
      const p = it.props;
      let a = ` text="${escapeXmlAttr(str(p.c))}"`;
      if (str(p.n)) a += ` _note="${escapeXmlAttr(str(p.n))}"`;
      if (p.cb) a += ' checkbox="true"';
      if (p.ck) a += ' complete="true"';
      const h = p.h as number | undefined;
      if (h && h >= 1 && h <= 3) a += ` heading="${h}"`;
      const cl = p.cl as number | undefined;
      if (cl && cl >= 1 && cl <= 6) a += ` colorLabel="${cl}"`;
      if (p.num) a += ' listStyle="arabic"';
      if (it.collapsed && it.children.length) a += ' collapsed="true"';
      if (!it.children.length) {
        lines.push(`${ind}<outline${a} />`);
      } else {
        lines.push(`${ind}<outline${a}>`);
        walk(it.children, depth + 1);
        lines.push(`${ind}</outline>`);
      }
    }
  };
  walk(items, 0);
  lines.push('  </body>', '</opml>');
  return lines.join('\n') + '\n';
}

/** Import: OPML (any flavor) → title + portable items; null if not OPML. */
export function opmlToPortable(xml: string): OpmlDocument | null {
  return parseOpml(xml);
}

// ---- tree convenience wrappers ---------------------------------------------------------------

export function exportText(tree: Tree, ids: NodeId[], opts: ExportScope & TextExportOptions = {}): string {
  const items = portableFromTree(tree, ids, opts);
  return portableToText(items, { ...opts, topNumbered: opts.topNumbered ?? parentNumbered(tree, ids) });
}

export function exportHtml(tree: Tree, ids: NodeId[], opts: ExportScope & HtmlExportOptions = {}): string {
  const items = portableFromTree(tree, ids, opts);
  return portableToHtml(items, { ...opts, topNumbered: opts.topNumbered ?? parentNumbered(tree, ids) });
}

export function exportOpml(tree: Tree, ids: NodeId[], opts: ExportScope & OpmlExportOptions = {}): string {
  return portableToOpml(portableFromTree(tree, ids, opts), opts);
}
