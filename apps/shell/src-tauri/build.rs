use std::env;

fn main() {
    // Android 15 introduced 16 KB page-size devices. A .so whose PT_LOAD
    // segments are 4 KB-aligned fails to load there with "empty/missing
    // PT_LOAD segment", so ask the linker for 16 KB alignment.
    //
    // This has to happen here rather than in .cargo/config.toml:
    //   * `target.<triple>.rustflags` is overridden by the
    //     CARGO_TARGET_<TRIPLE>_RUSTFLAGS the Tauri CLI exports;
    //   * `target.<triple>.linker` is likewise overridden by
    //     CARGO_TARGET_<TRIPLE>_LINKER, so a wrapper script is skipped.
    // `cargo:rustc-link-arg` is appended to the link line and wins over neither,
    // which is exactly what is needed.
    //
    // Same trap as the GPUI spike: work/outliner-native/task.md, "함정 C".
    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("android") {
        println!("cargo:rustc-link-arg=-Wl,-z,max-page-size=16384");
    }

    tauri_build::build()
}
