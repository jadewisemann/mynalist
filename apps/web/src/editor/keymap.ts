// Keyboard combos are normalized from KeyboardEvent.code (physical key), so
// shortcuts keep working while a non-Latin IME (e.g. Korean) is active.

export const IS_MAC = /Mac|iPhone|iPad/.test(navigator.platform);

const CODE_KEYS: Record<string, string> = {
  Period: '.', Comma: ',', Slash: '/', Backslash: '\\', BracketLeft: '[', BracketRight: ']',
  Backquote: '`', Minus: '-', Equal: '=', Semicolon: ';', Quote: "'",
  ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right',
  Enter: 'Enter', NumpadEnter: 'Enter', Backspace: 'Backspace', Delete: 'Delete', Tab: 'Tab',
  Escape: 'Escape', Home: 'Home', End: 'End', Space: 'Space', PageUp: 'PageUp', PageDown: 'PageDown',
};

export function keyOf(e: KeyboardEvent): string {
  const c = e.code;
  if (c.startsWith('Key')) return c.slice(3);
  if (c.startsWith('Digit')) return c.slice(5);
  if (CODE_KEYS[c]) return CODE_KEYS[c]!;
  if (/^F\d+$/.test(c)) return c;
  // unknown code (some mobile keyboards): fall back to key
  return e.key.length === 1 ? e.key.toUpperCase() : e.key;
}

