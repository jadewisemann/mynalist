// Fixed-position popups anchored to a caret / element rect.

/** Place a fixed-position popup below `r` (or above when there is more room there). */
export function placePopup(el: HTMLElement, r: DOMRect) {
  const vv = window.visualViewport;
  const vw = vv ? vv.width + vv.offsetLeft : innerWidth;
  const vh = vv ? vv.height + vv.offsetTop : innerHeight;
  const top0 = vv ? vv.offsetTop : 0;
  el.style.maxHeight = '';
  el.style.left = '0px';
  el.style.top = '0px';
  const w = el.offsetWidth;
  let hgt = el.offsetHeight;
  const gap = 4;
  const below = vh - r.bottom - gap - 4;
  const above = r.top - top0 - gap - 4;
  const down = hgt <= below || below >= above;
  const room = Math.max(80, down ? below : above);
  if (hgt > room) {
    el.style.maxHeight = `${room}px`;
    hgt = room;
  }
  const top = down ? r.bottom + gap : r.top - gap - hgt;
  const left = Math.max(4, Math.min(r.left, vw - w - 4));
  el.style.left = `${left}px`;
  el.style.top = `${Math.max(top0 + 4, top)}px`;
  el.classList.toggle('pop-above', !down);
}
