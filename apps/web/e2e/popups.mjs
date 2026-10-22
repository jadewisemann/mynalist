// In-editor popups: tag / item-link autocomplete, date picker, recurring check-off.
import { chromium } from 'playwright-core';
const exe = `${process.env.HOME}/.cache/ms-playwright/chromium-1228/chrome-linux/chrome`;
const URL = process.env.URL ?? 'http://localhost:5173/';
const browser = await chromium.launch({ executablePath: exe });
const errors = [];
let fails = 0;
function check(label, ok, info = '') {
  if (!ok) fails++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${ok ? '' : ` ${info}`}`);
}
const pad = (n) => String(n).padStart(2, '0');
const ymd = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const addDays = (n) => {
  const d = new Date();
  d.setDate(d.getDate() + n);
  return d;
};

async function setup(viewport) {
  const ctx = await browser.newContext({ viewport });
  const page = await ctx.newPage();
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(URL);
  await page.waitForSelector('.o-editor');
  return page;
}

async function newDoc(page, title) {
  if (await page.evaluate(() => matchMedia('(max-width: 760px)').matches)) await page.click('.topbar .icon-btn[title="Toggle sidebar"]');
  await page.click('.pane-head button');
  await page.click('text=New document');
  await page.keyboard.type(title);
  await page.keyboard.press('Enter');
  await page.waitForTimeout(200);
}

const raw = (page) => page.evaluate(() => {
  const el = document.querySelector('.o-editing');
  return el ? el.textContent : null;
});
const visible = (page, sel) => page.evaluate((s) => {
  const el = document.querySelector(s);
  return !!el && el.isConnected && el.getBoundingClientRect().height > 0;
}, sel);
const inViewport = (page, sel) => page.evaluate((s) => {
  const r = document.querySelector(s)?.getBoundingClientRect();
  if (!r) return 'missing';
  const ok = r.left >= 0 && r.top >= 0 && r.right <= innerWidth + 0.5 && r.bottom <= innerHeight + 0.5;
  return ok ? true : `${JSON.stringify({ l: r.left, t: r.top, r: r.right, b: r.bottom })} vs ${innerWidth}x${innerHeight}`;
}, sel);
// per top-level item: checked flag + date body (raw text when being edited)
const nodeDates = (page) => page.$$eval('.o-items > .o-node', (els) => els.map((n) => {
  const c = n.querySelector(':scope > .o-row .o-content');
  const d = c.classList.contains('o-editing') ? /!\(([^)]*)\)/.exec(c.textContent)?.[1] : c.querySelector('.o-date')?.dataset.date;
  return { ck: n.classList.contains('o-checked'), d: d ?? null, text: c.textContent };
}));
const acItems = (page) => page.$$eval('.ac-popup .ac-item .ac-label', (els) => els.map((e) => e.textContent));
const dates = (page) => page.$$eval('.o-items .o-date', (els) => els.map((e) => e.dataset.date));

// ---------------------------------------------------------------- desktop
{
  const page = await setup({ width: 1100, height: 760 });
  const K = (k) => page.keyboard.press(k);
  const T = (t) => page.keyboard.type(t);
  await newDoc(page, 'Popups');
  await page.click('.o-editor', { position: { x: 300, y: 120 } });

  // ---- tags
  await T('alpha #project one'); await K('Enter');
  await T('beta #proto #project'); await K('Enter');
  await T('gamma @me'); await K('Enter');
  await T('x #pr');
  let items = await acItems(page);
  check('tag popup lists matches by count', JSON.stringify(items) === '["#project","#proto"]', JSON.stringify(items));
  await K('ArrowDown'); await K('Enter');
  check('enter picks tag + space', (await raw(page)) === 'x #proto ', await raw(page));
  check('popup closed after pick', !(await visible(page, '.ac-popup')));
  await T('#p'); await K('Tab');
  check('tab picks', (await raw(page)) === 'x #proto #project ', await raw(page));
  await T('@');
  items = await acItems(page);
  check('@ lists @ tags only', JSON.stringify(items) === '["@me"]', JSON.stringify(items));
  await K('Escape');
  check('esc closes tag popup, keeps editing', !(await visible(page, '.ac-popup')) && (await raw(page)) === 'x #proto #project @');
  await K('Backspace');
  await T('#zz');
  check('no matches → no popup', !(await visible(page, '.ac-popup')));
  await K('Backspace'); await K('Backspace');
  check('popup reopens on narrowing back', JSON.stringify(await acItems(page)) === '["#project","#proto"]', JSON.stringify(await acItems(page)));
  await T(' ');
  check('space closes', !(await visible(page, '.ac-popup')));
  await K('Backspace'); await K('Backspace');
  await T('word#');
  check('no trigger mid-word', !(await visible(page, '.ac-popup')));
  await K('Control+a');
  await K('Backspace');
  // ---- global tags pref: #idea lives in the Welcome doc
  await T('#ide');
  check('other docs\' tags hidden by default', !(await visible(page, '.ac-popup')));
  await K('Escape'); await K('Escape');
  await page.click('.topbar .icon-btn[title="Settings"]');
  await page.click('label.field:has-text("Auto-complete tags from all docs") input');
  await page.click('.modal .foot .btn.primary');
  await page.click('.o-content >> text=gamma');
  await K('End'); await K('Enter');
  await T('#ide');
  items = await acItems(page);
  check('global_tag_autocomplete shows other docs', items.includes('#idea'), JSON.stringify(items));
  await K('Escape');
  await K('Control+a'); await K('Backspace');

  // ---- item links
  await T('see [[gam');
  items = await acItems(page);
  check('link popup fuzzy lists items', items[0] === 'gamma @me', JSON.stringify(items));
  const sub = await page.textContent('.ac-popup .ac-item .ac-sub');
  check('link popup shows doc path', sub === 'Popups', sub);
  await K('Enter');
  const linkRaw = await raw(page);
  check('link inserted', /^see \[gamma @me\]\(#d=[^&]+&z=[^)]+\)$/.test(linkRaw), linkRaw);
  const selText = await page.evaluate(() => getSelection().toString());
  check('title text selected', selText === 'gamma @me', selText);
  await K('End'); await K('Escape');
  const internal = await page.$$eval('.o-items .o-link-internal', (els) => els.length);
  check('renders as internal link', internal === 1);
  await page.click('.o-content >> text=see');
  await K('End'); await K('Enter');
  await T('[[zzzqqq');
  check('link popup hidden with no hits', !(await visible(page, '.ac-popup')));
  await T(']');
  await K('Control+a'); await K('Backspace');

  // ---- date picker via "!"
  await T('task !');
  check('date picker opens on !', await visible(page, '.dp'));
  const sel0 = await page.getAttribute('.dp .dp-day.sel', 'data-date');
  check('defaults to today', sel0 === ymd(new Date()), sel0);
  await K('ArrowRight');
  check('arrow moves day', (await page.getAttribute('.dp .dp-day.sel', 'data-date')) === ymd(addDays(1)));
  await K('Enter');
  check('enter writes token', (await raw(page)) === `task !(${ymd(addDays(1))})`, await raw(page));
  check('picker closed', !(await visible(page, '.dp')));
  await K('Escape');
  let chip = await page.textContent('.o-items .o-date');
  check('chip says Tomorrow', chip === 'Tomorrow', chip);

  // typing `!(` by hand dismisses the picker
  await page.click('.o-content >> text=task');
  await K('End'); await K('Enter');
  await T('pay !(2026-10-01 | 1m)');
  check('typing after ! dismisses picker', !(await visible(page, '.dp')) && (await raw(page)) === 'pay !(2026-10-01 | 1m)', await raw(page));
  // Esc on a fresh `!` keeps the text and the caret
  await K('Enter');
  await T('esc !');
  await K('Escape');
  check('esc cancels picker, keeps !', !(await visible(page, '.dp')) && (await raw(page)) === 'esc !', await raw(page));
  await T('x');
  check('caret still in item after esc', (await raw(page)) === 'esc !x');
  await K('Escape');

  // ---- click chip → edit: time + weekly repeat
  await page.click('.o-items .o-date >> text=Tomorrow');
  check('chip click opens picker', await visible(page, '.dp'));
  check('existing date selected', (await page.getAttribute('.dp .dp-day.sel', 'data-date')) === ymd(addDays(1)));
  check('delete button for existing date', await visible(page, '.dp .dp-delete'));
  await page.fill('.dp .dp-time', '3pm');
  await page.selectOption('.dp .dp-repeat', 'w');
  await page.focus('.dp .dp-time');
  await K('Enter');
  let ds = await dates(page);
  check('time + weekly written', ds.includes(`${ymd(addDays(1))} 15:00 | 1w`), JSON.stringify(ds));

  // invalid time
  await page.click('.o-items .o-date[data-date*="15:00"]');
  await page.fill('.dp .dp-time', '25:99');
  await page.click('.dp .dp-done');
  check('invalid time shows error', (await visible(page, '.dp .dp-err')) && (await visible(page, '.dp')));
  await K('Escape');
  check('esc closes without change', !(await visible(page, '.dp')) && (await dates(page)).includes(`${ymd(addDays(1))} 15:00 | 1w`));

  // custom recurrence modal: every 2 weeks Mon + Wed, from completion
  await page.click('.o-items .o-date[data-date*="15:00"]');
  await page.selectOption('.dp .dp-repeat', 'custom');
  check('custom modal opens', await visible(page, '.modal .dp-custom'));
  await page.fill('.modal .dp-n', '2');
  await page.selectOption('.modal .dp-unit', 'w');
  for (const b of await page.$$('.modal .dp-wd-toggle.on')) await b.click();
  await page.click('.modal .dp-wd-toggle[data-iso="1"]');
  await page.click('.modal .dp-wd-toggle[data-iso="3"]');
  await page.click('.modal .foot .btn.primary');
  check('modal closed, picker open', !(await visible(page, '.modal')) && (await visible(page, '.dp')));
  const opt = await page.$eval('.dp .dp-repeat', (s) => s.selectedOptions[0].textContent);
  check('repeat shows custom description', opt === 'Every 2 weeks on Mon, Wed', opt);
  await page.check('.dp .dp-fromc');
  await page.click('.dp .dp-done');
  ds = await dates(page);
  check('custom recurrence written', ds.includes(`${ymd(addDays(1))} 15:00 | ~2w13`), JSON.stringify(ds));

  // range: toggle, click end date
  await page.click('.o-items .o-date[data-date*="~2w13"]');
  await page.check('.dp .dp-range');
  const endDay = ymd(addDays(3));
  const startDay = ymd(addDays(1));
  if (!(await page.$(`.dp .dp-day[data-date="${endDay}"]`))) await page.click('.dp .dp-nav[aria-label="Next month"]');
  await page.click(`.dp .dp-day[data-date="${endDay}"]`);
  await page.fill('.dp .dp-end-time', '17:30');
  await page.click('.dp .dp-done');
  ds = await dates(page);
  check('range written', ds.includes(`${startDay} 15:00 - ${endDay} 17:30 | ~2w13`), JSON.stringify(ds));

  // delete date
  await page.click('.o-items .o-date[data-date*="~2w13"]');
  await page.click('.dp .dp-delete');
  ds = await dates(page);
  check('delete removes the date', !ds.some((d) => d.includes('~2w13')), JSON.stringify(ds));
  const taskText = await page.$$eval('.o-items .o-content', (els) => els.map((e) => e.textContent).find((t) => t.startsWith('task')));
  check('delete leaves text', taskText === 'task', JSON.stringify(taskText));

  // ---- recurring check-off (default new_instance)
  await page.click('.o-content >> text=pay');
  await K('Control+Enter');
  const rows = (await nodeDates(page)).filter((x) => x.d && x.d.includes('| 1m')).map(({ ck, d }) => ({ ck, d }));
  check('new_instance: checked copy above + advanced original',
    JSON.stringify(rows) === JSON.stringify([{ ck: true, d: '2026-10-01 | 1m' }, { ck: false, d: '2026-11-01 | 1m' }]), JSON.stringify(rows));
  await K('Control+z');
  const undone = (await nodeDates(page)).filter((x) => x.d && x.d.includes('| 1m')).map(({ ck, d }) => ({ ck, d }));
  check('undo restores single item', JSON.stringify(undone) === '[{"ck":false,"d":"2026-10-01 | 1m"}]', JSON.stringify(undone));

  // ---- add-date command on an item without a date (via mobile bar handler path)
  await page.click('.o-content >> text=gamma');
  await K('End');
  await page.evaluate(() => [...document.querySelectorAll('.mobile-bar button')].find((b) => b.textContent === '◷')?.click());
  const viaCmd = await visible(page, '.dp');
  check('add-date opens picker', viaCmd);
  if (viaCmd) {
    await page.click('.dp .dp-done');
    const g = (await nodeDates(page)).find((x) => x.text.startsWith('gamma'));
    check('add-date inserts token', g?.text === `gamma @me !(${ymd(new Date())})`, JSON.stringify(g));
  }

  // ---- recurrence_check_behavior = update
  await page.click('.topbar .icon-btn[title="Settings"]');
  await page.selectOption('label.field:has-text("Recurring date") select', 'update');
  await page.click('.modal .foot .btn.primary');
  await page.click('.o-content >> text=pay');
  await K('Control+Enter');
  const upd = (await nodeDates(page)).filter((x) => x.d && x.d.includes('| 1m')).map(({ ck, d }) => ({ ck, d }));
  check('update: only advances', JSON.stringify(upd) === JSON.stringify([{ ck: false, d: '2026-11-01 | 1m' }]), JSON.stringify(upd));
  await page.close();
}

// ---------------------------------------------------------------- mobile
{
  const page = await setup({ width: 390, height: 700 });
  const K = (k) => page.keyboard.press(k);
  const T = (t) => page.keyboard.type(t);
  await newDoc(page, 'Mobile popups');
  if (await page.evaluate(() => document.querySelector('.app').classList.contains('side-open'))) await page.click('.scrim', { position: { x: 375, y: 400 } });
  await page.click('.o-editor', { position: { x: 200, y: 100 } });
  await T('one #mobile'); await K('Enter');
  await T('two #m');
  check('mobile: tag popup visible', await visible(page, '.ac-popup'));
  check('mobile: tag popup in viewport', (await inViewport(page, '.ac-popup')) === true, await inViewport(page, '.ac-popup'));
  await K('Escape');
  // push the item toward the bottom so the picker has to flip above
  for (let i = 0; i < 12; i++) { await K('Enter'); await T(`line ${i}`); }
  await page.click('.mobile-bar button:has-text("◷")');
  check('mobile: add-date button opens picker', await visible(page, '.dp'));
  check('mobile: picker in viewport', (await inViewport(page, '.dp')) === true, await inViewport(page, '.dp'));
  const above = await page.evaluate(() => {
    const dp = document.querySelector('.dp').getBoundingClientRect();
    const cur = document.querySelector('.o-editing').getBoundingClientRect();
    const flipped = document.querySelector('.dp').classList.contains('pop-above');
    return (dp.bottom <= cur.top + 2 || dp.top >= cur.bottom - 2) && (cur.top > innerHeight / 2 ? flipped : true) ? true : JSON.stringify({ dp, cur, flipped });
  });
  check('mobile: picker flips above near the bottom, not covering the line', above === true, above);
  await K('Escape');
  await page.close();
}

console.log('fails:', fails, 'errors:', errors);
await browser.close();
process.exit(fails || errors.length ? 1 : 0);
