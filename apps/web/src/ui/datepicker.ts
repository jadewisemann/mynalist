// Date picker popover (docs/03-features-ui.md §3): month calendar, optional
// time, optional range, repeat dropdown + custom recurrence modal, "from
// completion". Produces a DateSpec; the caller writes the `!(…)` token.

import {
  type DateSpec, type DateTime, MONTH_NAMES, type Recurrence, type RecurUnit, WEEKDAY_NAMES, daysInMonth,
  describeRecurrence, formatTime,
} from '@outliner/core';
import { h } from './dom.ts';
import { modal } from './overlay.ts';
import { placePopup } from './position.ts';

export interface DatePickerOptions {
  /** existing date being edited, or null for a new one (today) */
  spec: DateSpec | null;
  /** first day of week, 0 = Sunday */
  startDay: number;
  hour12: boolean;
  anchor: () => DOMRect;
  onDone(spec: DateSpec): void;
  /** present when editing an existing date */
  onDelete?: () => void;
  /** Esc / Cancel button (not outside clicks) */
  onCancel?: () => void;
  /** always called once when the popover goes away */
  onClose(): void;
}

interface Day {
  year: number;
  month: number;
  day: number;
}

type TimeValue = { hour: number; minute: number } | 'allday';

const TIME_ERROR = 'Please enter a valid time format like 22:42';

/** Parse a typed time: "22:42", "9", "3pm", "3:30 pm", "1530", "All day", "Now". Null = invalid. */
export function parseTimeInput(raw: string, now = new Date()): TimeValue | null {
  const s = raw.trim().toLowerCase().replace(/\./g, '');
  if (!s || s === 'all day' || s === 'allday') return 'allday';
  if (s === 'now') return { hour: now.getHours(), minute: now.getMinutes() };
  if (s === 'noon') return { hour: 12, minute: 0 };
  if (s === 'midnight') return { hour: 0, minute: 0 };
  const m = /^(\d{1,2})(?:(?::|h)?(\d{2}))?\s*(am|pm|a|p)?$/.exec(s);
  if (!m) return null;
  let hour = +m[1]!;
  const minute = m[2] ? +m[2] : 0;
  if (minute > 59) return null;
  if (m[3]) {
    if (hour < 1 || hour > 12) return null;
    hour = (hour % 12) + (m[3].startsWith('p') ? 12 : 0);
  } else if (hour > 23) return null;
  return { hour, minute };
}

const dayOf = (d: Date): Day => ({ year: d.getFullYear(), month: d.getMonth() + 1, day: d.getDate() });
const toJs = (d: Day) => new Date(d.year, d.month - 1, d.day);
const addDays = (d: Day, n: number): Day => dayOf(new Date(d.year, d.month - 1, d.day + n));
const cmp = (a: Day, b: Day) => toJs(a).getTime() - toJs(b).getTime();
const same = (a: Day, b: Day) => cmp(a, b) === 0;
const iso = (d: Day) => toJs(d).getDay() || 7;
const key = (d: Day) => `${d.year}-${String(d.month).padStart(2, '0')}-${String(d.day).padStart(2, '0')}`;

function addMonths(d: Day, n: number): Day {
  const idx = d.year * 12 + d.month - 1 + n;
  const year = Math.floor(idx / 12);
  const month = (idx % 12) + 1;
  return { year, month, day: Math.min(d.day, daysInMonth(year, month)) };
}

function ordinal(n: number): string {
  const v = n % 100;
  if (v >= 11 && v <= 13) return `${n}th`;
  return n + (['th', 'st', 'nd', 'rd'][n % 10] ?? 'th');
}

const timeText = (dt: DateTime | undefined, hour12: boolean) =>
  dt && dt.hour !== undefined ? formatTime(dt.hour, dt.minute ?? 0, hour12) : '';

