// Shell entry point for both the desktop binary and the Android library.
//
// The web app is shipped as-is: `frontendDist` points at the existing
// `apps/web/dist` build of the outliner repo, so nothing here changes web code.
// This file only adds what a browser cannot do for itself: a real window,
// a macOS menu bar, and handing external links to the system browser.

use tauri::plugin::TauriPlugin;
use tauri::{Manager, Runtime, Url};

/// URL schemes the webview may navigate to. Everything else is handed to the
/// OS, so share links and `target="_blank"` links do not replace the app.
fn is_internal(url: &Url) -> bool {
    match url.scheme() {
        "tauri" | "about" | "blob" | "data" => true,
        "http" | "https" => matches!(url.host_str(), Some("tauri.localhost") | Some("localhost")),
        _ => false,
    }
}

/// Intercepts top-level navigation. Tauri 2 only exposes `on_navigation` on the
/// webview *builder*, and the window here comes from `tauri.conf.json`, so the
/// hook is installed as a plugin (which is called for every webview).
fn navigation_plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("outliner-nav")
        .on_navigation(|_webview, url| {
            if is_internal(url) {
                return true;
            }
            if matches!(url.scheme(), "http" | "https" | "mailto") {
                let _ = tauri_plugin_opener::open_url(url.as_str(), None::<&str>);
            }
            false
        })
        .build()
}

/// Restores `window.open` and `target="_blank"` inside the webview.
///
/// WKWebView and WebKitGTK do not open a new browser window for `window.open`
/// unless the embedding app handles the request, so on its own:
///   * `window.open(url, '_blank')` returns `null` and does nothing (the web app
///     uses it for "open in new tab", and for the share dialog's Open button);
///   * an `<a target="_blank">` click is swallowed.
/// In the Android WebView `setSupportMultipleWindows` is off, so `window.open`
/// fails there too. Either way the user sees a dead control.
///
/// The fix is shell-side only: route those URLs to the OS browser, which is
/// what a desktop app should do anyway. Injected at document start so the web
/// code sees a normal `window.open`; the app is not modified.
const OPEN_EXTERNAL_JS: &str = r#"
(() => {
  const openExternal = (url) => {
    const target = String(url);
    try {
      // Never navigate the app itself: that is the bug being fixed here.
      Promise.resolve(window.__TAURI_INTERNALS__.invoke('plugin:opener|open_url', { url: target }))
        .catch((e) => console.warn('[shell] could not open externally:', target, e));
    } catch (e) {
      console.warn('[shell] opener unavailable:', target, e);
    }
    return null;
  };

  const isExternal = (url) => {
    try {
      const u = new URL(String(url), window.location.href);
      // In-app routes and blob/data URLs must stay in the app.
      if (u.protocol === 'tauri:' || u.protocol === 'about:') return false;
      if (u.protocol === 'blob:' || u.protocol === 'data:') return false;
      if (u.protocol === 'http:' || u.protocol === 'https:') {
        return !(u.hostname === 'tauri.localhost' || u.hostname === 'localhost');
      }
      return u.protocol === 'mailto:' || u.protocol === 'tel:';
    } catch (_) {
      return false;
    }
  };

  const nativeOpen = window.open.bind(window);
  window.open = function (url, target, features) {
    if (url && isExternal(url)) return openExternal(url);
    // Anything internal keeps the platform behaviour (which may still be a
    // no-op, but it is the browser's own decision, not ours).
    return nativeOpen(url, target, features);
  };

  // `target="_blank"` links are used in the share dialog. Bubble phase (false)
  // so the web app's own click handlers run first and can still cancel it;
  // preventDefault then stops the webview from trying to load the URL itself.
  document.addEventListener('click', (e) => {
    const a = e.target && e.target.closest ? e.target.closest('a[href]') : null;
    if (!a) return;
    if (a.target !== '_blank' && a.target !== '_new') return;
    if (!isExternal(a.href)) return;
    e.preventDefault();
    openExternal(a.href);
  }, false);
})();
"#;

