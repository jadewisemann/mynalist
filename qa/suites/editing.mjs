// Keyboard editing behavior, scenario by scenario. Same steps run on Dynalist and ours; the
// resulting tree/props/caret/selection must match. Fixtures are pasted as indented plain text.

const F1 = 'one\ntwo\nthree';
const F2 = 'alpha\n  a1\n  a2\nbeta\n  b1\ngamma';
const F3 = 'top\n  mid\n    deep\n  mid2\nlast';
const T = (type) => ({ type });
const at = (i, a = 'end', f = 'c') => ({ at: [i, a, f] });

const S = [];
const add = (group, id, fx, steps, setup, extra) => S.push({ group, id: `${group}/${id}`, fx, steps, setup, ...extra });
// pixel-x caret mapping between different fonts (Whitney vs Fira Sans): allow one character
const GEOM = { caretTol: 1 };
// title <-> item: the title is 28px and starts 26px further left, and that pixel mapping also shifts by one
// character between Chromium builds (sub-pixel rendering), so recordings replayed elsewhere need one more
const GEOM_TITLE = { caretTol: 2 };

// --- Enter --------------------------------------------------------------------------------
add('enter', 'end', F1, [at(0), 'Enter', T('x')]);
add('enter', 'start', F1, [at(1, 'start'), 'Enter', T('x')]);
add('enter', 'middle', F1, [at(1, 1), 'Enter', T('x')]);
add('enter', 'end-of-last', F1, [at(2), 'Enter', T('x')]);
add('enter', 'parent-end', F2, [at(0), 'Enter', T('x')]);
add('enter', 'parent-start', F2, [at(0, 'start'), 'Enter', T('x')]);
add('enter', 'parent-middle', F2, [at(0, 2), 'Enter', T('x')]);
add('enter', 'collapsed-parent-end', F2, [at(0), 'Enter', T('x')], [['collapse', 0]]);
add('enter', 'collapsed-parent-middle', F2, [at(0, 2), 'Enter', T('x')], [['collapse', 0]]);
add('enter', 'child-last-end', F2, [at(2), 'Enter', T('x')]);
add('enter', 'empty-child', F2, [at(2), 'Enter', 'Enter', T('x')]);
add('enter', 'empty-top', F1, [at(2), 'Enter', 'Enter', T('x')]);
add('enter', 'title-end', F1, [at(-1), 'Enter', T('x')]);
add('enter', 'title-middle', F1, [at(-1, 2), 'Enter', T('x')]);
add('enter', 'with-note-end', F1, [at(0), 'Enter', T('x')], [['note', 0, 'nn']]);
add('enter', 'in-note', F1, [at(0, 'end', 'n'), 'Enter', T('x')], [['note', 0, 'nn']]);
add('enter', 'checked-end', F1, [at(1), 'Enter', T('x')], [['check', 1]]);
add('enter', 'heading-end', F1, [at(1), 'Enter', T('x')], [['heading', 1, 1]]);
add('enter', 'checkbox-end', F1, [at(1), 'Enter', T('x')], [['checkbox', 1]]);
add('enter', 'selection-replace', F1, [at(1, 1), 'Shift+ArrowRight', 'Enter', T('x')]);
add('enter', 'shift-enter-note', F1, [at(1, 1), 'Shift+Enter', T('n')]);
add('enter', 'shift-enter-back', F1, [at(1, 1), 'Shift+Enter', T('n'), 'Shift+Enter', T('c')]);
add('enter', 'mod-shift-enter-newline', F1, [at(1, 1), 'Control+Shift+Enter', T('x')]);

// --- Backspace / Delete -------------------------------------------------------------------
add('backspace', 'start-merge', F1, [at(1, 'start'), 'Backspace']);
add('backspace', 'start-merge-type', F1, [at(1, 'start'), 'Backspace', T('x')]);
add('backspace', 'start-first', F1, [at(0, 'start'), 'Backspace', T('x')]);
add('backspace', 'middle', F1, [at(1, 2), 'Backspace']);
add('backspace', 'empty-item', F1, [at(0), 'Enter', 'Backspace', T('x')]);
add('backspace', 'empty-item-twice', F1, [at(0), 'Enter', 'Backspace', 'Backspace', T('x')]);
add('backspace', 'empty-child', F2, [at(2), 'Enter', 'Backspace', T('x')]);
add('backspace', 'first-child-start', F2, [at(1, 'start'), 'Backspace', T('x')]);
add('backspace', 'into-prev-with-children', F2, [at(3, 'start'), 'Backspace', T('x')]);
add('backspace', 'self-has-children', F2, [at(3, 'start'), 'Backspace'], [['note', 2, 'n']]);
add('backspace', 'after-child', F2, [at(5, 'start'), 'Backspace', T('x')]);
add('backspace', 'prev-collapsed', F2, [at(1, 'start'), 'Backspace', T('x')], [['collapse', 0]]);
add('backspace', 'prev-has-note', F1, [at(1, 'start'), 'Backspace', T('x')], [['note', 0, 'nn']]);
add('backspace', 'note-start', F1, [at(0, 'start', 'n'), 'Backspace', T('x')], [['note', 0, 'nn']]);
add('backspace', 'selection', F1, [at(1, 0), 'Shift+End', 'Backspace', T('x')]);
add('backspace', 'word', F1, [at(2), 'Control+Backspace']);
add('backspace', 'title-start', F1, [at(-1, 'start'), 'Backspace']);
add('backspace', 'heading-start', F1, [at(1, 'start'), 'Backspace'], [['heading', 1, 1]]);
add('backspace', 'checkbox-start', F1, [at(1, 'start'), 'Backspace'], [['checkbox', 1]]);
add('delete', 'end-merge', F1, [at(0), 'Delete', T('x')]);
add('delete', 'end-last', F1, [at(2), 'Delete', T('x')]);
add('delete', 'end-parent', F2, [at(0), 'Delete', T('x')]);
add('delete', 'end-before-parent', F2, [at(2), 'Delete', T('x')]);
add('delete', 'end-collapsed-next', F2, [at(0), 'Delete'], [['collapse', 3]]);
add('delete', 'middle', F1, [at(1, 1), 'Delete']);

