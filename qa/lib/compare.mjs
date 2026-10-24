// Verdict for one scenario from the two apps' results. Shared by live runs and golden replays, so a
// replay judges exactly like the live comparison did.

import { diffKinds, same, withinCaretTol } from './state.mjs';

// Divergences that are Dynalist quirks rather than editing behaviour (documented in qa/PLAN.md).
// `test` (traced scenarios) gets the steps, the index of the first divergent step and the (shared) state before
// it. `marks` (search highlighting) gets both apps' final marks when they are the only difference.
export const KNOWN = [
  {
    id: 'tab-without-focus',
    why: 'Dynalist lets Tab/Shift+Tab through when nothing is focused; the browser moves focus to its focus receiver, which selects an item',
    test: (steps, k, pre) => /^(Shift\+)?Tab$/.test(steps[k]) && !pre.focus && !pre.selected.length,
  },
  {
    id: 'untitled-placeholder',
    why: 'editing an empty title (typing, or merging an item into it), Dynalist keeps its "Untitled" placeholder as real text',
    test: (steps, k, pre, dyn) => pre.title === '' && dyn.title !== 'Untitled' && dyn.title.includes('Untitled'),
  },
  {
    id: 'title-start-enter-undo',
    why: 'Enter at the very start of the title moves it into a new item; Dynalist\'s undo does not revert that',
    test: (steps, k, pre, dyn, errors, trace) =>
      steps[k] === 'Control+z' && steps[k - 1] === 'Enter' && (k >= 2 ? trace[k - 2] : pre)?.focus?.i === -1 && (k >= 2 ? trace[k - 2] : pre).focus.s === 0,
  },
  {
    id: 'demo-reset-in-undo',
    why: 'harness: the demo document is reset by deleting its content, and Dynalist\'s undo can bring it back',
    test: (steps, k, pre, dyn) => steps[k] === 'Control+z' && dyn.items.some((it) => it.c.startsWith('Hi there!')),
  },
  {
    id: 'code-backtick-highlighted',
    why: 'a search match inside inline code makes Dynalist show the opening backtick of that code span (not the closing one)',
    marks: (dyn, ours) =>
      dyn.length === ours.length &&
      dyn.every((row, i) => row.every((f, j) => f === ours[i][j] || (f.split('`').length > ours[i][j].split('`').length && f.replace(/`/g, '') === ours[i][j].replace(/`/g, '')))),
  },
  {
    id: 'dynalist-exception',
    why: 'Dynalist threw an exception during the scenario',
    test: (steps, k, pre, dyn, errors) => errors.length > 0,
  },
];

/**
 * @param sc scenario; @param dyn / ours results of runScenario ({ pre, post, clip, trace, errors, fatal })
 * @returns {{ status: 'match'|'geometry'|'known'|'diff'|'setup'|'error', kinds: string[] }}
 */
export function compare(sc, dyn, ours) {
  let kinds = [...new Set([...diffKinds(dyn.post, ours.post), ...(dyn.clip !== ours.clip ? ['clipboard'] : [])])];
  // typed text lands one char off when the caret is: accept both if that's the only difference
  if (sc.caretTol && kinds.every((k) => ['caret', 'tree', 'zoom'].includes(k)) && withinCaretTol(dyn.post, ours.post, sc.caretTol)) kinds = [];
  let tolerated = false;
  let known;
  if (sc.trace && dyn.trace && ours.trace) {
    // first step after which the two apps disagree
    const k = dyn.trace.findIndex((st, i) => !same(st, ours.trace[i]));
    if (k >= 0) {
      const key = typeof sc.steps[k] === 'string' ? sc.steps[k] : JSON.stringify(sc.steps[k]);
      kinds.unshift(`step ${k + 1}/${sc.steps.length}: ${key}`);
      // a vertical move landing one character off is font geometry, not behavior; later steps inherit it
      const d = diffKinds(dyn.trace[k], ours.trace[k]);
      tolerated = /^(Shift\+)?Arrow(Up|Down)$/.test(key) && d.every((x) => x === 'caret') && Math.abs(dyn.trace[k].focus.s - ours.trace[k].focus.s) <= 1;
      const pre = k ? dyn.trace[k - 1] : dyn.pre;
      known = KNOWN.find((q) => pre && q.test?.(sc.steps, k, pre, dyn.trace[k], dyn.errors ?? [], dyn.trace));
      if (known) kinds.unshift(`known:${known.id}`);
    }
  }
  if (!known && kinds.length && kinds.every((x) => x === 'marks') && ours.post?.marks) {
    known = KNOWN.find((q) => q.marks?.(dyn.post.marks, ours.post.marks));
    if (known) kinds.unshift(`known:${known.id}`);
  }
  const preKinds = diffKinds(dyn.pre, ours.pre).filter((k) => k !== 'focus' && k !== 'caret' && k !== 'selection');
  const status = dyn.fatal || ours.fatal ? 'error' : preKinds.length ? 'setup' : tolerated ? 'geometry' : known ? 'known' : kinds.length ? 'diff' : 'match';
  return { status, kinds: status === 'setup' ? preKinds : kinds };
}