fn external_links_plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("outliner-external-links")
        .js_init_script(OPEN_EXTERNAL_JS)
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(navigation_plugin())
        .plugin(external_links_plugin());

    builder = builder.setup(|app| {
        // The window is created by Tauri from tauri.conf.json before setup runs.
        #[cfg(desktop)]
        if app.get_webview_window("main").is_none() {
            eprintln!("outliner: warning: no 'main' window from config");
        }
        eprintln!("outliner: shell ready");

        // Headless verification hook. A GPU-less X server cannot be judged by
        // pixels, so the smoke test asks the webview itself what it rendered:
        //   OUTLINER_SMOKE_EVAL='document.title' ./outliner
        // The result goes to stderr as `outliner: eval <json>`.
        if let Ok(js) = std::env::var("OUTLINER_SMOKE_EVAL") {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(
                    std::env::var("OUTLINER_SMOKE_DELAY").ok().and_then(|s| s.parse().ok()).unwrap_or(4),
                ));
                if let Some(w) = handle.get_webview_window("main") {
                    let js = js.clone();
                    let _ = w.eval_with_callback(&js, move |result| {
                        eprintln!("outliner: eval {}", result);
                    });
                }
            });
        }

        Ok(())
    });

    #[cfg(desktop)]
    {
        builder = builder.menu(build_menu).on_menu_event(|app, event| {
            let id = event.id().0.as_str();
            match id {
                "quit" => app.exit(0),
                "close-window" => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.close();
                    }
                }
                // Everything else is replayed into the web app as a synthetic
                // keydown, so the menu and the keymap cannot drift apart.
                other => {
                    if let Some(combo) = menu_combo(other) {
                        forward_combo(app, combo);
                    }
                }
            }
        });
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running the Outliner shell");
}

/// Native macOS menu bar. Items that exist in the web app's keymap are marked
/// with an accelerator and dispatched to the webview, so the web app stays the
/// single source of truth for what each command does.
/// The menu is built on every desktop platform so the code is type-checked on
/// all of them; on macOS it is the menu bar the product needs, and on Linux/BSD
/// it is a normal GTK menu bar with the same commands (CmdOrCtrl becomes Ctrl).
#[cfg(desktop)]
fn build_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{AboutMetadata, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};

    let about = AboutMetadata {
        name: Some("Outliner".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        ..Default::default()
    };

    let app_menu = SubmenuBuilder::new(app, "Outliner")
        .item(&PredefinedMenuItem::about(app, Some("About Outliner"), Some(about))?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("settings", "Settings…")
                .accelerator("CmdOrCtrl+,")
                .build(app)?,
        )
        .separator()
        .item(&PredefinedMenuItem::services(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, None)?)
        .item(&PredefinedMenuItem::hide_others(app, None)?)
        .item(&PredefinedMenuItem::show_all(app, None)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("quit", "Quit Outliner")
                .accelerator("CmdOrCtrl+Q")
                .build(app)?,
        )
        .build()?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(
            &MenuItemBuilder::with_id("save", "Save")
                .accelerator("CmdOrCtrl+S")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("download-backup", "Download Backup…").build(app)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("inbox-capture", "Capture to Inbox")
                .accelerator("CmdOrCtrl+Shift+I")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("close-window", "Close Window")
                .accelerator("CmdOrCtrl+W")
                .build(app)?,
        )
        .build()?;

    // Undo/redo are NOT `PredefinedMenuItem::undo/redo`: those drive the
    // webview's native text undo, which knows nothing about the outline tree.
    // The app keeps its own transactional history (`editor.undo()` ->
    // `DocSession.undo()`), so these dispatch `Mod+Z` / `Mod+Shift+Z` instead.
    // Cut/copy/paste/select-all below ARE native on purpose: the editor handles
    // the real `copy`/`cut`/`paste` events, so the predefined items are correct.
    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .item(
            &MenuItemBuilder::with_id("undo", "Undo")
                .accelerator("CmdOrCtrl+Z")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("redo", "Redo")
                .accelerator("CmdOrCtrl+Shift+Z")
                .build(app)?,
        )
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("search", "Find in Document")
                .accelerator("CmdOrCtrl+F")
                .build(app)?,
        )
        // "Find and Replace" is deliberately absent: the web app's
        // `search-replace` command has no default key binding (keymap.ts:
        // `keys: []`), and the shell cannot invoke commands by id -- only by
        // replaying a keydown. A menu item that does nothing is worse than no
        // item. If it ever gets a default binding, add it to `menu_combo` and
        // `scripts/smoke-menu-keys.sh` will accept it automatically.
        .item(
            &MenuItemBuilder::with_id("go-anywhere", "File Finder")
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("goto-item", "Item Finder")
                .accelerator("CmdOrCtrl+Shift+O")
                .build(app)?,
        )
        .build()?;

    let view_menu = SubmenuBuilder::new(app, "View")
        .item(
            &MenuItemBuilder::with_id("toggle-file-pane", "Toggle File Pane")
                .accelerator("CmdOrCtrl+Shift+F")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("toggle-bookmark-pane", "Toggle Bookmark Pane")
                .accelerator("CmdOrCtrl+Shift+B")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("collapse-all", "Expand / Collapse All")
                .accelerator("CmdOrCtrl+Shift+.")
                .build(app)?,
        )
        .separator()
        .item(&PredefinedMenuItem::fullscreen(app, None)?)
        .build()?;

    let window_menu = SubmenuBuilder::new(app, "Window")
        .item(&PredefinedMenuItem::minimize(app, None)?)
        .item(&PredefinedMenuItem::maximize(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::close_window(app, None)?)
        .build()?;

    let help_menu = SubmenuBuilder::new(app, "Help")
        .item(
            &MenuItemBuilder::with_id("help", "Outliner Help / Shortcuts")
                .accelerator("CmdOrCtrl+/")
                .build(app)?,
        )
        .build()?;

    MenuBuilder::new(app)
        .items(&[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &window_menu,
            &help_menu,
        ])
        .build()
}

