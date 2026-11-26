//! Layout adapted from ffight763-glitch/Vinteria-git3, src/ui/mobile.rs (204bdd9).

use super::{PAGES, ViSales, theme, visuals};
use eframe::egui::{self, Button, Color32, Frame, Margin, RichText, Stroke};

const TOUCH: f32 = 46.;

pub(super) fn shell(app: &mut ViSales, ctx: &egui::Context, bottom_tabs: bool) {
    egui::TopBottomPanel::top("mobile-header")
        .frame(panel(ctx, 12, 6))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("ViSales")
                        .size(22.)
                        .strong()
                        .color(accent(app.dark)),
                );
                if ui.small_button("Заблокувати").clicked() {
                    app.lock_screen();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if app.dark {
                        "Світла тема"
                    } else {
                        "Темна тема"
                    };
                    if ui.add_sized([98., TOUCH], Button::new(label)).clicked() {
                        app.dark = !app.dark;
                        ctx.set_visuals(visuals(app.dark));
                    }
                });
            });
        });
    if bottom_tabs {
        egui::TopBottomPanel::bottom("mobile-tabs")
            .frame(panel(ctx, 6, 6))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.x = 3.;
                let columns = if ui.available_width() >= 345. { 6 } else { 3 };
                let indices: Vec<_> = app.visible_pages().collect();
                for pages in indices.chunks(columns) {
                    ui.columns(columns, |uis| {
                        for (column, &page) in pages.iter().enumerate() {
                            tab(app, &mut uis[column], page, PAGES[page], TOUCH);
                        }
                    });
                }
            });
    } else {
        egui::SidePanel::left("mobile-rail")
            .exact_width(118.)
            .resizable(false)
            .frame(panel(ctx, 6, 8))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for index in app.visible_pages() {
                        tab(app, ui, index, PAGES[index], TOUCH + 8.);
                    }
                });
            });
    }
}

fn panel(ctx: &egui::Context, horizontal: i8, vertical: i8) -> Frame {
    let visuals = ctx.style().visuals.clone();
    Frame::new()
        .fill(visuals.window_fill)
        .stroke(visuals.widgets.noninteractive.bg_stroke)
        .inner_margin(Margin::symmetric(horizontal, vertical))
}

fn accent(dark: bool) -> Color32 {
    if dark { theme::SIGNAL } else { theme::BLACK }
}

fn tab(app: &mut ViSales, ui: &mut egui::Ui, index: usize, label: &str, height: f32) {
    let active = index == app.page;
    let widgets = &ui.visuals().widgets;
    let button = Button::new(RichText::new(label).size(13.).strong().color(if active {
        theme::BLACK
    } else {
        ui.visuals().text_color()
    }))
    .fill(if active {
        theme::SIGNAL
    } else {
        widgets.inactive.bg_fill
    })
    .stroke(if active {
        Stroke::new(1.0_f32, accent(app.dark))
    } else {
        widgets.inactive.bg_stroke
    });
    if ui
        .add_sized([ui.available_width(), height], button)
        .clicked()
    {
        app.navigate(index);
    }
}

pub(super) fn pills(ui: &mut egui::Ui, selected: &mut usize, labels: &[&str], dark: bool) {
    ui.horizontal_wrapped(|ui| {
        for (index, label) in labels.iter().enumerate() {
            let active = index == *selected;
            let mut button =
                Button::new(RichText::new(*label).strong()).min_size(egui::vec2(82., 38.));
            if active {
                button = button
                    .fill(ui.visuals().selection.bg_fill)
                    .stroke(Stroke::new(1.0_f32, accent(dark)));
            }
            if ui.add(button).clicked() {
                *selected = index;
            }
        }
    });
}

pub(super) fn startup_splash(ctx: &egui::Context, elapsed: f32) -> bool {
    const DURATION: f32 = 0.46;
    if elapsed >= DURATION || !cfg!(target_os = "android") {
        return false;
    }

    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::BLACK))
        .show(ctx, |ui| {
            let center = ui.max_rect().center();
            let painter = ui.painter();
            painter.text(
                center + egui::vec2(0., -24.),
                egui::Align2::CENTER_CENTER,
                "ViSales",
                egui::FontId::proportional(35.),
                Color32::from_rgb(229, 238, 244),
            );
            for dot in 0..8 {
                let offset = dot as f32 / 8.;
                let phase = (elapsed * 2.4 + offset).fract();
                let angle = (offset + elapsed * 0.9) * std::f32::consts::TAU;
                let point =
                    center + egui::vec2(angle.cos(), angle.sin()) * 13. + egui::vec2(0., 29.);
                painter.circle_filled(
                    point,
                    1.5 + phase,
                    theme::SIGNAL.linear_multiply(0.3 + 0.7 * phase),
                );
            }
        });
    ctx.request_repaint_after(std::time::Duration::from_millis(16));
    true
}
