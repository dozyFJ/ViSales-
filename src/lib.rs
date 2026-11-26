mod exchange;
mod store;
mod ui;

use std::path::PathBuf;

pub fn run(directory: PathBuf, options: eframe::NativeOptions) -> eframe::Result {
    eframe::run_native(
        "ViSales",
        options,
        Box::new(move |context| {
            context.egui_ctx.set_visuals(ui::visuals(true));
            Ok(Box::new(ui::ViSales::open(directory)))
        }),
    )
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
    if let Some(path) = app.internal_data_path() {
        let _ = run(
            path,
            eframe::NativeOptions {
                android_app: Some(app),
                ..Default::default()
            },
        );
    }
}