// --- Indent / outdent ----------------------------------------------------------------------
add('indent', 'first', F1, [at(0, 1), 'Tab', T('x')]);
add('indent', 'second', F1, [at(1, 1), 'Tab', T('x')]);
add('indent', 'twice', F1, [at(2, 1), 'Tab', 'Tab', T('x')]);
add('indent', 'with-children', F2, [at(3), 'Tab', T('x')]);
add('indent', 'into-collapsed', F2, [at(1), 'Tab', T('x')], [['collapse', 0]]);
add('indent', 'into-parent-with-children', F2, [at(3, 2), 'Tab']);
add('indent', 'in-note', F1, [at(1, 'end', 'n'), 'Tab', T('x')], [['note', 1, 'nn']]);
add('indent', 'title', F1, [at(-1), 'Tab', T('x')]);
add('outdent', 'top', F1, [at(1, 1), 'Shift+Tab', T('x')]);
add('outdent', 'first-child', F2, [at(1, 1), 'Shift+Tab', T('x')]);
add('outdent', 'last-child', F2, [at(2, 1), 'Shift+Tab', T('x')]);
add('outdent', 'deep', F3, [at(2, 1), 'Shift+Tab', T('x')]);
add('outdent', 'with-children', F3, [at(1), 'Shift+Tab', T('x')]);
add('outdent', 'twice', F3, [at(2), 'Shift+Tab', 'Shift+Tab', T('x')]);

// --- Move ----------------------------------------------------------------------------------
add('move', 'up', F1, [at(1, 1), 'Control+ArrowUp', T('x')]);
add('move', 'up-first', F1, [at(0, 1), 'Control+ArrowUp', T('x')]);
add('move', 'up-over-parent', F2, [at(3), 'Control+ArrowUp', T('x')]);
add('move', 'up-first-child', F2, [at(1), 'Control+ArrowUp', T('x')]);
add('move', 'up-first-child-of-second', F2, [at(4), 'Control+ArrowUp', T('x')]);
add('move', 'up-twice', F1, [at(2), 'Control+ArrowUp', 'Control+ArrowUp', T('x')]);
add('move', 'down', F1, [at(0, 1), 'Control+ArrowDown', T('x')]);
add('move', 'down-last', F1, [at(2, 1), 'Control+ArrowDown', T('x')]);
add('move', 'down-last-child', F2, [at(2), 'Control+ArrowDown', T('x')]);
add('move', 'down-over-parent', F2, [at(0), 'Control+ArrowDown', T('x')]);
add('move', 'down-into-collapsed', F2, [at(0), 'Control+ArrowDown', T('x')], [['collapse', 3]]);
add('move', 'up-from-note', F1, [at(1, 'end', 'n'), 'Control+ArrowUp', T('x')], [['note', 1, 'nn']]);
add('move', 'zoom-prev-sibling', F2, [at(3), 'Control+Shift+BracketLeft']);
add('move', 'zoom-next-sibling', F2, [at(0), 'Control+Shift+BracketRight']);

// --- Caret navigation ----------------------------------------------------------------------
add('nav', 'down', F1, [at(0, 2), 'ArrowDown', T('x')]);
add('nav', 'down-shorter', F1, [at(2, 4), 'ArrowUp', T('x')]);
add('nav', 'up', F1, [at(1, 1), 'ArrowUp', T('x')]);
add('nav', 'up-first-to-title', F1, [at(0, 1), 'ArrowUp', T('x')], undefined, GEOM_TITLE);
add('nav', 'down-last', F1, [at(2, 1), 'ArrowDown', T('x')]);
add('nav', 'down-title', F1, [at(-1, 2), 'ArrowDown', T('x')], undefined, GEOM_TITLE);
add('nav', 'left-at-start', F1, [at(1, 'start'), 'ArrowLeft', T('x')]);
add('nav', 'right-at-end', F1, [at(1), 'ArrowRight', T('x')]);
add('nav', 'down-into-note', F1, [at(0, 1), 'ArrowDown', T('x')], [['note', 0, 'nn']]);
add('nav', 'up-from-note-skip', F1, [at(1, 1), 'ArrowUp', T('x')], [['note', 0, 'nn']]);
add('nav', 'right-end-into-note', F1, [at(0), 'ArrowRight', T('x')], [['note', 0, 'nn']]);
add('nav', 'down-over-collapsed', F2, [at(0, 1), 'ArrowDown', T('x')], [['collapse', 0]]);
add('nav', 'down-into-children', F2, [at(0, 1), 'ArrowDown', T('x')]);
add('nav', 'mod-home', F2, [at(4, 1), 'Control+Home', T('x')]);
add('nav', 'mod-end', F2, [at(1, 1), 'Control+End', T('x')]);
add('nav', 'home-end', F1, [at(1, 1), 'End', T('x'), 'Home', T('y')]);
add('nav', 'escape', F1, [at(1, 1), 'Escape']);
add('nav', 'escape-type', F1, [at(1, 1), 'Escape', T('x')]);