/// Menu id -> the web app's key combo. The format matches `comboOf()` in
/// apps/web/src/editor/keymap.ts (`Mod` = Cmd on macOS).
#[cfg(desktop)]
fn menu_combo(id: &str) -> Option<&'static str> {
    Some(match id {
        "undo" => "Mod+Z",
        "redo" => "Mod+Shift+Z",
        "settings" => "Mod+,",
        "save" => "Mod+S",
        "inbox-capture" => "Mod+Shift+I",
        "search" => "Mod+F",
        "go-anywhere" => "Mod+O",
        "goto-item" => "Mod+Shift+O",
        "toggle-file-pane" => "Mod+Shift+F",
        "toggle-bookmark-pane" => "Mod+Shift+B",
        "collapse-all" => "Mod+Shift+.",
        "help" => "Mod+/",
        _ => return None,
    })
}

/// Replay a menu command into the webview as a synthetic keydown.
///
/// The web keymap reads `KeyboardEvent.code` first (so it keeps working under a
/// non-Latin IME), so both `code` and `key` are filled in.
///
/// `Mod` must be *only* Cmd (metaKey) on macOS and *only* Ctrl elsewhere.
/// `comboOf()` in apps/web/src/editor/keymap.ts does:
///     mod = IS_MAC ? e.metaKey : e.ctrlKey;
///     if (IS_MAC && e.ctrlKey) parts.push("Ctrl");
/// so setting both keys on macOS produces "Mod+Ctrl+S", which matches no
/// binding and the menu item silently does nothing.
/// `Mod` is Cmd on macOS and Ctrl everywhere else; see `forward_combo`.
#[cfg(all(desktop, target_os = "macos"))]
const MOD_JS: &str = "metaKey:true,";
#[cfg(all(desktop, not(target_os = "macos")))]
const MOD_JS: &str = "ctrlKey:true,";

#[cfg(desktop)]
fn forward_combo(app: &tauri::AppHandle, combo: &str) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let parts: Vec<&str> = combo.split('+').collect();
    let Some(key) = parts.last().copied() else {
        return;
    };
    let mut modifiers = String::new();
    for p in &parts[..parts.len() - 1] {
        match *p {
            "Mod" => modifiers.push_str(MOD_JS),
            "Ctrl" => modifiers.push_str("ctrlKey:true,"),
            "Alt" => modifiers.push_str("altKey:true,"),
            "Shift" => modifiers.push_str("shiftKey:true,"),
            _ => {}
        }
    }
    let code = match key {
        "," => "Comma".to_string(),
        "." => "Period".to_string(),
        "/" => "Slash".to_string(),
        k if k.len() == 1 => format!("Key{}", k.to_ascii_uppercase()),
        k => k.to_string(),
    };
    let script = format!(
        "(()=>{{const t=document.activeElement||document.body;\
         t.dispatchEvent(new KeyboardEvent('keydown',{{key:{key:?},code:{code:?},{modifiers}\
         bubbles:true,cancelable:true}}));}})()"
    );
    let _ = window.eval(&script);
}
