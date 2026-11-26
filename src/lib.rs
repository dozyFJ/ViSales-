#[cfg(target_os = "android")]
mod android_scanner;
mod exchange;
mod store;
mod ui;

use std::path::PathBuf;

pub fn run(directory: PathBuf, options: eframe::NativeOptions) -> eframe::Result {
    #[cfg(target_os = "android")]
    let android_app = options.android_app.clone();

    eframe::run_native(
        "ViSales",
        options,
        Box::new(move |context| {
            context.egui_ctx.set_visuals(ui::visuals(true));
            let app = ui::ViSales::open(directory);
            #[cfg(target_os = "android")]
            let app = app.with_android_app(android_app);
            Ok(Box::new(app))
        }),
    )
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::platform::android::activity::WindowManagerFlags;

    app.set_window_flags(
        WindowManagerFlags::FULLSCREEN,
        WindowManagerFlags::FORCE_NOT_FULLSCREEN,
    );

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
