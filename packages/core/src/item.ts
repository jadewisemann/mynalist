import type { Props, TNode } from './tree.ts';

/**
 * Outline item properties. Keys are short because they are stored and synced
 * for every node. Unset = falsy (see Tree.apply cleanup rules).
 */
export interface ItemProps {
  /** content (inline markdown source) */
  c?: string;
  /** note (multi-line) */
  n?: string;
  /** shows a checkbox */
  cb?: boolean;
  /** completed */
  ck?: boolean;
  /** heading level 1..3 */
  h?: number;
  /** color label 1..6 */
  cl?: number;
  /** children are numbered */
  num?: boolean;
}

export type ItemKey = keyof ItemProps;

export const content = (n: TNode | undefined): string => (n?.props.c as string | undefined) ?? '';
export const note = (n: TNode | undefined): string => (n?.props.n as string | undefined) ?? '';
export const isChecked = (n: TNode | undefined): boolean => !!n?.props.ck;
export const hasCheckbox = (n: TNode | undefined): boolean => !!n?.props.cb;
export const heading = (n: TNode | undefined): number => (n?.props.h as number | undefined) ?? 0;
export const color = (n: TNode | undefined): number => (n?.props.cl as number | undefined) ?? 0;
export const numbered = (n: TNode | undefined): boolean => !!n?.props.num;

export const COLOR_NAMES = ['none', 'red', 'orange', 'yellow', 'green', 'blue', 'purple'] as const;

/** Item-level metadata copied by split/merge (everything except text). */
export function metaOf(p: Props): Props {
  const { c: _c, n: _n, ...rest } = p;
  return rest;
}
