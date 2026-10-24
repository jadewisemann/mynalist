// Inline syntax rendering: each input is pasted as one item, then the rendered (non-editing) form is
// compared as styled text runs (bold/italic/…/link href/tag/date label/latex source).

import { QA_NOW } from '../lib/runner.mjs';

const INPUTS = [
  // emphasis
  ['bold', '**bold** text'],
  ['italic', '__italic__ text'],
  ['strike', '~~gone~~ text'],
  ['highlight', '==marked== text'],
  ['code', 'run `x = 1` now'],
  ['bold-italic-nested', '**bold __both__ end**'],
  ['italic-in-bold-edge', '__**bi**__'],
  ['triple-star', '***bi***'],
  ['single-star', '*not italic*'],
  ['single-underscore', '_not italic_'],
  ['snake-case', 'snake_case_name and __x__'],
  ['bold-in-word', 'a**b**c'],
  ['italic-in-word', 'a__b__c'],
  ['unclosed-bold', '**open only'],
  ['unclosed-code', 'tick ` alone'],
  ['empty-bold', '**** x'],
  ['empty-italic', '____ x'],
  ['empty-strike', '~~~~ x'],
  ['empty-highlight', '==== x'],
  ['empty-code', '`` x'],
  ['bold-spaces-inside', '** spaced ** x'],
  ['markers-in-code', '`**not bold**`'],
  ['escape-star', '\\*\\*not bold\\*\\*'],
  ['escape-backslash', 'back \\\\ slash'],
  ['strike-highlight', '~~==both==~~'],
  ['code-backticks-double', '``a`b`` x'],
  // links
  ['md-link', 'see [site](https://example.com) ok'],
  ['md-link-bold-text', '[**bold link**](https://example.com)'],
  ['bare-url', 'go https://example.com/path?q=1 now'],
  ['bare-url-trailing-dot', 'end https://example.com.'],
  ['bare-url-parens', '(https://example.com/x)'],
  ['bare-url-comma', 'https://a.com, https://b.com'],
  ['www', 'visit www.example.com today'],
  ['email', 'mail me@example.com now'],
  ['link-no-scheme', '[x](example.com)'],
  ['image', '![cat](https://example.com/cat.png)'],
  ['dynalist-internal', '[doc](https://dynalist.io/d/abc#z=def)'],
  ['url-in-code', '`https://example.com`'],
  // tags
  ['hash-tag', 'a #tag b'],
  ['at-tag', 'ping @alice now'],
  ['tag-dash', '#multi-word'],
  ['tag-dot', '#a.b end'],
  ['tag-trailing-punct', 'done #tag, next'],
  ['tag-korean', '#한글태그 x'],
  ['tag-number-only', 'issue #123'],
  ['tag-in-word', 'a#b c'],
  ['tag-start', '#start of line'],
  ['tag-colon', '#a:b'],
  ['tag-underscore', '#snake_tag'],
  ['tag-in-url', 'https://e.com/#anchor'],
  // dates
  ['date', 'due !(2024-01-02)'],
  ['date-time', 'at !(2024-01-02 10:00)'],
  ['date-time-pm', 'at !(2024-01-02 15:30)'],
  ['date-range', '!(2024-01-02 - 2024-01-05)'],
  ['date-recurring', '!(2024-01-02 | 1w)'],
  ['date-future', '!(2099-12-31)'],
  ['date-rec-1d', '!(2024-01-02 | 1d)'],
  ['date-rec-2d', '!(2024-01-02 | 2d)'],
  ['date-rec-3w', '!(2024-01-02 | 3w)'],
  ['date-rec-1m', '!(2024-01-02 | 1m)'],
  ['date-rec-2m', '!(2024-01-02 | 2m)'],
  ['date-rec-1y', '!(2024-01-02 | 1y)'],
  ['date-rec-time', '!(2024-01-02 09:05 | 1w)'],
  ['date-time-midnight', '!(2024-01-02 00:00)'],
  ['date-time-noon', '!(2024-01-02 12:00)'],
  ['date-range-time', '!(2024-01-02 10:00 - 2024-01-02 11:30)'],
  ['date-other-year', '!(2031-06-15)'],
  ['date-1st', '!(2024-03-01)'],
  ['date-22nd', '!(2024-03-22)'],
  ['date-11th', '!(2024-03-11)'],
  ['date-invalid', '!(2024-13-45)'],
  ['date-bang-only', 'wow! (2024-01-02)'],
  // latex / code block
  ['latex', 'math $$x^2$$ here'],
  ['latex-unclosed', '$$x^2'],
  ['codeblock', '```print(1)```'],
  ['codeblock-lang', '```js const a = 1```'],
  // misc
  ['html-chars', '<b>not html</b> & co'],
  ['emoji', 'ok ✅ 👍'],
  ['leading-spaces-kept', 'a  b   c'],
  ['heading-markdown', '# not a heading'],
  ['checkbox-markdown', '[ ] not a box'],
  ['numbered-text', '1. first'],
  ['quote', '> quote'],
  ['hr', '---'],
];

// relative labels depend on "now": every scenario runs at QA_NOW in UTC (see runner.mjs), so generate around it
const p2 = (x) => String(x).padStart(2, '0');
const ymd = (d) => `${d.getUTCFullYear()}-${p2(d.getUTCMonth() + 1)}-${p2(d.getUTCDate())}`;
const day = (n) => ymd(new Date(QA_NOW + n * 86400000));
for (const n of [-8, -6, -2, -1, 0, 1, 2, 5, 6, 7, 8, 30, 400]) INPUTS.push([`date-rel${n}`, `!(${day(n)})`]);
const at = (mins) => {
  const d = new Date(QA_NOW + mins * 60000);
  return `${ymd(d)} ${p2(d.getUTCHours())}:${p2(d.getUTCMinutes())}`;
};
for (const m of [-1500, -300, -90, -61, -30, -5, 3, 30, 59, 61, 90, 300, 700, 1300, 1500, 2900, 4400]) INPUTS.push([`date-in${m}m`, `!(${at(m)})`]);
INPUTS.push(['date-rel0-time', `!(${day(0)} 15:00)`], ['date-rel1-range', `!(${day(1)} - ${day(3)})`], ['date-rel0-range-time', `!(${day(0)} 09:00 - ${day(0)} 17:00)`]);

export default INPUTS.map(([id, text]) => ({ group: 'render', id: `render/${id}`, fx: text, steps: [], render: true }));
