import { chromium } from 'playwright-core';
const exe = `${process.env.HOME}/.cache/ms-playwright/chromium-1228/chrome-linux/chrome`;
const browser = await chromium.launch({ executablePath: exe });
const ctx = await browser.newContext({ viewport: { width: 1100, height: 760 } });
const page = await ctx.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
await page.goto('http://localhost:5173/');
await page.waitForSelector('.o-editor');
const K = (k) => page.keyboard.press(k);
const T = (t) => page.keyboard.type(t);
const outline = () => page.evaluate(() => {
  const walk = (el, d) => [...el.children].map((n) => '  '.repeat(d) + (n.classList.contains('o-collapsed') ? '+' : '') + (n.classList.contains('o-checked') ? '[x]' : '') + n.querySelector(':scope > .o-row .o-content').textContent + '\n' + walk(n.querySelector(':scope > .o-kids'), d + 1)).join('');
  return walk(document.querySelector('.o-items'), 0);
});
let fails = 0;
async function expect(label, want) {
  const got = await outline();
  const ok = got === want;
  if (!ok) fails++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${ok ? '' : `\n--- want\n${want}--- got\n${got}`}`);
}
// fresh doc
await page.click('.pane-head button');
await page.click('text=New document');
await T('Test');
await K('Enter');
await page.waitForTimeout(200);
await page.click('.o-editor', { position: { x: 300, y: 120 } });
await T('a'); await K('Enter'); await T('b'); await K('Enter'); await T('c');
await expect('enter creates siblings', 'a\nb\nc\n');
await K('Tab');
await expect('tab indents', 'a\nb\n  c\n');
await K('Shift+Tab');
await expect('shift+tab', 'a\nb\nc\n');
await K('Control+ArrowUp');
await expect('mod+up swaps', 'a\nc\nb\n');
await K('Control+ArrowDown');
await K('Home');
await K('Backspace');
await expect('backspace merges', 'a\nbc\n');
await K('Control+z');
await expect('undo merge', 'a\nb\nc\n');
await K('Control+y');
await expect('redo merge', 'a\nbc\n');
await K('ArrowLeft'); // to end of a (left at start → prev item end)
await K('Enter'); await T('x'); await K('Tab'); await K('Enter'); await T('y');
await expect('nested', 'a\n  x\n  y\nbc\n');
await K('ArrowUp'); await K('ArrowUp');
await K('Control+Period');
await expect('collapse', '+a\nbc\n');
await K('Control+Period');
await K('Control+BracketRight');
const title = await page.textContent('.o-title .o-content');
console.log(title === 'a' ? 'ok   zoom title' : `FAIL zoom title: ${title}`);
await expect('zoomed children', 'x\ny\n');
await K('Control+BracketLeft');
await expect('zoom out', 'a\n  x\n  y\nbc\n');
// block selection: shift+down from x
await page.click('.o-content >> text=x');
await K('Shift+ArrowDown'); await K('Shift+ArrowDown');
const sel = await page.$$eval('.o-selected', (els) => els.length);
console.log(sel === 2 ? 'ok   block select' : `FAIL block select ${sel}`);
await K('Tab');
await expect('block noop tab (first child)', 'a\n  x\n  y\nbc\n');
await K('Control+Enter');
await expect('block check', 'a\n  [x]x\n  [x]y\nbc\n');
await K('Control+c');
await K('Escape');
await page.click('.o-content >> text=bc');
await K('End');
await K('Enter');
await K('Control+v');
await expect('paste block', 'a\n  [x]x\n  [x]y\nbc\n[x]x\n[x]y\n');
// enter mid text
await page.click('.o-content >> text=bc');
await K('Home'); await K('ArrowRight');
await K('Enter');
await expect('split', 'a\n  [x]x\n  [x]y\nb\nc\n[x]x\n[x]y\n');
// note
await K('Shift+Enter'); await T('note text'); await K('Shift+Enter');
const note = await page.$$eval('.o-note', (els) => els.map((e) => e.textContent).filter(Boolean));
console.log(note.includes('note text') ? 'ok   note' : `FAIL note ${note}`);
// bold toggle
await K('End'); await K('Shift+Home'); await K('Control+b');
await K('Escape');
const html = await page.innerHTML('.o-node:nth-child(3) > .o-row .o-content');
console.log(html.includes('o-bold') ? 'ok   bold' : `FAIL bold ${html}`);
// empty enter outdent
await page.click('.o-content >> text=y');
await K('End'); await K('Enter'); await K('Enter');
const o = await outline();
console.log(o.startsWith('a\n  [x]x\n  [x]y\n\n') ? 'ok   empty enter outdents' : `FAIL empty enter\n${o}`);
console.log('fails:', fails, 'errors:', errors);
await browser.close();
