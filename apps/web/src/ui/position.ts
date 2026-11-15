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
  // the design's values (space.step4, space.popup_room via styles.css), as the native picker's popover reads them
  const css = getComputedStyle(document.documentElement);
  const gap = parseFloat(css.getPropertyValue('--step-4')) || 4;
  const below = vh - r.bottom - gap - gap;
  const above = r.top - top0 - gap - gap;
  const down = hgt <= below || below >= above;
  const room = Math.max(parseFloat(css.getPropertyValue('--popup-room')) || 80, down ? below : above);
  if (hgt > room) {
    el.style.maxHeight = `${room}px`;
    hgt = room;
  }
  const top = down ? r.bottom + gap : r.top - gap - hgt;
  const left = Math.max(gap, Math.min(r.left, vw - w - gap));
  el.style.left = `${left}px`;
  el.style.top = `${Math.max(top0 + gap, top)}px`;
  el.classList.toggle('pop-above', !down);
}
