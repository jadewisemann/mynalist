import { chromium } from 'playwright-core';
const exe = `${process.env.HOME}/.cache/ms-playwright/chromium-1228/chrome-linux/chrome`;
const browser = await chromium.launch({ executablePath: exe });
const page = await (await browser.newContext({ viewport: { width: 1100, height: 760 } })).newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
await page.goto(process.env.URL ?? 'http://localhost:5174/');
await page.waitForSelector('.o-editor');
const K = (k) => page.keyboard.press(k), T = (t) => page.keyboard.type(t);
const outline = () => page.evaluate(() => {
  const walk = (el, d) => [...el.children].map((n) => '  '.repeat(d) + n.querySelector(':scope > .o-row .o-content').textContent + '\n' + walk(n.querySelector(':scope > .o-kids'), d + 1)).join('');
  return walk(document.querySelector('.o-items'), 0);
});
await page.click('.pane-head button[title^="New"]'); await page.click('text=New document'); await T('Drag'); await K('Enter');
await page.waitForTimeout(200);
await page.click('.o-editor', { position: { x: 300, y: 120 } });
await T('one'); await K('Enter'); await T('two'); await K('Enter'); await T('three'); await K('Escape');
const box = (text, sel) => page.evaluate(([text, sel]) => {
  const n = [...document.querySelectorAll('.o-node')].find((n) => n.querySelector(':scope > .o-row .o-content').textContent === text);
  const r = (sel ? n.querySelector(sel) : n.querySelector(':scope > .o-row')).getBoundingClientRect();
  return { x: r.x, y: r.y, width: r.width, height: r.height };
}, [text, sel]);
const bullet = (text) => box(text, ':scope > .o-row > .o-bullet');
const row = (text) => box(text);
const nodeAttr = (text, sel, fn) => page.evaluate(([text, sel]) => {
  const n = [...document.querySelectorAll('.o-node')].find((n) => n.querySelector(':scope > .o-row .o-content').textContent === text);
  return sel ? n.querySelector(sel).className : n.className;
}, [text, sel]);
// drag "three" above "one"
let b = await bullet('three'), r = await row('one');
await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
await page.mouse.down();
await page.mouse.move(b.x + 10, b.y - 10, { steps: 3 });
await page.mouse.move(r.x + 60, r.y + 3, { steps: 5 });
await page.mouse.up();
let o = await outline();
console.log(o === 'three\none\ntwo\n' ? 'ok   drag before' : `FAIL drag before\n${o}`);
// drag "two" inside "one" (far right, lower half)
b = await bullet('two'); r = await row('one');
await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
await page.mouse.down();
await page.mouse.move(b.x + 10, b.y + 5, { steps: 3 });
await page.mouse.move(r.x + r.width - 20, r.y + r.height - 4, { steps: 5 });
await page.mouse.up();
o = await outline();
console.log(o === 'three\none\n  two\n' ? 'ok   drag inside' : `FAIL drag inside\n${o}`);
// context menu: right-click bullet → color
b = await bullet('three');
await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2, { button: 'right' });
await page.waitForSelector('.menu');
await page.click('.menu .sw.o-cl4');
const cls = await nodeAttr('three', ':scope > .o-row .o-content');
console.log(cls.includes('o-cl4') ? 'ok   menu color' : `FAIL menu color ${cls}`);
// bullet click still collapses
b = await bullet('one');
await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2);
const collapsed = (await nodeAttr('one')).includes('o-collapsed');
console.log(collapsed ? 'ok   bullet click collapse' : 'FAIL bullet click');
console.log('errors', errors);
await page.screenshot({ path: '/tmp/drag.png' });
await browser.close();