const UNIT_LABELS: [RecurUnit, string][] = [['d', 'Day'], ['w', 'Week'], ['m', 'Month'], ['y', 'Year']];

export class DatePicker {
  readonly el = h('div.dp', { tabindex: '-1', role: 'dialog', 'aria-label': 'Date picker' });
  private start: Day;
  private end: Day;
  private range: boolean;
  /** which endpoint the next calendar click / arrow key sets */
  private picking: 'start' | 'end' = 'start';
  private view: Day;
  private recur: Recurrence | null;
  private closed = false;
  private modalOpen = false;

  private grid = h('div.dp-grid');
  private title = h('span.dp-title');
  private summary = h('div.dp-summary');
  private time = h('input.dp-time', { placeholder: 'All day', list: 'dp-time-opts', autocomplete: 'off', spellcheck: false });
  private endTime = h('input.dp-time.dp-end-time', { placeholder: 'All day', list: 'dp-time-opts', autocomplete: 'off', spellcheck: false });
  private err = h('div.dp-err', { hidden: true });
  private rangeBox = h('input.dp-range', { type: 'checkbox' });
  private endRow: HTMLElement;
  private repeat = h('select.dp-repeat');
  private fromC = h('input.dp-fromc', { type: 'checkbox' });

  private o: DatePickerOptions;

  constructor(o: DatePickerOptions) {
    this.o = o;
    const s = o.spec;
    this.start = s ? { year: s.start.year, month: s.start.month, day: s.start.day } : dayOf(new Date());
    this.end = s?.end ? { year: s.end.year, month: s.end.month, day: s.end.day } : this.start;
    this.range = !!s?.end;
    this.view = { ...this.start, day: 1 };
    this.recur = s?.recur ? { ...s.recur, fromCompletion: false } : null;
    this.fromC.checked = !!s?.recur?.fromCompletion;
    this.time.value = timeText(s?.start, o.hour12);
    this.endTime.value = timeText(s?.end, o.hour12);
    this.rangeBox.checked = this.range;

    const nav = (d: number, label: string) =>
      h('button.dp-nav', { type: 'button', 'aria-label': label, onclick: () => this.shiftView(d) }, d < 0 ? '‹' : '›');
    const opts = h('datalist', { id: 'dp-time-opts' }, h('option', { value: 'All day' }), h('option', { value: 'Now' }));
    this.endRow = h('label.dp-row.dp-end-row', {}, h('span', {}, 'End time'), this.endTime);
    const help = h('span.dp-help', { title: 'Turn this on, then click a start date and an end date in the calendar.' }, '?');
    const fromHelp = h('span.dp-help', { title: 'The next date is counted from the day you check the item off, not from the due date.' }, '?');
    this.el.append(
      h('div.dp-head', {}, nav(-1, 'Previous month'), this.title, nav(1, 'Next month')),
      this.grid,
      this.summary,
      h('div.dp-fields', {},
        h('label.dp-row', {}, h('span', {}, 'Time'), this.time),
        this.err,
        h('label.dp-check', {}, this.rangeBox, h('span', {}, 'Create date range'), help),
        this.endRow,
        h('label.dp-row', {}, h('span', {}, 'Repeat'), this.repeat),
        h('label.dp-check', {}, this.fromC, h('span', {}, 'From completion'), fromHelp),
      ),
      h('div.dp-foot', {},
        o.onDelete ? h('button.btn.dp-delete', { type: 'button', onclick: () => this.remove() }, 'Delete date') : null,
        h('span.spacer'),
        h('button.btn.dp-cancel', { type: 'button', onclick: () => this.cancel() }, 'Cancel'),
        h('button.btn.primary.dp-done', { type: 'button', onclick: () => this.done() }, 'Done'),
      ),
      opts,
    );

    // keep the editor's caret while clicking around (inputs still take focus)
    this.el.addEventListener('mousedown', (e) => {
      const t = e.target as HTMLElement;
      if (!t.closest('input, select, label')) e.preventDefault();
    });
    this.el.addEventListener('keydown', (e) => {
      if (e.isComposing) return;
      if (this.handleKey(e)) {
        e.preventDefault();
        e.stopPropagation();
      }
    });
    for (const inp of [this.time, this.endTime]) {
      inp.addEventListener('input', () => (this.err.hidden = true));
      inp.addEventListener('change', () => this.normalizeTime(inp));
    }
    this.rangeBox.addEventListener('change', () => {
      this.range = this.rangeBox.checked;
      this.picking = this.range ? 'end' : 'start';
      if (cmp(this.end, this.start) < 0) this.end = this.start;
      this.render();
    });
    this.repeat.addEventListener('change', () => this.onRepeat());
    this.fromC.addEventListener('change', () => this.render());
  }

