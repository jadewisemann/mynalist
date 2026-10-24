// Post-processing of rendered item text: syntax highlighting + Copy button for code blocks, and LaTeX
// via KaTeX (loaded on first use). Text content of highlighted code is unchanged, so click → source
// offset mapping (render.ts) still works.

import { toast } from '../ui/dom.ts';

const KEYWORDS = new Set((
  'abstract and as async await break case catch class const continue def default del do elif else enum export extends ' +
  'false final finally fn for from func function go if impl import in instanceof interface is lambda let loop match mod ' +
  'module mut new nil none not null or package pass private protected pub public raise return self static struct super ' +
  'switch this throw true try type typeof undefined use var void when where while with yield select insert update delete ' +
  'into values create table alter drop join left right inner outer on group by order having limit begin end then echo fi esac'
).split(' '));

const TOKEN = /(\/\/[^\n]*|#(?![{[(])[^\n]*|--[^\n]*|\/\*[\s\S]*?\*\/)|("(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|`(?:[^`\\]|\\.)*`)|(\b\d[\d_]*(?:\.\d+)?(?:e[+-]?\d+)?\b|\b0x[\da-f]+\b)|([A-Za-z_$][\w$]*)/gi;

/** Highlight `text` into spans (comments, strings, numbers, keywords); returns a fragment. */
export function highlight(text: string, lang: string): DocumentFragment {
  const frag = document.createDocumentFragment();
  // `#` starts a comment only in shell-like languages; `--` only in SQL/Lua/Haskell
  const hashComments = /^(py|python|sh|bash|zsh|shell|rb|ruby|r|yaml|yml|toml|perl|pl|conf|make|makefile|dockerfile)$/i.test(lang);
  const dashComments = /^(sql|lua|hs|haskell|elm)$/i.test(lang);
  let last = 0;
  const push = (s: string, cls?: string) => {
    if (!s) return;
    if (!cls) return frag.append(s);
    const span = document.createElement('span');
    span.className = `o-hl o-hl-${cls}`;
    span.textContent = s;
    frag.append(span);
  };
  const re = new RegExp(TOKEN.source, 'gi');
  for (let m = re.exec(text); m; m = re.exec(text)) {
    const [all, comment, str, num, word] = m;
    let cls: string | undefined;
    if (comment) cls = comment.startsWith('#') ? (hashComments ? 'c' : undefined) : comment.startsWith('--') ? (dashComments ? 'c' : undefined) : 'c';
    else if (str) cls = 's';
    else if (num) cls = 'n';
    else if (word && KEYWORDS.has(word.toLowerCase())) cls = 'k';
    if (!cls) {
      // not a comment in this language: keep scanning right after its first character
      if (comment) re.lastIndex = m.index + 1;
      continue;
    }
    push(text.slice(last, m.index));
    push(all, cls);
    last = m.index + all.length;
  }
  push(text.slice(last));
  return frag;
}

type Katex = typeof import('katex').default;
let katex: Katex | null = null;
let loading: Promise<void> | null = null;
const waiting = new Set<HTMLElement>();

function renderTex(el: HTMLElement) {
  const tex = el.dataset.tex ?? '';
  try {
    katex!.render(tex, el, { throwOnError: false, output: 'html' });
    el.title = tex;
  } catch {
    el.textContent = tex;
  }
}

export function enhance(root: HTMLElement) {
  for (const pre of root.querySelectorAll<HTMLElement>('pre.o-codeblock')) {
    const holder = pre.querySelector<HTMLElement>('.o-t');
    if (holder && !holder.querySelector('.o-match, .o-hl') && holder.childNodes.length === 1) {
      holder.replaceChildren(highlight(holder.textContent ?? '', pre.dataset.lang ?? ''));
    }
    if (!pre.querySelector('.o-copy')) {
      const b = document.createElement('button');
      b.className = 'o-copy';
      b.title = 'Copy';
      b.dataset.atomic = '';
      b.addEventListener('mousedown', (e) => e.preventDefault());
      b.addEventListener('click', (e) => {
        e.preventDefault();
        e.stopPropagation();
        void navigator.clipboard.writeText(pre.querySelector('code')?.textContent ?? '').then(() => toast('Copied!'));
      });
      pre.prepend(b);
    }
  }
  const tex = root.querySelectorAll<HTMLElement>('.o-latex');
  if (!tex.length) return;
  if (katex) {
    tex.forEach(renderTex);
    return;
  }
  tex.forEach((el) => waiting.add(el));
  loading ??= Promise.all([import('katex'), import('katex/dist/katex.min.css')]).then(([m]) => {
    katex = m.default;
    for (const el of waiting) if (el.isConnected) renderTex(el);
    waiting.clear();
  }, () => waiting.clear());
}
