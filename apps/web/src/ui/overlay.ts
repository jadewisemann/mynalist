import { h } from './dom.ts';

export interface MenuItem {
  label: string;
  key?: string;
  run?: () => void;
  sep?: boolean;
  custom?: HTMLElement;
  /** submenu, opened on hover or click */
  sub?: MenuItem[];
  /** shown with a check mark */
  checked?: boolean;
}

let openMenu: HTMLElement | null = null;
let subMenu: HTMLElement | null = null;

export function closeMenu() {
  openMenu?.remove();
  subMenu?.remove();
  openMenu = subMenu = null;
}

function fill(m: HTMLElement, items: MenuItem[], nested: boolean) {
  for (const it of items) {
    if (it.sep) m.append(h('div.sep'));
    else if (it.custom) m.append(it.custom);
    else {
      const row: HTMLElement = h('div.mi', {
        onmousedown: (e: Event) => e.preventDefault(),
        onmouseenter: () => {
          if (nested) return;
          subMenu?.remove();
          subMenu = null;
          if (it.sub) openSub(row, it.sub);
        },
        onclick: () => {
          if (it.sub) return openSub(row, it.sub);
          closeMenu();
          it.run?.();
        },
      }, h('span', {}, it.checked ? `✓ ${it.label}` : it.label), it.key ? h('span.k', {}, it.key) : it.sub ? h('span.k', {}, '▸') : null);
      m.append(row);
    }
  }
}

function place(m: HTMLElement, x: number, y: number) {
  document.body.append(m);
  const r = m.getBoundingClientRect();
  m.style.left = `${Math.max(4, Math.min(x, innerWidth - r.width - 4))}px`;
  m.style.top = `${Math.max(4, Math.min(y, innerHeight - r.height - 4))}px`;
}

function openSub(row: HTMLElement, items: MenuItem[]) {
  subMenu?.remove();
  const m = h('div.menu');
  fill(m, items, true);
  const r = row.getBoundingClientRect();
  place(m, r.right - 4, r.top - 4);
  if (m.getBoundingClientRect().left < r.right - 8) m.style.left = `${Math.max(4, r.left - m.offsetWidth + 4)}px`;
  subMenu = m;
}

export function showMenu(x: number, y: number, items: MenuItem[]) {
  closeMenu();
  const m = h('div.menu');
  fill(m, items, false);
  place(m, x, y);
  openMenu = m;
  setTimeout(() => {
    const off = (e: Event) => {
      if (openMenu === m && !m.contains(e.target as Node) && !subMenu?.contains(e.target as Node)) closeMenu();
      if (openMenu !== m) {
        document.removeEventListener('mousedown', off, true);
        document.removeEventListener('keydown', esc, true);
      }
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && openMenu === m) {
        e.stopPropagation();
        closeMenu();
      }
    };
    document.addEventListener('mousedown', off, true);
    document.addEventListener('keydown', esc, true);
  });
}

/** Modal dialog; returns a close function. */
export function modal(title: string, body: HTMLElement, buttons: { label: string; primary?: boolean; run?: () => boolean | void }[] = []) {
  const close = () => {
    ov.remove();
    document.removeEventListener('keydown', onKey, true);
  };
  const foot = buttons.length
    ? h('div.foot', {}, ...buttons.map((b) => h(b.primary ? 'button.btn.primary' : 'button.btn', { onclick: () => b.run?.() !== false && close() }, b.label)))
    : null;
  const ov = h('div.overlay', { onmousedown: (e: Event) => e.target === ov && close() },
    h('div.modal', {}, title ? h('h2', {}, title) : null, h('div.body', {}, body), foot));
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      close();
    } else if (e.key === 'Enter' && !(e.target instanceof HTMLTextAreaElement) && buttons.some((b) => b.primary)) {
      const b = buttons.find((x) => x.primary)!;
      e.preventDefault();
      if (b.run?.() !== false) close();
    }
  };
  document.addEventListener('keydown', onKey, true);
  document.body.append(ov);
  (body.querySelector('input, select, textarea') as HTMLElement | null)?.focus();
  return close;
}

export interface FinderItem {
  label: string;
  sub?: string;
  run: () => void;
}

/** Fuzzy finder overlay. `search` returns ranked items for a query. */
export function finder(placeholder: string, search: (q: string) => FinderItem[]) {
  let items: FinderItem[] = [];
  let active = 0;
  const input = h('input', { placeholder });
  const results = h('div.results');
  const close = () => {
    ov.remove();
  };
  const paint = () => {
    results.textContent = '';
    items.forEach((it, i) => {
      const r = h('div.res', { onmousedown: (e: Event) => (e.preventDefault(), close(), it.run()) }, h('span', {}, it.label), it.sub ? h('span.sub', {}, it.sub) : null);
      if (i === active) r.classList.add('active');
      results.append(r);
    });
    results.children[active]?.scrollIntoView({ block: 'nearest' });
  };
  const update = () => {
    items = search(input.value).slice(0, 50);
    active = 0;
    paint();
  };
  input.addEventListener('input', update);
  input.addEventListener('keydown', (e) => {
    if (e.isComposing) return;
    if (e.key === 'ArrowDown') active = Math.min(items.length - 1, active + 1);
    else if (e.key === 'ArrowUp') active = Math.max(0, active - 1);
    else if (e.key === 'Enter') {
      const it = items[active];
      close();
      it?.run();
      return;
    } else if (e.key === 'Escape') {
      close();
      return;
    } else return;
    e.preventDefault();
    paint();
  });
  const ov = h('div.overlay', { onmousedown: (e: Event) => e.target === ov && close() }, h('div.modal.finder', {}, input, results));
  document.body.append(ov);
  input.focus();
  update();
}
