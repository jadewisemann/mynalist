// Onboarding tutorial and "What's new" (docs/03 §10), opened from the main menu.

import { h } from './dom.ts';
import { modal } from './overlay.ts';

const STEPS: [string, string][] = [
  ['Items', 'Everything is a list of items. Enter adds an item, Tab and Shift+Tab indent and unindent, Ctrl+↑/↓ moves an item.'],
  ['Zoom', 'Ctrl+] zooms into an item so it fills the page (Ctrl+[ zooms back out). Click a bullet to expand or collapse; double-click it to expand or collapse everything below.'],
  ['Notes and checkboxes', 'Shift+Enter adds a note under an item. Ctrl+Enter checks an item off; Ctrl+Shift+C gives it a checkbox.'],
  ['Formatting', '**bold**, __italic__, `code`, ==highlight==, [links](https://…), #tags and dates like !(2026-12-31). Type # or ! to get suggestions and a date picker.'],
  ['Links between items', 'Type [[ to link to any item in any document. With "Show backlinks" on, linked items show who refers to them.'],
  ['Search', 'Ctrl+F filters the document as you type. Shift+Enter in the search box shows a flat list of results, Ctrl+Enter searches every document.'],
  ['Find and capture', 'Ctrl+O jumps to a document, Ctrl+Shift+O to any item. Set an item as your inbox from its menu, then Ctrl+Shift+I captures a thought into it from anywhere.'],
  ['Your data', 'Everything works offline and syncs to your own server. Version history (document menu) and Download backup (main menu) keep you safe.'],
];

export function tutorial(onDone: () => void) {
  let i = 0;
  const title = h('h3');
  const text = h('p');
  const dots = h('div.tour-dots');
  const next = h('button.btn.primary');
  const paint = () => {
    title.textContent = `${i + 1}. ${STEPS[i]![0]}`;
    text.textContent = STEPS[i]![1];
    dots.replaceChildren(...STEPS.map((_, k) => h('span', { class: k === i ? 'on' : undefined, onclick: () => ((i = k), paint()) })));
    next.textContent = i === STEPS.length - 1 ? 'Done' : 'Next';
  };
  const close = modal('Welcome to Outliner', h('div.tour', {}, title, text, dots, h('div.tour-foot', {},
    h('button.btn', { onclick: () => (close(), onDone()) }, 'Skip'), next)));
  next.addEventListener('click', () => {
    if (i < STEPS.length - 1) {
      i++;
      paint();
    } else {
      close();
      onDone();
    }
  });
  paint();
}

const NEWS: [string, string[]][] = [
  ['October 2026', [
    'Version history with restore, and nightly server backups; Download backup from the main menu.',
    'Search results screens: flat search (Shift+Enter) and search everywhere (Ctrl+Enter), with sorting.',
    'Search and replace, more sort orders, backlinks, inbox capture (Ctrl+Shift+I).',
    'Settings with a keymap editor, per-document view options, article layout, mind map, locked documents.',
    'Syntax highlighting and a Copy button for code blocks; LaTeX rendering.',
    'File uploads, shared read-only links and a calendar feed (with a sync server).',
  ]],
];

export function whatsNew() {
  modal("What's new", h('div', {}, ...NEWS.flatMap(([when, items]) => [h('h3', {}, when), h('ul', {}, ...items.map((t) => h('li', {}, t)))])),
    [{ label: 'Okay', primary: true }]);
}
