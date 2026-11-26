#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".local/share"))
        })
        .unwrap_or_else(std::env::temp_dir);
    visales::run(
        base.join("ViSales"),
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1240., 790.]),
            ..Default::default()
        },
    )
}

#[cfg(target_os = "android")]
fn main() {}
