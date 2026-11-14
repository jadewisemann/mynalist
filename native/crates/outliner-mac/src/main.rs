//! The macOS app: `outliner_app::App` in a window, drawn by GPUI.
//!
//! `cargo run -p outliner-mac` on a Mac. CI packages the binary as `Outliner.app`
//! (`.github/workflows/native.yml`).

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
fn main() -> std::process::ExitCode {
    use nsh_platform_gpui::{run_app, set_ui_sets, Appearance, AppOptions, AppSource, Root};

    set_ui_sets(vec![nsh_base::registry(), outliner_app::registry()]);
    run_app(
        AppSource::Rust(Root::of::<outliner_app::App>()),
        AppOptions {
            app_id: "dev.outliner.app".to_owned(),
            title: "Outliner".to_owned(),
            size: (900, 760),
            appearance: Appearance::System,
            ..Default::default()
        },
    )
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn main() {}