  open() {
    this.render();
    document.body.append(this.el);
    this.reposition();
  }

  close() {
    if (this.closed) return;
    this.closed = true;
    this.el.remove();
    this.o.onClose();
  }

  reposition() {
    if (!this.closed && this.el.isConnected) placePopup(this.el, this.o.anchor());
  }

  /** Keys from the editor (via interceptor) or from inside the popover. */
  handleKey(e: KeyboardEvent): boolean {
    if (this.closed || this.modalOpen) return false;
    const t = e.target as HTMLElement | null;
    const inField = !!t && this.el.contains(t) && /^(INPUT|SELECT)$/.test(t.tagName) && (t as HTMLInputElement).type !== 'checkbox';
    if (e.key === 'Escape') {
      this.cancel();
      return true;
    }
    if (e.key === 'Enter' && !e.shiftKey && !e.altKey) {
      this.done();
      return true;
    }
    if (inField || e.ctrlKey || e.metaKey || e.altKey) return false;
    const moves: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };
    if (e.key in moves && !e.shiftKey) {
      this.moveSelected(moves[e.key]!);
      return true;
    }
    if (e.key === 'PageUp' || e.key === 'PageDown') {
      const d = e.key === 'PageUp' ? -1 : 1;
      if (this.range && this.picking === 'end') this.end = addMonths(this.end, d);
      else this.start = addMonths(this.start, d);
      this.syncAfterMove();
      return true;
    }
    if (e.key === 'Tab' && !this.el.contains(t)) {
      // from the editor: jump into the time field instead of indenting
      this.time.focus();
      this.time.select();
      return true;
    }
    return false;
  }

  // ---- state changes ----------------------------------------------------------------------

  private shiftView(d: number) {
    this.view = addMonths(this.view, d);
    this.render();
  }

  private moveSelected(n: number) {
    if (this.range && this.picking === 'end') {
      this.end = addDays(this.end, n);
      if (cmp(this.end, this.start) < 0) this.end = this.start;
    } else {
      const span = Math.round((toJs(this.end).getTime() - toJs(this.start).getTime()) / 86400000);
      this.start = addDays(this.start, n);
      this.end = addDays(this.start, Math.max(0, span));
    }
    this.syncAfterMove();
  }

  private syncAfterMove() {
    const focus = this.range && this.picking === 'end' ? this.end : this.start;
    this.view = { ...focus, day: 1 };
    this.render();
  }

  private clickDay(d: Day) {
    if (this.range && this.picking === 'end') {
      if (cmp(d, this.start) < 0) this.start = d;
      else {
        this.end = d;
        this.picking = 'start';
      }
    } else {
      this.start = d;
      if (cmp(this.end, d) < 0 || !this.range) this.end = d;
      if (this.range) this.picking = 'end';
    }
    if (d.month !== this.view.month || d.year !== this.view.year) this.view = { ...d, day: 1 };
    this.render();
  }

  private normalizeTime(inp: HTMLInputElement) {
    const v = parseTimeInput(inp.value);
    if (v === null) {
      this.err.textContent = TIME_ERROR;
      this.err.hidden = false;
      return;
    }
    inp.value = v === 'allday' ? '' : formatTime(v.hour, v.minute, this.o.hour12);
  }

  // ---- repeat -----------------------------------------------------------------------------

  private presets(): [string, string, Recurrence | null][] {
    const s = this.start;
    const r = (unit: RecurUnit): Recurrence => ({ n: 1, unit, weekdays: [], fromCompletion: false });
    return [
      ['', 'Does not repeat', null],
      ['d', 'Daily', r('d')],
      ['w', `Weekly on ${WEEKDAY_NAMES[toJs(s).getDay()]}`, r('w')],
      ['m', `Monthly on the ${ordinal(s.day)}`, r('m')],
      ['y', `Annually on ${MONTH_NAMES[s.month - 1]} ${s.day}`, r('y')],
    ];
  }

  /** Select value for the current recurrence ('cur' = non-preset). */
  private repeatValue(): string {
    const r = this.recur;
    if (!r) return '';
    if (r.n !== 1) return 'cur';
    if (r.unit === 'w') return !r.weekdays.length || (r.weekdays.length === 1 && r.weekdays[0] === iso(this.start)) ? 'w' : 'cur';
    return r.unit;
  }

  private onRepeat() {
    const v = this.repeat.value;
    if (v === 'custom') {
      this.repeat.value = this.repeatValue();
      this.customModal();
      return;
    }
    if (v === 'cur') return;
    this.recur = this.presets().find(([k]) => k === v)?.[2] ?? null;
    this.render();
  }

  private customModal() {
    const cur = this.recur ?? { n: 1, unit: 'w' as RecurUnit, weekdays: [iso(this.start)], fromCompletion: false };
    const n = h('input.dp-n', { type: 'number', min: '1', value: String(cur.n) });
    const unit = h('select.dp-unit', {}, ...UNIT_LABELS.map(([u, l]) => h('option', { value: u, selected: u === cur.unit }, l)));
    const picked = new Set(cur.unit === 'w' && cur.weekdays.length ? cur.weekdays : [iso(this.start)]);
    const toggles = [1, 2, 3, 4, 5, 6, 7].map((d) => {
      const b = h('button.dp-wd-toggle', { type: 'button', 'data-iso': String(d), title: WEEKDAY_NAMES[d % 7] }, 'MTWTFSS'[d - 1]!);
      const paint = () => b.classList.toggle('on', picked.has(d));
      b.addEventListener('click', () => {
        if (picked.has(d)) picked.delete(d);
        else picked.add(d);
        paint();
      });
      paint();
      return b;
    });
    const days = h('div.dp-row.dp-weekdays-row', {}, h('span', {}, 'Repeat on'), h('div.dp-weekdays', {}, ...toggles));
    const sync = () => (days.hidden = unit.value !== 'w');
    unit.addEventListener('change', sync);
    sync();
    const err = h('div.dp-err', { hidden: true }, 'Enter a whole number of 1 or more');
    const body = h('div.dp-custom', {},
      h('div.dp-row', {}, h('span', {}, 'Repeat every'), h('div.dp-every', {}, n, unit)),
      days, err);
    this.modalOpen = true;
    modal('Custom recurrence', body, [
      { label: 'Cancel' },
      {
        label: 'Done',
        primary: true,
        run: () => {
          const num = Number(n.value);
          if (!Number.isInteger(num) || num < 1) {
            err.hidden = false;
            return false;
          }
          const u = unit.value as RecurUnit;
          const wd = u === 'w' ? [...picked].sort((a, b) => a - b) : [];
          // a single weekday equal to the start's is the same as none
          this.recur = { n: num, unit: u, weekdays: u === 'w' && !(wd.length === 1 && wd[0] === iso(this.start)) ? wd : [], fromCompletion: false };
          return true;
        },
      },
    ]);
    const ov = body.closest('.overlay');
    const mo = new MutationObserver(() => {
      if (ov && ov.isConnected) return;
      mo.disconnect();
      this.modalOpen = false;
      this.render();
      this.el.focus({ preventScroll: true });
    });
    mo.observe(document.body, { childList: true });
  }

  // ---- finish -----------------------------------------------------------------------------

  private done() {
    const now = new Date();
    const st = parseTimeInput(this.time.value, now);
    const et = this.range ? parseTimeInput(this.endTime.value, now) : 'allday';
    if (st === null || et === null) {
      this.err.textContent = TIME_ERROR;
      this.err.hidden = false;
      (st === null ? this.time : this.endTime).focus();
      return;
    }
    const withTime = (d: Day, t: TimeValue): DateTime => (t === 'allday' ? { ...d } : { ...d, hour: t.hour, minute: t.minute });
    let a = this.start;
    let b = this.end;
    if (cmp(b, a) < 0) [a, b] = [b, a];
    const spec: DateSpec = { start: withTime(a, st) };
    if (this.range) spec.end = withTime(b, et);
    if (this.recur) spec.recur = { ...this.recur, fromCompletion: this.fromC.checked };
    this.close();
    this.o.onDone(spec);
  }

  private cancel() {
    this.close();
    this.o.onCancel?.();
  }

  private remove() {
    this.close();
    this.o.onDelete?.();
  }

  // ---- render -----------------------------------------------------------------------------

  private render() {
    const v = this.view;
    this.title.textContent = `${MONTH_NAMES[v.month - 1]} ${v.year}`;
    const today = dayOf(new Date());
    const sd = ((this.o.startDay % 7) + 7) % 7;
    this.grid.textContent = '';
    for (let i = 0; i < 7; i++) this.grid.append(h('span.dp-wd', {}, WEEKDAY_NAMES[(sd + i) % 7]!.slice(0, 2)));
    const first: Day = { year: v.year, month: v.month, day: 1 };
    const lead = (toJs(first).getDay() - sd + 7) % 7;
    let d = addDays(first, -lead);
    for (let i = 0; i < 42; i++) {
      const day = d;
      const cls = ['dp-day'];
      if (day.month !== v.month) cls.push('other');
      if (same(day, today)) cls.push('today');
      if (same(day, this.start)) cls.push('sel');
      if (this.range) {
        if (same(day, this.end)) cls.push('sel', 'sel-end');
        if (cmp(day, this.start) > 0 && cmp(day, this.end) < 0) cls.push('in-range');
      }
      this.grid.append(h(`button.${cls.join('.')}` as 'button.dp-day', {
        type: 'button', tabindex: '-1', 'data-date': key(day), onclick: () => this.clickDay(day),
      }, String(day.day)));
      d = addDays(d, 1);
    }
    const fmt = (x: Day) => `${WEEKDAY_NAMES[toJs(x).getDay()]!.slice(0, 3)}, ${MONTH_NAMES[x.month - 1]!.slice(0, 3)} ${x.day}`;
    this.summary.textContent = this.range
      ? `${fmt(this.start)} – ${fmt(this.end)}${this.picking === 'end' ? ' · pick the end date' : ''}`
      : fmt(this.start) + (this.start.year !== today.year ? ` ${this.start.year}` : '');
    this.endRow.hidden = !this.range;

    const val = this.repeatValue();
    const options = this.presets().map(([k, label]) => h('option', { value: k, selected: k === val }, label));
    if (val === 'cur' && this.recur) {
      const label = describeRecurrence(this.recur);
      options.push(h('option', { value: 'cur', selected: true }, label[0]!.toUpperCase() + label.slice(1)));
    }
    options.push(h('option', { value: 'custom' }, 'Custom…'));
    this.repeat.replaceChildren(...options);
    this.fromC.disabled = !this.recur;
    if (this.el.isConnected) this.reposition();
  }
}