// --- Selection -----------------------------------------------------------------------------
add('select', 'shift-down-mid', F1, [at(0, 1), 'Shift+ArrowDown']);
add('select', 'shift-down-end', F1, [at(0), 'Shift+ArrowDown']);
add('select', 'shift-down-twice', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown']);
add('select', 'shift-down-thrice', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Shift+ArrowDown']);
add('select', 'shift-up', F1, [at(2, 1), 'Shift+ArrowUp']);
add('select', 'shift-up-twice', F1, [at(2, 1), 'Shift+ArrowUp', 'Shift+ArrowUp']);
add('select', 'shift-down-up', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Shift+ArrowUp']);
add('select', 'shift-down-children', F2, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown']);
add('select', 'shift-down-from-child', F2, [at(2, 1), 'Shift+ArrowDown', 'Shift+ArrowDown']);
add('select', 'select-all', F2, [at(1, 1), 'Control+Shift+A']);
add('select', 'mod-a', F1, [at(1, 1), 'Control+a', T('x')]);
add('select', 'mod-a-twice', F1, [at(1, 1), 'Control+a', 'Control+a']);
add('select', 'shift-right', F1, [at(1, 1), 'Shift+ArrowRight', 'Shift+ArrowRight', T('x')]);
add('select', 'then-escape', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Escape']);
add('select', 'then-delete', F2, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Backspace']);
add('select', 'then-tab', F1, [at(1, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Tab']);
add('select', 'then-shift-tab', F2, [at(1, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Shift+Tab']);
add('select', 'then-move-up', F1, [at(1, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Control+ArrowUp']);
add('select', 'then-move-down', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Control+ArrowDown']);
add('select', 'then-check', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Control+Enter']);
add('select', 'then-type', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', T('x')]);
add('select', 'then-enter', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Enter']);
add('select', 'then-arrow-down', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'ArrowDown']);
add('select', 'then-arrow-up', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'ArrowUp']);
add('select', 'then-collapse', F2, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Control+Period']);
add('select', 'then-undo', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Backspace', 'Control+z']);

// --- Item properties -----------------------------------------------------------------------
add('props', 'check', F1, [at(1, 1), 'Control+Enter']);
add('props', 'check-twice', F1, [at(1, 1), 'Control+Enter', 'Control+Enter']);
add('props', 'check-parent', F2, [at(0, 1), 'Control+Enter']);
add('props', 'check-child-last', F2, [at(1, 1), 'Control+Enter', at(2, 1), 'Control+Enter']);
add('props', 'check-title', F1, [at(-1, 1), 'Control+Enter']);
add('props', 'checkbox', F1, [at(1, 1), 'Control+Shift+C']);
add('props', 'checkbox-twice', F1, [at(1, 1), 'Control+Shift+C', 'Control+Shift+C']);
add('props', 'checkbox-then-check', F1, [at(1, 1), 'Control+Shift+C', 'Control+Enter']);
add('props', 'heading-cycle-1', F1, [at(1, 1), 'Control+Shift+H']);
add('props', 'heading-cycle-2', F1, [at(1, 1), 'Control+Shift+H', 'Control+Shift+H']);
add('props', 'heading-cycle-3', F1, [at(1, 1), 'Control+Shift+H', 'Control+Shift+H', 'Control+Shift+H']);
add('props', 'heading-cycle-4', F1, [at(1, 1), 'Control+Shift+H', 'Control+Shift+H', 'Control+Shift+H', 'Control+Shift+H']);
add('props', 'numbered', F2, [at(0, 1), 'Control+Shift+X']);
add('props', 'numbered-leaf', F1, [at(1, 1), 'Control+Shift+X']);
add('props', 'numbered-twice', F2, [at(0, 1), 'Control+Shift+X', 'Control+Shift+X']);

// --- Collapse / zoom -----------------------------------------------------------------------
add('collapse', 'parent', F2, [at(0, 1), 'Control+Period']);
add('collapse', 'parent-twice', F2, [at(0, 1), 'Control+Period', 'Control+Period']);
add('collapse', 'leaf', F1, [at(1, 1), 'Control+Period', T('x')]);
add('collapse', 'from-child', F2, [at(1, 1), 'Control+Period', T('x')]);
add('collapse', 'all', F3, [at(0, 1), 'Control+Shift+Period']);
add('collapse', 'all-twice', F3, [at(0, 1), 'Control+Shift+Period', 'Control+Shift+Period']);
add('collapse', 'title', F2, [at(-1, 1), 'Control+Period']);
add('zoom', 'in', F2, [at(0, 1), 'Control+BracketRight']);
add('zoom', 'in-type', F2, [at(0, 1), 'Control+BracketRight', T('x')]);
add('zoom', 'in-leaf', F1, [at(1, 1), 'Control+BracketRight']);
add('zoom', 'in-out', F2, [at(1, 1), 'Control+BracketRight', 'Control+BracketLeft']);
add('zoom', 'in-child-out', F3, [at(2, 1), 'Control+BracketRight', 'Control+BracketLeft']);
add('zoom', 'in-enter-title', F2, [at(0, 1), 'Control+BracketRight', at(-1), 'Enter', T('x')]);
add('zoom', 'in-collapsed', F2, [at(0, 1), 'Control+BracketRight'], [['collapse', 0]]);
add('zoom', 'out-at-root', F1, [at(1, 1), 'Control+BracketLeft', T('x')]);

// --- Delete item ---------------------------------------------------------------------------
add('delitem', 'middle', F1, [at(1, 1), 'Control+Shift+Backspace', T('x')]);
add('delitem', 'first', F1, [at(0, 1), 'Control+Shift+Backspace', T('x')]);
add('delitem', 'last', F1, [at(2, 1), 'Control+Shift+Backspace', T('x')]);
add('delitem', 'parent', F2, [at(0, 1), 'Control+Shift+Backspace', T('x')]);
add('delitem', 'only-child', F2, [at(4, 1), 'Control+Shift+Backspace', T('x')]);
add('delitem', 'from-above', F1, [at(1, 1), 'Control+Shift+Delete', T('x')]);

// --- Undo / redo ---------------------------------------------------------------------------
add('undo', 'typing', F1, [at(1), T('abc'), 'Control+z']);
add('undo', 'typing-words', F1, [at(1), T('abc def'), 'Control+z']);
add('undo', 'typing-pause', F1, [at(1), T('abc'), { wait: 1600 }, T('def'), 'Control+z']);
add('undo', 'enter', F1, [at(1, 1), 'Enter', 'Control+z', T('x')]);
add('undo', 'enter-type', F1, [at(1), 'Enter', T('new'), 'Control+z']);
add('undo', 'enter-type-twice', F1, [at(1), 'Enter', T('new'), 'Control+z', 'Control+z']);
add('undo', 'tab', F1, [at(1, 1), 'Tab', 'Control+z', T('x')]);
add('undo', 'merge', F1, [at(1, 'start'), 'Backspace', 'Control+z', T('x')]);
add('undo', 'move', F1, [at(1, 1), 'Control+ArrowUp', 'Control+z', T('x')]);
add('undo', 'check', F1, [at(1, 1), 'Control+Enter', 'Control+z', T('x')]);
add('undo', 'redo-y', F1, [at(1, 1), 'Tab', 'Control+z', 'Control+y', T('x')]);
add('undo', 'redo-shift-z', F1, [at(1, 1), 'Tab', 'Control+z', 'Control+Shift+Z', T('x')]);
add('undo', 'delete-item', F2, [at(0, 1), 'Control+Shift+Backspace', 'Control+z', T('x')]);
add('undo', 'collapse', F2, [at(0, 1), 'Control+Period', 'Control+z']);
add('undo', 'many', F1, [at(1), 'Enter', T('a'), 'Tab', 'Enter', T('b'), 'Control+z', 'Control+z', 'Control+z', 'Control+z']);

// --- Inline formatting keys ----------------------------------------------------------------
add('format', 'bold-empty', F1, [at(1), 'Control+b', T('x')]);
add('format', 'bold-selection', F1, [at(1, 0), 'Shift+End', 'Control+b']);
add('format', 'bold-selection-type', F1, [at(1, 0), 'Shift+End', 'Control+b', T('x')]);
add('format', 'bold-toggle-off', 'one\n**two**\nthree', [at(1, 2), 'Shift+ArrowRight', 'Shift+ArrowRight', 'Shift+ArrowRight', 'Control+b']);
add('format', 'italic-selection', F1, [at(1, 0), 'Shift+End', 'Control+i']);
add('format', 'code-selection', F1, [at(1, 0), 'Shift+End', 'Control+Backquote']);
add('format', 'link-selection', F1, [at(1, 0), 'Shift+End', 'Control+k']);
add('format', 'link-empty', F1, [at(1), 'Control+k', T('x')]);
add('format', 'items-bold', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Control+b']);

// --- Typing & auto-pairs -------------------------------------------------------------------
add('typing', 'plain', F1, [at(1), T(' hello world')]);
add('typing', 'paren', F1, [at(1), T('(')]);
add('typing', 'bracket', F1, [at(1), T('[')]);
add('typing', 'backtick', F1, [at(1), T('`')]);
add('typing', 'star', F1, [at(1), T('*')]);
add('typing', 'quote', F1, [at(1), T('"')]);
add('typing', 'paren-close', F1, [at(1), T('(x)')]);
add('typing', 'double-bracket', F1, [at(1), T(' [['), 'Escape']);
add('typing', 'hash', F1, [at(1), T(' #ta'), 'Escape']);
add('typing', 'hash-space', F1, [at(1), T(' #tag more')]);
add('typing', 'bang-date', F1, [at(1), T(' !'), 'Escape']);
add('typing', 'emoji', F1, [at(1), T(' 한글 ✓')]);

// --- Paste ---------------------------------------------------------------------------------
add('paste', 'single-line', F1, [at(1, 1), { paste: 'XY' }]);
add('paste', 'multi-line-end', F1, [at(1), { paste: 'p1\np2' }]);
add('paste', 'multi-line-middle', F1, [at(1, 1), { paste: 'p1\np2' }]);
add('paste', 'multi-line-start', F1, [at(1, 'start'), { paste: 'p1\np2' }]);
add('paste', 'multi-line-empty', F1, [at(1), 'Enter', { paste: 'p1\np2' }]);
add('paste', 'indented', F1, [at(1), 'Enter', { paste: 'p1\n  p2\n    p3\np4' }]);
add('paste', 'tabs', F1, [at(1), 'Enter', { paste: 'p1\n\tp2\n\t\tp3' }]);
add('paste', 'dash-list', F1, [at(1), 'Enter', { paste: '- p1\n- p2\n  - p3' }]);
add('paste', 'star-list', F1, [at(1), 'Enter', { paste: '* p1\n* p2' }]);
add('paste', 'numbered-list', F1, [at(1), 'Enter', { paste: '1. p1\n2. p2' }]);
add('paste', 'markdown-checkbox', F1, [at(1), 'Enter', { paste: '- [ ] todo\n- [x] done' }]);
add('paste', 'blank-lines', F1, [at(1), 'Enter', { paste: 'p1\n\np2\n\n\np3' }]);
add('paste', 'crlf', F1, [at(1), 'Enter', { paste: 'p1\r\np2' }]);
add('paste', 'trailing-newline', F1, [at(1), 'Enter', { paste: 'p1\np2\n' }]);
add('paste', 'into-note', F1, [at(1, 'end', 'n'), { paste: 'p1\np2' }], [['note', 1, 'nn']]);
add('paste', 'into-title', F1, [at(-1), { paste: 'p1\np2' }]);
add('paste', 'over-selection', F1, [at(1, 0), 'Shift+End', { paste: 'p1\np2' }]);
add('paste', 'nested-under-child', F2, [at(2), 'Enter', { paste: 'p1\n  p2' }]);
add('paste', 'url', F1, [at(1), { paste: ' https://example.com/x' }]);

// --- Copy / cut format (what lands on the clipboard) ---------------------------------------
add('copy', 'items', F2, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', { copy: true }]);
add('copy', 'items-with-note', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', { copy: true }], [['note', 0, 'nn']]);
add('copy', 'items-checked', F1, [at(0, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', { copy: true }], [['check', 1]]);
add('copy', 'cut-items', F2, [at(3, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', { cut: true }]);
add('copy', 'text', F1, [at(1, 0), 'Shift+End', { copy: true }]);
// (Dynalist ignores synthetic pastes that carry text/html — it waits for the native paste —
// so HTML paste and copy→paste round trips can't be compared here; see qa/PLAN.md)

// --- Edge cases learned from earlier diffs ---------------------------------------------------
add('backspace', 'empty-first', F1, [at(-1), 'Enter', 'Backspace', T('x')]);
add('backspace', 'first-with-children', F2, [at(0, 'start'), 'Backspace', T('x')]);
add('backspace', 'title-only-item-empty', '', [at(0), 'Backspace', T('x')]);
add('props', 'uncheck', F1, [at(1, 1), 'Control+Enter', T('x')], [['check', 1]]);
add('props', 'check-last', F1, [at(2, 1), 'Control+Enter', T('x')]);
add('props', 'check-then-type', F1, [at(0, 1), 'Control+Enter', T('x')]);
add('select', 'then-delete-last', F1, [at(1, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Backspace', T('x')]);
add('select', 'then-delete-all', F1, [at(0, 1), 'Control+Shift+A', 'Backspace', T('x')]);
add('select', 'then-check-last', F1, [at(1, 1), 'Shift+ArrowDown', 'Shift+ArrowDown', 'Control+Enter', T('x')]);
add('delitem', 'only-item', 'one', [at(0, 1), 'Control+Shift+Backspace', T('x')]);
add('delitem', 'last-child', F2, [at(2, 1), 'Control+Shift+Backspace', T('x')]);
add('nav', 'up-title', F1, [at(-1, 2), 'ArrowUp', T('x')]);
add('nav', 'down-from-note', F1, [at(0, 'end', 'n'), 'ArrowDown', T('x')], [['note', 0, 'nn']]);
add('nav', 'up-from-note', F1, [at(0, 'start', 'n'), 'ArrowUp', T('x')], [['note', 0, 'nn']]);
add('typing', 'hash-existing', 'one #tag\ntwo', [at(1), T(' #t'), 'Escape', T('x')]);
add('typing', 'hash-escape-type', F1, [at(1), T(' #ta'), 'Escape', T('x')]);
add('zoom', 'out-title-caret', F2, [at(0, 1), 'Control+BracketRight', at(-1, 2), 'Control+BracketLeft', T('x')]);
add('collapse', 'title-type', F2, [at(-1, 1), 'Control+Period', T('x')]);

add('select', 'title-shift-down', F1, [at(-1, 2), 'Shift+ArrowDown', T('x')]);
add('select', 'title-shift-up', F1, [at(-1, 2), 'Shift+ArrowUp', T('x')]);
add('select', 'block-down-at-end', F1, [at(2, 1), 'Shift+ArrowUp', 'ArrowDown', T('x')]);
add('select', 'block-up-at-start', F1, [at(0, 1), 'Shift+ArrowDown', 'ArrowUp', T('x')]);
add('select', 'child-then-delete', F2, [at(2, 1), 'Shift+ArrowUp', 'Shift+ArrowUp', 'Backspace', T('x')]);
add('nav', 'escape-twice', F1, [at(1, 1), 'Escape', 'Escape', T('x')]);
add('nav', 'escape-arrow', F1, [at(1, 1), 'Escape', 'ArrowDown', T('x')]);
add('enter', 'title-start', F1, [at(-1, 'start'), 'Enter', T('x')]);
add('nav', 'up-to-title-end', 'single', [at(0), 'ArrowUp', T('x')], undefined, GEOM_TITLE);
add('nav', 'up-to-title-mid', 'singlelonger', [at(0, 4), 'ArrowUp', T('x')], undefined, GEOM_TITLE);
add('nav', 'down-from-title-end', 'one\ntwo', [at(-1), 'ArrowDown', T('x')], undefined, GEOM_TITLE);
add('nav', 'down-from-title-6', 'abcdefghijkl\ntwo', [at(-1, 6), 'ArrowDown', T('x')], undefined, GEOM_TITLE);

add('move', 'down-last-child-into-leaf', 'p\n  c\nq', [at(1, 1), 'Control+ArrowDown', T('x')]);
add('move', 'up-first-child-into-leaf', 'p\nq\n  c', [at(2, 1), 'Control+ArrowUp', T('x')]);
add('move', 'down-last-child-no-next', 'p\n  c', [at(1, 1), 'Control+ArrowDown', T('x')]);
add('move', 'up-first-child-no-prev', 'p\n  c', [at(1, 1), 'Control+ArrowUp', T('x')]);
add('move', 'down-last-child-deep', 'p\n  q\n    c\nr', [at(2, 1), 'Control+ArrowDown', T('x')]);
add('move', 'down-into-collapsed-sibling', 'p\n  c\nq\n  q1', [at(1, 1), 'Control+ArrowDown', T('x')], [['collapse', 2]]);
add('nav', 'goal-column', 'abcdefgh\nab\nabcdefgh', [at(0, 6), 'ArrowDown', 'ArrowDown', T('x')]);
add('nav', 'goal-column-up', 'abcdefgh\nab\nabcdefgh', [at(2, 6), 'ArrowUp', 'ArrowUp', T('x')]);
add('nav', 'escape-title', F1, [at(-1), 'Escape', T('x')]);
add('nav', 'escape-title-twice', F1, [at(-1, 3), 'Escape', 'Escape', T('x')]);
// vertical moves across depths: where does the caret land in a shallower/deeper item?
add('nav', 'up-shallower-start', 'alpha\n  a1\nbeta\n  b1', [at(3, 'start'), 'ArrowUp', T('x')]);
add('nav', 'up-shallower-1', 'alpha\n  a1\nbeta\n  b1', [at(3, 1), 'ArrowUp', T('x')], undefined, GEOM);
add('nav', 'up-shallower-2', 'alphabet\n  a1xyz', [at(1, 2), 'ArrowUp', T('x')], undefined, GEOM);
add('nav', 'down-deeper-start', 'alphabet\n  a1xyz', [at(0, 'start'), 'ArrowDown', T('x')]);
add('nav', 'down-deeper-5', 'alphabet\n  a1xyz', [at(0, 5), 'ArrowDown', T('x')], undefined, GEOM);
add('nav', 'down-shallower-start', 'top\n  mid\nbottomline', [at(1, 'start'), 'ArrowDown', T('x')]);
add('nav', 'down-shallower-2', 'top\n  mid\nbottomline', [at(1, 2), 'ArrowDown', T('x')], undefined, GEOM);
add('nav', 'down-shallower-end', 'top\n  mid\nbottomline', [at(1), 'ArrowDown', T('x')], undefined, GEOM);
add('nav', 'up-end-longer', 'bottomline\nab', [at(1), 'ArrowUp', T('x')], undefined, GEOM);
add('nav', 'up-start-to-title', 'abc\n  def', [at(0, 'start'), 'ArrowUp', T('x')]);
add('nav', 'down-title-start', 'abc\n  def', [at(-1, 'start'), 'ArrowDown', T('x')]);
add('nav', 'up-start-select', 'alpha\n  a1\nbeta\n  b1', [at(3, 'start'), 'Shift+ArrowUp', T('x')]);
add('nav', 'left-into-empty-title', F1, [at(-1), 'Shift+Home', 'Backspace', 'ArrowDown', 'Home', 'ArrowLeft', T('x')]);
add('select', 'block-right-descendants', F2, [at(3, 'start'), 'Shift+ArrowUp', 'Shift+ArrowDown', 'ArrowRight', T('x')]);
add('select', 'block-left', F2, [at(3, 'start'), 'Shift+ArrowUp', 'Shift+ArrowDown', 'ArrowLeft', T('x')]);
add('nav', 'tab-no-focus-enter', F1, [at(0, 'start'), 'Escape', 'Tab', 'Enter', 'Control+Enter', T('x')]);
add('undo', 'redo-noop-then-undo', 'a\nb1\nc', [at(1, 'start'), 'Control+y', 'End', 'Backspace', 'Control+z', T('x')]);
add('undo', 'redo-noop-escape-undo', 'a\nb1\nc', [at(1, 'start'), 'Control+y', 'End', 'Backspace', 'Escape', 'Control+z', T('x')]);
add('undo', 'escape-then-undo-end', 'a\nb1\nc', [at(1), 'Backspace', 'Escape', 'Control+z', T('x')]);
add('undo', 'escape-then-undo-start', 'a\ndeep\nc', [at(1, 1), 'Backspace', 'Escape', 'Control+z', T('x')]);
add('undo', 'escape-then-undo-mid', 'a\nabcdef\nc', [at(1, 4), 'Backspace', 'Escape', 'Control+z', T('x')]);
add('undo', 'escape-then-undo-two', 'a\nabcdef\nc', [at(1, 4), 'Backspace', 'Backspace', 'Escape', 'Control+z', T('x')]);
add('undo', 'escape-then-undo-typed', 'a\nabc\nc', [at(1, 1), T('xy'), 'Escape', 'Control+z', T('q')]);
add('undo', 'title-enter-start', F1, [at(-1, 'start'), 'Enter', 'Control+z', T('x')], undefined, { trace: true });
add('undo', 'title-enter-mid', F1, [at(-1, 3), 'Enter', 'Control+z', T('x')]);
add('undo', 'title-enter-end', F1, [at(-1), 'Enter', 'Control+z', T('x')]);
add('undo', 'title-enter-empty', 'one', [at(-1, 'start'), 'Shift+End', 'Backspace', 'Enter', 'Control+z', T('x')]);
// which keys end the hidden title selection (Enter splits the title again afterwards)
add('nav', 'title-hidden-sel-then-ArrowUp', F1, [at(-1, 3), 'Shift+ArrowUp', 'ArrowUp', 'Enter', T('x')]);
add('nav', 'title-hidden-sel-then-Control-Shift-Backspace', F1, [at(-1, 3), 'Shift+ArrowUp', 'Control+Shift+Backspace', 'Enter', T('x')]);
add('nav', 'title-hidden-sel-then-Control-ArrowUp', F1, [at(-1, 3), 'Shift+ArrowUp', 'Control+ArrowUp', 'Enter', T('x')]);
add('nav', 'title-hidden-sel-then-Shift-Tab', F1, [at(-1, 3), 'Shift+ArrowUp', 'Shift+Tab', 'Enter', T('x')]);
add('nav', 'title-hidden-sel-then-Tab', F1, [at(-1, 3), 'Shift+ArrowUp', 'Tab', 'Enter', T('x')]);
add('nav', 'title-hidden-sel-then-Backspace', F1, [at(-1, 3), 'Shift+ArrowUp', 'Backspace', 'Enter', T('x')]);
add('nav', 'title-hidden-sel-then-Home', F1, [at(-1, 3), 'Shift+ArrowUp', 'Home', 'Enter', T('x')]);
add('nav', 'title-hidden-sel-then-ArrowLeft', F1, [at(-1, 3), 'Shift+ArrowUp', 'ArrowLeft', 'Enter', T('x')]);
add('nav', 'title-hidden-sel-type-enter', F1, [at(-1, 3), 'Shift+ArrowUp', T('ab'), 'Enter', T('x')]);
// merging a parent with its only child when that child has children
add('delete', 'end-into-only-child-with-kids', 'x\ny\n  y1\n    y2\nz', [at(1), 'Delete', T('q')]);
add('backspace', 'start-only-child-with-kids', 'x\ny\n  y1\n    y2\nz', [at(2, 'start'), 'Backspace', T('q')]);
add('backspace', 'start-only-child-with-kids-empty-parent', 'x\n\n  y1\n    y2\nz', [at(2, 'start'), 'Backspace', T('q')]);
add('nav', 'title-shiftup-bksp', F1, [at(-1, 3), 'Shift+ArrowUp', 'Backspace', T('x')]);
add('nav', 'title-shiftup-down', F1, [at(-1, 3), 'Shift+ArrowUp', 'ArrowDown', T('x')]);
add('nav', 'title-shiftup-enter', F1, [at(-1, 3), 'Shift+ArrowUp', 'Enter', T('x')]);
add('nav', 'title-shiftup-type-right', F1, [at(-1, 3), 'Shift+ArrowUp', T('ab'), 'ArrowRight', T('x')]);
add('nav', 'title-shiftup-shiftdown', F1, [at(-1, 3), 'Shift+ArrowUp', 'Shift+ArrowDown', T('x')]);
add('nav', 'title-shiftup-right', F1, [at(-1, 3), 'Shift+ArrowUp', 'ArrowRight', T('x')]);
add('nav', 'title-shiftup-left', F1, [at(-1, 3), 'Shift+ArrowUp', 'ArrowLeft', T('x')]);
add('nav', 'title-shiftup-escape-now', F1, [at(-1, 3), 'Shift+ArrowUp', 'Escape', T('x')]);
add('nav', 'title-shiftdown-escape', F1, [at(-1, 3), 'Shift+ArrowDown', T('ab'), 'Escape', T('x')]);
add('nav', 'title-shiftup-type-escape2', F1, [at(-1, 3), 'Shift+ArrowUp', T('ab'), 'Escape', 'Escape', T('x')]);
add('nav', 'title-shiftup-backspace', F1, [at(-1, 3), 'Shift+ArrowUp', 'Delete', T('x')]);
add('nav', 'title-tab-escape', F1, [at(-1, 3), 'Tab', 'Escape', T('x')]);
add('nav', 'title-tab-type-escape', F1, [at(-1, 3), 'Tab', T('ab'), 'Escape', T('x')]);
add('nav', 'title-shiftup-escape', F1, [at(-1, 3), 'Shift+ArrowUp', T('ab'), 'Escape', T('x')]);
add('nav', 'escape-title-type-escape', F1, [at(-1, 3), 'Escape', T('ab'), 'Escape', T('x')]);
add('nav', 'escape-title-tab-escape', 'single', [at(0), 'ArrowUp', 'Tab', 'Escape', 'Shift+ArrowUp', T('ab'), 'Escape', T('x')]);
add('nav', 'escape-title-move-escape', F1, [at(-1, 3), 'Escape', 'End', 'Escape', T('x')]);
add('undo', 'caret-bs', 'single', [at(0, 1), 'Backspace', 'Control+z', T('x')]);
add('undo', 'caret-bs-shifttab', 'single', [at(0, 1), 'Backspace', 'Shift+Tab', 'Control+z', T('x')]);
add('undo', 'caret-bs-wait', 'single', [at(0, 1), 'Backspace', { wait: 1500 }, 'Control+z', T('x')]);
add('undo', 'caret-bs2-wait', 'abcdef', [at(0, 4), 'Backspace', { wait: 1500 }, 'Backspace', 'Control+z', T('x')]);
add('undo', 'caret-bs-end-wait', 'abcdef', [at(0), 'Backspace', { wait: 1500 }, 'Control+z', T('x')]);
add('undo', 'caret-bs-left', 'abcdef', [at(0, 4), 'Backspace', 'ArrowLeft', 'Control+z', T('x')]);
add('delete', 'empty-item', 'alpha\n  a1\nbeta', [at(1), 'Enter', 'Delete', T('x')]);
add('delete', 'empty-item-same-depth', F1, [at(0), 'Enter', 'Delete', T('x')]);
add('delete', 'empty-with-note', F1, [at(0), 'Enter', 'Shift+Enter', T('nn'), 'Shift+Enter', 'Delete', T('x')]);
add('delitem', 'block-delete-forward', F1, [at(1, 'start'), 'Shift+ArrowUp', 'Shift+ArrowDown', 'Delete', T('x')]);
add('delitem', 'block-backspace', F1, [at(1, 'start'), 'Shift+ArrowUp', 'Shift+ArrowDown', 'Backspace', T('x')]);
add('delitem', 'block-delete-last', F1, [at(2, 'start'), 'Shift+ArrowUp', 'Shift+ArrowDown', 'Delete', T('x')]);
add('nav', 'escape-block-resume', F1, [at(1, 2), 'Shift+ArrowUp', 'Shift+ArrowDown', 'Escape', 'Escape', T('x')]);
add('nav', 'escape-block-down-resume', F1, [at(0, 2), 'Shift+ArrowDown', 'Escape', 'Escape', T('x')]);
add('nav', 'escape-title-typed', F1, [at(-1, 3), T('ab'), 'Escape', T('x')]);
add('nav', 'escape-title-from-item', 'single', [at(0, 'start'), 'ArrowUp', T('ab'), 'Escape', T('x')]);
add('nav', 'escape-title-end', F1, [at(-1), 'Escape', T('x')]);
// title-only document (the single item merged into the title)
add('nav', 'title-only-shift-down', 'single', [at(0, 'start'), 'Backspace', 'Home', 'Shift+ArrowDown', T('x')]);
add('nav', 'title-only-shift-down-right', 'single', [at(0, 'start'), 'Backspace', 'Home', 'Shift+ArrowDown', 'ArrowRight', T('x')]);
add('nav', 'title-only-down', 'single', [at(0, 'start'), 'Backspace', 'Home', 'ArrowDown', T('x')]);
add('nav', 'title-only-shift-up', 'single', [at(0, 'start'), 'Backspace', 'Shift+ArrowUp', T('x')]);
// where undo puts the caret after a text edit
add('undo', 'caret-backspaces-end', 'abc', [at(0), 'Backspace', 'Backspace', 'Control+z', T('x')]);
add('undo', 'caret-backspaces-mid', 'abcdef', [at(0, 4), 'Backspace', 'Backspace', 'Control+z', T('x')]);
add('undo', 'caret-backspace-all', 'z1', [at(0), 'Backspace', 'Backspace', 'Control+z', T('x')]);
add('undo', 'caret-typed', 'abc', [at(0, 1), T('xyz'), 'Control+z', T('q')]);
add('undo', 'caret-typed-moved', 'abcdef', [at(0, 1), T('xyz'), 'End', 'Control+z', T('q')]);
add('undo', 'backspace-caret', F1, [at(1), 'Backspace', 'Control+z', T('x')]);
add('undo', 'backspace-mid-caret', F1, [at(1, 2), 'Backspace', 'Control+z', T('x')]);
add('undo', 'delete-caret', F1, [at(1, 1), 'Delete', 'Control+z', T('x')]);
add('undo', 'redo-typing-caret', F1, [at(1), T('abc'), 'Control+z', 'Control+y', T('x')]);
add('undo', 'word-delete-caret', 'one two three', [at(0), 'Control+Backspace', 'Control+z', T('x')]);

export default S;
