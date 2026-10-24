// New feature checks (search views, replace, sort, inbox, settings, help, backup, code/latex rendering, hover toolbar).
//   apps/web/e2e/snapshot.sh && node apps/web/e2e/features.mjs
import { chromium } from 'playwright-core';
import { existsSync, readFileSync } from 'node:fs';

const LOCAL = `${process.env.HOME}/.cache/ms-playwright/chromium-1228/chrome-linux/chrome`;
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || (existsSync(LOCAL) ? LOCAL : undefined) });
const ctx = await browser.newContext({ viewport: { width: 1200, height: 800 }, acceptDownloads: true });
const page = await ctx.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
await page.goto(process.env.URL ?? 'http://localhost:5174/');
await page.waitForSelector('.o-editor');
const K = (k) => page.keyboard.press(k);
const T = (t) => page.keyboard.type(t, { delay: 10 });
const texts = () => page.$$eval('.o-items > .o-node > .o-row .o-content', (els) => els.map((e) => e.textContent));
let fails = 0;
const check = (label, ok, info = '') => {
  if (!ok) fails++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${ok ? '' : ` ${info}`}`);
};
const menu = async (sel, ...labels) => {
  await page.hover(sel);
  await page.click(`${sel} >> xpath=ancestor-or-self::*[contains(@class,"o-row")][1]/*[contains(@class,"o-menu-btn")]`);
  for (const l of labels) await page.click(`.menu .mi:has-text("${l}")`);
};

await page.click('.pane-head button[title^="New"]');
await page.click('text=New document');
await T('Fruits');
await K('Enter');
await page.waitForTimeout(150);
await page.click('.o-title .o-content');
await K('End');
await K('Enter');
for (const [i, t] of ['apple', 'banana #yellow', 'cherry'].entries()) {
  if (i) await K('Enter');
  await T(t);
}
await page.waitForTimeout(100);
check('typed items', JSON.stringify(await texts()) === JSON.stringify(['apple', 'banana #yellow', 'cherry']), JSON.stringify(await texts()));

// sort (item menu ▸ Sort ▸ Title Z→A on the document title)
await page.keyboard.press('Escape');
await menu('.o-title .o-content', 'Sort');
await page.click('.menu .mi:has-text("Title Z→A")');
await page.waitForTimeout(100);
check('sort Z→A', (await texts())[0] === 'cherry', JSON.stringify(await texts()));
await K('Control+z');
await page.waitForTimeout(100);
check('undo sort', (await texts())[0] === 'apple', JSON.stringify(await texts()));

// search & replace
await menu('.o-title .o-content', 'Search and replace');
await page.fill('.modal input[placeholder="Find"]', 'an');
check('replace count', (await page.textContent('.replace-count')).startsWith('2 matches'), await page.textContent('.replace-count'));
await page.fill('.modal input[placeholder="Replace with"]', 'AN');
await page.click('.modal button:has-text("Replace all")');
await page.waitForTimeout(150);
check('replace all', (await texts())[1].startsWith('bANANa'), JSON.stringify(await texts()));
await K('Control+z');

// flat search
await page.click('.o-content >> text=cherry');
await K('Control+f');
await T('an');
await K('Shift+Enter');
await page.waitForSelector('.results-view:not([hidden])');
check('flat search header', (await page.textContent('.results-head h2')).includes('Search results for "an" in "Fruits"'));
check('flat search count', (await page.textContent('.results-bar')).includes('Found 1 match'), await page.textContent('.results-bar'));
check('flat in url', (await page.evaluate(() => location.hash)).includes('f=1'));
await K('Escape');
await page.waitForTimeout(150);
check('esc back to doc', await page.isVisible('.o-editor') && !(await page.isVisible('.results-view')));

// global search
await K('Control+f');
await page.fill('.search-box input', '#yellow');
await K('Control+Enter');
await page.waitForSelector('.res-card');
check('global search card', (await page.textContent('.res-card .res-doc')) === 'Fruits');
await page.click('.res-item');
await page.waitForTimeout(150);
check('open result → item', (await page.evaluate(() => document.activeElement?.textContent)) === 'banana #yellow');

// inbox: set the document as inbox, capture two lines
await page.keyboard.press('Escape');
await menu('.o-title .o-content', 'Set as inbox');
await K('Control+Shift+I');
await page.waitForSelector('textarea.capture');
await page.fill('textarea.capture', 'captured one\ncaptured two');
await page.click('.modal button:has-text("Add")');
await page.waitForTimeout(150);
const after = await texts();
check('capture to inbox (end)', after.at(-2) === 'captured one' && after.at(-1) === 'captured two', JSON.stringify(after));

// settings: tabs, keymap
await K('Control+Comma');
await page.waitForSelector('.settings');
await page.click('.set-tabs .tab:has-text("Keymap")');
check('keymap tab lists commands', (await page.$$('.km-row')).length > 50);
await page.fill('.keymap > input', 'inbox');
check('keymap filter', (await page.$$('.km-row')).length >= 2);
await page.click('.set-tabs .tab:has-text("Preferences")');
await page.check('label:has-text("Unindent items in-place") input');
await page.click('.modal button:has-text("Done")');

// unindent in place: child b of a, then its following sibling c becomes b's child
await page.click('.o-content >> text=cherry');
await K('End');
await K('Enter');
await T('p');
await K('Enter');
await T('q');
await K('Tab');
await K('Enter');
await T('r');
await K('ArrowUp');
await K('Shift+Tab');
await page.waitForTimeout(100);
const qKids = await page.$$eval('.o-node', (ns) => {
  const q = ns.find((n) => n.querySelector(':scope > .o-row .o-content').textContent === 'q');
  return [...q.querySelectorAll(':scope > .o-kids > .o-node > .o-row .o-content')].map((e) => e.textContent);
});
check('unindent in place', JSON.stringify(qKids) === '["r"]', JSON.stringify(qKids));

// help side panel
await K('Control+Slash');
check('help panel opens', await page.isVisible('.help-panel'));
check('help lists unbound commands', (await page.textContent('.help-panel')).includes('Go to parent item'));
await K('Control+Slash');
check('help panel closes', !(await page.isVisible('.help-panel')));

// code block + latex rendering
await page.click('.o-content >> text=r');
await K('End');
await K('Enter');
await T('```js const a = 1 // hi```');
await K('Enter');
await T('$$x^2$$');
await K('Escape');
await page.waitForTimeout(800);
check('code highlighted', (await page.$$('.o-codeblock .o-hl-k')).length > 0 && (await page.$$('.o-codeblock .o-hl-c')).length > 0);
check('code copy button', (await page.$$('.o-codeblock .o-copy')).length === 1);
check('latex rendered', (await page.$$('.o-latex .katex')).length === 1);

// backup download
await page.click('header .icon-btn[title="Menu"]');
const [dl] = await Promise.all([page.waitForEvent('download'), page.click('.menu .mi:has-text("Download backup")')]);
const zip = readFileSync(await dl.path());
check('backup is a zip with the doc', zip.subarray(0, 2).toString() === 'PK' && zip.includes('Fruits.opml') && zip.includes('Fruits.txt'));

// bullet double-click: collapse all
const qNode = (fn) => page.$$eval('.o-node', (ns, f) => {
  const q = ns.find((n) => n.querySelector(':scope > .o-row .o-content')?.textContent === 'q');
  return new Function('q', f)(q);
}, fn);
const box = await qNode("const r = q.querySelector(':scope > .o-row > .o-bullet').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 };");
await page.mouse.dblclick(box.x, box.y);
await page.waitForTimeout(150);
check('bullet dblclick collapses subtree', await qNode("return q.classList.contains('o-collapsed')"));

// HTML paste (the Dynalist demo ignores synthetic HTML paste, so this is checked here): nested lists, bold
await page.click('.o-content >> text=cherry');
await K('End');
await K('Enter');
await page.evaluate(() => {
  const dt = new DataTransfer();
  dt.setData('text/html', '<ul><li>h1<ul><li>h2</li></ul></li><li><b>bold</b> x</li></ul>');
  dt.setData('text/plain', 'h1\n  h2\nbold x');
  document.activeElement.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
});
await page.waitForTimeout(150);
const pasted = await page.evaluate(() => {
  const items = window.__qa().items;
  const i = items.findIndex((x) => x.c === 'h1');
  return i < 0 ? null : items.slice(i, i + 3).map((x) => `${x.d}:${x.c}`);
});
check('html paste keeps nesting and bold', JSON.stringify(pasted?.map((x) => x.replace(/^\d+/, (d) => d - Number(pasted[0].split(':')[0])))) === JSON.stringify(['0:h1', '1:h2', '0:**bold** x']), JSON.stringify(pasted));

// desktop hover toolbar: off by default; on via Settings → Preferences
const qaItem = (c) => page.evaluate((c) => window.__qa().items.find((x) => x.c === c), c);
await K('Escape');
await page.hover('.o-content >> text=apple');
await page.waitForTimeout(150);
check('hover toolbar off by default', !(await page.isVisible('.hover-bar')));
await K('Control+Comma');
await page.waitForSelector('.settings');
await page.check('label:has-text("Show item toolbar on hover (desktop)") input');
await page.click('.modal button:has-text("Done")');
await page.hover('.o-content >> text=cherry');
await page.hover('.o-content >> text=apple');
await page.waitForSelector('.hover-bar:not([hidden])');
check('hover toolbar shows above the row', await page.evaluate(() => {
  const bar = document.querySelector('.hover-bar').getBoundingClientRect();
  const row = [...document.querySelectorAll('.o-items .o-row')].find((r) => r.querySelector('.o-content').textContent === 'apple').getBoundingClientRect();
  return bar.bottom <= row.top + 1 && bar.right <= row.right + 1;
}));
await page.click('.hover-bar button[title="Toggle checkbox"]');
await page.waitForTimeout(100);
check('hover toolbar: toggle checkbox', (await qaItem('apple'))?.cb === true, JSON.stringify(await qaItem('apple')));
// edit cherry, then act on apple from the bar: the caret stays in cherry
await page.click('.o-content >> text=cherry');
await K('End');
await page.hover('.o-content >> text=apple');
await page.waitForSelector('.hover-bar:not([hidden])');
await page.click('.hover-bar button[title="Set color label"]');
await page.click('.menu .sw.o-cl2');
await page.waitForTimeout(100);
check('hover toolbar: color label', await page.evaluate(() => [...document.querySelectorAll('.o-content.o-cl2')].some((e) => e.textContent === 'apple')));
check('hover toolbar keeps the caret', await page.evaluate(() => document.activeElement?.textContent === 'cherry' && getSelection().focusOffset === 6));
await page.hover('.o-content >> text=banana');
await page.waitForSelector('.hover-bar:not([hidden])');
await page.click('.hover-bar button[title="Indent"]');
await page.waitForTimeout(100);
check('hover toolbar: indent', (await qaItem('banana #yellow'))?.d === (await qaItem('apple')).d + 1, JSON.stringify(await qaItem('banana #yellow')));
check('hover toolbar follows the row', await page.isVisible('.hover-bar'));
await page.click('.hover-bar button[title="Unindent"]');
await page.waitForTimeout(100);
check('hover toolbar: unindent', (await qaItem('banana #yellow'))?.d === (await qaItem('apple')).d);
await page.mouse.move(5, 790);
await page.waitForTimeout(400);
check('hover toolbar hides on leave', !(await page.isVisible('.hover-bar')));

check('no page errors', errors.length === 0, errors.join('\n'));
await browser.close();
console.log(fails ? `${fails} FAILED` : 'all ok');
process.exit(fails ? 1 : 0);
