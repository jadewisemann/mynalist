// Strict shape validation for client input. Anything malformed is rejected
// with a 400 before it can reach Tree.apply or the database.

import type { Op, Props } from '@outliner/core';

export class BadRequest extends Error {}

export interface SyncEntry {
  doc: string;
  baseVersion: number;
  ops: Op[];
}

const ID_RE = /^[A-Za-z0-9_-]{1,128}$/;
const MAX_PROP_KEYS = 64;

export function isDocId(v: unknown): v is string {
  return typeof v === 'string' && ID_RE.test(v);
}

function isNodeId(v: unknown): v is string {
  return typeof v === 'string' && v.length > 0 && v.length <= 128;
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function fail(path: string, msg: string): never {
  throw new BadRequest(`${path}: ${msg}`);
}

function nodeId(v: unknown, path: string): string {
  if (!isNodeId(v)) fail(path, 'expected non-empty string id (max 128 chars)');
  return v;
}

function nullableId(v: unknown, path: string): string | null {
  if (v === null) return null;
  return nodeId(v, path);
}

function ts(v: unknown, path: string): number {
  if (typeof v !== 'number' || !Number.isFinite(v)) fail(path, 'expected finite number');
  return v;
}

function props(v: unknown, path: string): Props {
  if (!isPlainObject(v)) fail(path, 'expected object');
  const keys = Object.keys(v);
  if (keys.length > MAX_PROP_KEYS) fail(path, 'too many keys');
  const out: Props = {};
  for (const k of keys) {
    if (k === '__proto__' || k.length === 0 || k.length > 64) fail(`${path}.${k}`, 'invalid key');
    const val = v[k];
    // JSON-only values (no undefined/functions can arrive from JSON.parse anyway)
    if (typeof val === 'number' && !Number.isFinite(val)) fail(`${path}.${k}`, 'non-finite number');
    out[k] = val;
  }
  return out;
}

/** Validate one op and return a normalized copy containing only known fields. */
export function parseOp(v: unknown, path: string): Op {
  if (!isPlainObject(v)) fail(path, 'expected op object');
  switch (v.t) {
    case 'create':
      return {
        t: 'create',
        id: nodeId(v.id, `${path}.id`),
        parent: nodeId(v.parent, `${path}.parent`),
        after: nullableId(v.after, `${path}.after`),
        props: props(v.props, `${path}.props`),
        ts: ts(v.ts, `${path}.ts`),
      };
    case 'set':
      return {
        t: 'set',
        id: nodeId(v.id, `${path}.id`),
        props: props(v.props, `${path}.props`),
        ts: ts(v.ts, `${path}.ts`),
      };
    case 'move':
      return {
        t: 'move',
        id: nodeId(v.id, `${path}.id`),
        parent: nullableId(v.parent, `${path}.parent`),
        after: nullableId(v.after, `${path}.after`),
        ts: ts(v.ts, `${path}.ts`),
      };
    case 'collapse':
      if (typeof v.v !== 'boolean') fail(`${path}.v`, 'expected boolean');
      return { t: 'collapse', id: nodeId(v.id, `${path}.id`), v: v.v };
    default:
      fail(`${path}.t`, 'unknown op type');
  }
}

export function parseSyncBody(body: unknown): SyncEntry[] {
  if (!isPlainObject(body) || !Array.isArray(body.docs)) fail('body', 'expected {docs: [...]}');
  const seen = new Set<string>();
  return body.docs.map((d, i): SyncEntry => {
    const path = `docs[${i}]`;
    if (!isPlainObject(d)) fail(path, 'expected object');
    if (!isDocId(d.doc)) fail(`${path}.doc`, 'invalid doc id');
    if (seen.has(d.doc)) fail(`${path}.doc`, 'duplicate doc in batch');
    seen.add(d.doc);
    const bv = d.baseVersion;
    if (typeof bv !== 'number' || !Number.isSafeInteger(bv) || bv < 0) fail(`${path}.baseVersion`, 'expected integer >= 0');
    const ops = d.ops ?? [];
    if (!Array.isArray(ops)) fail(`${path}.ops`, 'expected array');
    return { doc: d.doc, baseVersion: bv, ops: ops.map((op, j) => parseOp(op, `${path}.ops[${j}]`)) };
  });
}
