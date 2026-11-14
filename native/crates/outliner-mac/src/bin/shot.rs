//! The offscreen render: the look fixtures of `outliner_app::fixture` drawn by GPUI into PNG files, for people to
//! look at (CI uploads them as an artifact; nothing compares them with reference images: the structural snapshot,
//! `outliner-app/tests/snapshot.rs`, is the check).
//!
//! `cargo run -p outliner-mac --features shot --bin shot -- <dir>` writes `<dir>/<doc>-<device>-<state>.png`.
//! GPUI's test platform (`HeadlessAppContext`) runs the windows, without a screen or a window manager, and the
//! platform's offscreen renderer draws their frames: Metal on macOS, wgpu (Vulkan) on Linux.

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn main() -> std::process::ExitCode {
    match shots() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("shot: {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn main() {}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn shots() -> anyhow::Result<()> {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use nsh_platform_gpui::gpui::{self, px, size, AppContext as _, HeadlessAppContext};
    use nsh_platform_gpui::{set_ui_sets, App, AppOptions, AppView, Appearance};

    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "shots".into()));
    std::fs::create_dir_all(&dir)?;
    // the dates' local zone
    std::env::set_var("TZ", "UTC");
    set_ui_sets(vec![nsh_base::registry(), outliner_app::registry()]);

    let text_system = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text_system, Arc::new(()), gpui_platform::current_headless_renderer);
    // the engines run on threads of their own
    cx.allow_parking();

    // name, size, laid out as a phone
    let devices = [("desktop", (1100.0, 800.0), false), ("phone", (390.0, 844.0), true)];
    for doc in outliner_app::fixture::DOCS {
        for (device, (w, h), phone) in devices {
            for editing in [false, true] {
                // `style::phone()` reads it as the views are made (GPUI's test platform is a desktop)
                if phone {
                    std::env::set_var("OUTLINER_PHONE", "1");
                } else {
                    std::env::remove_var("OUTLINER_PHONE");
                }
                let edited = editing.then(|| outliner_app::fixture::edited_item(doc));
                let root = outliner_app::FONTS
                    .iter()
                    .fold(outliner_app::fixture::root(doc, edited), |root, font| root.font(*font));
                let options = AppOptions {
                    app_id: "dev.outliner.shot".to_owned(),
                    title: "Outliner".to_owned(),
                    size: (w as i32, h as i32),
                    appearance: Appearance::Light,
                    ..Default::default()
                };
                let window = cx.open_window(size(px(w), px(h)), |window, cx| {
                    let view = cx.new(|cx| AppView::new(options, window, cx));
                    view.read(cx).load(App::Rust(root));
                    view
                })?;
                let handle: gpui::AnyWindowHandle = window.into();

                // draw until the frame stops changing (the engine lays out on its own thread)
                let start = Instant::now();
                let mut last: Option<image::RgbaImage> = None;
                let mut same = 0;
                let frame = loop {
                    cx.run_until_parked();
                    cx.update_window(handle, |_, window, _| window.refresh())?;
                    cx.run_until_parked();
                    let now = cx.capture_screenshot(handle)?;
                    same = if last.as_ref() == Some(&now) { same + 1 } else { 0 };
                    if same >= 5 && start.elapsed() > Duration::from_millis(500) {
                        break now;
                    }
                    if start.elapsed() > Duration::from_secs(20) {
                        eprintln!("shot: {doc} {device}: the frame never settled; writing the last one");
                        break now;
                    }
                    last = Some(now);
                    std::thread::sleep(Duration::from_millis(50));
                };
                let name = format!("{doc}-{device}-{}.png", if editing { "edit" } else { "rest" });
                frame.save(dir.join(&name))?;
                println!("{name} {}x{}", frame.width(), frame.height());
                cx.update_window(handle, |_, window, _| window.remove_window())?;
                cx.run_until_parked();
            }
        }
    }
    Ok(())
}
