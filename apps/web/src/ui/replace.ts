// Search and replace (docs/03 §5.3): Find, Match case, Replace with; "Replace all" within the current zoom.

import { countMatches, replaceAll } from '@outliner/core';
import type { Editor } from '../editor/editor.ts';
import { h, toast } from './dom.ts';
import { modal } from './overlay.ts';

export function searchReplaceDialog(editor: Editor, initial = '') {
  const find = h('input', { placeholder: 'Find', value: initial });
  const repl = h('input', { placeholder: 'Replace with' });
  const matchCase = h('input', { type: 'checkbox' });
  const count = h('div.replace-count');
  const scope = () => editor.view.zoom;
  const update = () => {
    const n = countMatches(editor.tree, scope(), find.value, matchCase.checked);
    count.textContent = find.value ? `${n} match${n === 1 ? '' : 'es'} in this ${scope() === 'root' ? 'document' : 'item'}` : '';
  };
  find.addEventListener('input', update);
  matchCase.addEventListener('change', update);
  update();
  modal('Search and replace', h('div', {},
    h('label.field', {}, 'Find', find),
    h('label.field.row', {}, matchCase, 'Match case'),
    h('label.field', {}, 'Replace with', repl),
    count), [
    { label: 'Cancel' },
    {
      label: 'Replace all',
      primary: true,
      run: () => {
        let n = 0;
        editor.run((tx) => {
          n = replaceAll(tx, scope(), find.value, repl.value, matchCase.checked);
          return n ? { sel: { k: 'none' } } : null;
        });
        toast(n ? `Replaced ${n} occurrence${n === 1 ? '' : 's'}` : 'Nothing to replace');
      },
    },
  ]);
}
