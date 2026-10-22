// Native-shell hooks (apps/android, Capacitor). Everything here is a no-op in a normal browser.
import { Capacitor, SystemBars, SystemBarsStyle } from '@capacitor/core';
import { App } from '@capacitor/app';
import { closeMenu } from './ui/overlay.ts';

function escapeTo(target: EventTarget) {
  target.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape', bubbles: true, cancelable: true }));
}

/** Close the topmost transient UI (menu, dialog, finder, mobile sidebar). Returns true if something was closed. */
function closeTopLayer(): boolean {
  if (document.querySelector('.menu')) {
    closeMenu();
    return true;
  }
  const overlays = document.querySelectorAll<HTMLElement>('.overlay');
  const ov = overlays[overlays.length - 1];
  if (ov) {
    // Dialogs close on Escape (capture listener on document, which also cleans up after itself);
    // the finder has no other state, so removing it is its close().
    if (!ov.querySelector('.finder')) escapeTo(document);
    if (ov.isConnected) ov.remove();
    return true;
  }
  const app = document.querySelector('.app.side-open');
  if (app) {
    app.classList.remove('side-open');
    return true;
  }
  return false;
}

function syncBarStyle() {
  // `Dark` = light icons for a dark background.
  const theme = document.documentElement.dataset.theme;
  const dark = theme === 'dark' || theme === 'black';
  void SystemBars.setStyle({ style: dark ? SystemBarsStyle.Dark : SystemBarsStyle.Light }).catch(() => {});
}

if (Capacitor.isNativePlatform()) {
  document.documentElement.classList.add('native', `native-${Capacitor.getPlatform()}`);

  void App.addListener('backButton', ({ canGoBack }) => {
    if (closeTopLayer()) return;
    if (canGoBack) history.back();
    else void App.exitApp();
  });

  syncBarStyle();
  new MutationObserver(syncBarStyle).observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
}