/** "Mod+Shift+." style combo. Mod = Cmd on Mac, Ctrl elsewhere; Ctrl = literal Control on Mac. */
export function comboOf(e: KeyboardEvent): string {
  const parts: string[] = [];
  const mod = IS_MAC ? e.metaKey : e.ctrlKey;
  if (mod) parts.push('Mod');
  if (IS_MAC && e.ctrlKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  parts.push(keyOf(e));
  return parts.join('+');
}

export interface CommandInfo {
  id: string;
  label: string;
  group: 'Basics' | 'General' | 'Lists' | 'Navigate' | 'Search' | 'Format';
  keys: string[];
}

// Default bindings follow docs/02-editor-keyboard.md §1.
export const COMMANDS: CommandInfo[] = [
  { id: 'indent', label: 'Indent', group: 'Basics', keys: ['Tab'] },
  { id: 'unindent', label: 'Unindent', group: 'Basics', keys: ['Shift+Tab'] },
  { id: 'collapse', label: 'Expand/collapse', group: 'Basics', keys: ['Mod+.'] },
  { id: 'zoom-in', label: 'Zoom in', group: 'Basics', keys: ['Mod+]'] },
  { id: 'zoom-out', label: 'Zoom out', group: 'Basics', keys: ['Mod+['] },
  { id: 'delete-left', label: 'Delete item', group: 'Basics', keys: ['Mod+Shift+Backspace'] },

  { id: 'help', label: 'Shortcuts & formatting help', group: 'General', keys: ['Mod+/', 'Mod+Shift+/'] },
  { id: 'settings', label: 'Open settings', group: 'General', keys: ['Mod+,'] },
  { id: 'toggle-file-pane', label: 'Toggle file pane', group: 'General', keys: ['Mod+Shift+F'] },
  { id: 'toggle-bookmark-pane', label: 'Toggle bookmark pane', group: 'General', keys: ['Mod+Shift+B'] },
  { id: 'toggle-tag-pane', label: 'Toggle tag pane', group: 'General', keys: [] },
  { id: 'toggle-show-checked', label: 'Show/hide checked items', group: 'General', keys: [] },
  { id: 'toggle-show-notes', label: 'Show/hide notes', group: 'General', keys: [] },
  { id: 'save', label: 'Save / sync now', group: 'General', keys: ['Mod+S'] },
  { id: 'inbox-capture', label: 'Capture to inbox', group: 'General', keys: ['Mod+Shift+I'] },
  { id: 'bookmark', label: 'Bookmark current view', group: 'General', keys: [] },

  { id: 'delete-right', label: 'Delete item (focus next)', group: 'Lists', keys: ['Mod+Shift+Delete'] },
  { id: 'goto-note', label: 'Move between item and note', group: 'Lists', keys: ['Shift+Enter'] },
  { id: 'toggle-checked', label: 'Toggle completed', group: 'Lists', keys: ['Mod+Enter'] },
  { id: 'uncheck-children', label: 'Uncheck item and descendants', group: 'Lists', keys: [] },
  { id: 'collapse-all', label: 'Expand/collapse all', group: 'Lists', keys: ['Mod+Shift+.'] },
  { id: 'toggle-siblings', label: 'Expand/collapse siblings', group: 'Lists', keys: [] },
  { id: 'toggle-checkbox', label: 'Toggle checkbox', group: 'Lists', keys: ['Mod+Shift+C'] },
  { id: 'toggle-children-checkbox', label: 'Toggle checkbox on children', group: 'Lists', keys: [] },
  { id: 'toggle-numbered', label: 'Toggle numbered list', group: 'Lists', keys: ['Mod+Shift+X'] },
  { id: 'duplicate', label: 'Duplicate', group: 'Lists', keys: [] },
  { id: 'select-up', label: 'Select up', group: 'Lists', keys: ['Shift+Up'] },
  { id: 'select-down', label: 'Select down', group: 'Lists', keys: ['Shift+Down'] },
  { id: 'select-all', label: 'Select everything', group: 'Lists', keys: ['Mod+Shift+A'] },
  { id: 'undo', label: 'Undo', group: 'Lists', keys: ['Mod+Z'] },
  { id: 'redo', label: 'Redo', group: 'Lists', keys: ['Mod+Y', 'Mod+Shift+Z'] },
  { id: 'copy-link', label: 'Copy link to item', group: 'Lists', keys: [] },
  { id: 'delete-checked', label: 'Delete completed items', group: 'Lists', keys: [] },
  { id: 'sort-asc', label: 'Sort children A→Z', group: 'Lists', keys: [] },
  { id: 'sort-desc', label: 'Sort children Z→A', group: 'Lists', keys: [] },
  { id: 'expand-1', label: 'Expand to level 1', group: 'Lists', keys: [] },
  { id: 'expand-2', label: 'Expand to level 2', group: 'Lists', keys: [] },
  { id: 'expand-3', label: 'Expand to level 3', group: 'Lists', keys: [] },
  { id: 'expand-4', label: 'Expand to level 4', group: 'Lists', keys: [] },

  { id: 'move-to', label: 'Move item to…', group: 'Navigate', keys: ['Mod+Shift+M'] },
  { id: 'swap-up', label: 'Move up', group: 'Navigate', keys: ['Mod+Up'] },
  { id: 'swap-down', label: 'Move down', group: 'Navigate', keys: ['Mod+Down'] },
  { id: 'zoom-prev-sibling', label: 'Zoom into previous sibling', group: 'Navigate', keys: ['Mod+Shift+['] },
  { id: 'zoom-next-sibling', label: 'Zoom into next sibling', group: 'Navigate', keys: ['Mod+Shift+]'] },
  { id: 'goto-inbox', label: 'Go to inbox', group: 'Navigate', keys: [] },
  { id: 'goto-first', label: 'Go to first item', group: 'Navigate', keys: ['Mod+Home'] },
  { id: 'goto-last', label: 'Go to last item', group: 'Navigate', keys: ['Mod+End'] },
  { id: 'back', label: 'Back', group: 'Navigate', keys: ['Alt+Left'] },
  { id: 'forward', label: 'Forward', group: 'Navigate', keys: ['Alt+Right'] },

  { id: 'search', label: 'Search in document', group: 'Search', keys: ['Mod+F'] },
  { id: 'go-anywhere', label: 'File finder', group: 'Search', keys: ['Mod+O'] },
  { id: 'goto-item', label: 'Item finder', group: 'Search', keys: ['Mod+Shift+O'] },
  { id: 'search-replace', label: 'Search and replace', group: 'Search', keys: [] },

  { id: 'bold', label: 'Bold', group: 'Format', keys: ['Mod+B'] },
  { id: 'italic', label: 'Italic', group: 'Format', keys: ['Mod+I'] },
  { id: 'code', label: 'Inline code', group: 'Format', keys: ['Mod+`'] },
  { id: 'code-block', label: 'Code block', group: 'Format', keys: [] },
  { id: 'link', label: 'Link', group: 'Format', keys: ['Mod+K'] },
  { id: 'strike', label: 'Strikethrough', group: 'Format', keys: [] },
  { id: 'highlight', label: 'Highlight', group: 'Format', keys: [] },
  { id: 'latex', label: 'LaTeX', group: 'Format', keys: [] },
  { id: 'newline', label: 'New line in item', group: 'Format', keys: ['Mod+Shift+Enter'] },
  { id: 'cycle-heading', label: 'Cycle heading', group: 'Format', keys: ['Mod+Shift+H'] },
  { id: 'heading-1', label: 'Heading 1', group: 'Format', keys: [] },
  { id: 'heading-2', label: 'Heading 2', group: 'Format', keys: [] },
  { id: 'heading-3', label: 'Heading 3', group: 'Format', keys: [] },
  { id: 'heading-0', label: 'Clear heading', group: 'Format', keys: [] },
  { id: 'cycle-color', label: 'Cycle color label', group: 'Format', keys: ['Mod+Shift+L'] },
  ...[1, 2, 3, 4, 5, 6].map((n) => ({
    id: `color-${n}`, label: `Color label ${n}`, group: 'Format' as const, keys: [] as string[],
  })),
  { id: 'color-0', label: 'Clear color label', group: 'Format', keys: [] },
  { id: 'insert-date', label: 'Insert current date', group: 'Format', keys: [] },
  { id: 'insert-time', label: 'Insert current time', group: 'Format', keys: [] },
];

const WORKFLOWY: Record<string, string[]> = {
  'zoom-in': ['Alt+Right'],
  'zoom-out': ['Alt+Left'],
  collapse: [IS_MAC ? 'Ctrl+Up' : 'Mod+Up'],
  'swap-up': ['Alt+Shift+Up'],
  'swap-down': ['Alt+Shift+Down'],
};

export class Keymap {
  private map = new Map<string, string>();

  constructor(opts: { overrides?: Record<string, string[]>; workflowy?: boolean } = {}) {
    for (const c of COMMANDS) {
      const keys = opts.overrides?.[c.id] ?? (opts.workflowy ? WORKFLOWY[c.id] : undefined) ?? c.keys;
      for (const k of keys) this.map.set(k, c.id);
    }
  }

  resolve(combo: string): string | undefined {
    return this.map.get(combo);
  }

  keysFor(id: string): string[] {
    return [...this.map].filter(([, v]) => v === id).map(([k]) => k);
  }
}

export function prettyCombo(combo: string): string {
  return combo
    .split('+')
    .map((p) => (p === 'Mod' ? (IS_MAC ? '⌘' : 'Ctrl') : p === 'Ctrl' ? '⌃' : p === 'Alt' ? (IS_MAC ? '⌥' : 'Alt') : p === 'Shift' ? (IS_MAC ? '⇧' : 'Shift') : p))
    .join(IS_MAC ? '' : '+');
}
