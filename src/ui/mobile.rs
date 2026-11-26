//! Layout adapted from ffight763-glitch/Vinteria-git3, src/ui/mobile.rs (204bdd9).

use super::{PAGES, ViSales, visuals};
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
            if app.dark {
                let width = ui.available_width();
                let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 2.), egui::Sense::hover());
                let mut mesh = egui::Mesh::default();
                for (index, color) in [
                    Color32::from_rgb(255, 79, 200),
                    Color32::from_rgb(177, 107, 255),
                    Color32::from_rgb(77, 163, 255),
                ]
                .into_iter()
                .enumerate()
                {
                    let x = rect.left() + rect.width() * index as f32 / 2.;
                    mesh.colored_vertex(egui::pos2(x, rect.top()), color);
                    mesh.colored_vertex(egui::pos2(x, rect.bottom()), color);
                }
                mesh.indices.extend([0, 1, 2, 1, 3, 2, 2, 3, 4, 3, 5, 4]);
                ui.painter().add(egui::Shape::mesh(mesh));
            }
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
    if dark {
        Color32::from_rgb(93, 215, 225)
    } else {
        Color32::from_rgb(0, 104, 123)
    }
}

fn tab(app: &mut ViSales, ui: &mut egui::Ui, index: usize, label: &str, height: f32) {
    let active = index == app.page;
    let widgets = &ui.visuals().widgets;
    let button = Button::new(RichText::new(label).size(13.).strong().color(if active {
        Color32::WHITE
    } else {
        ui.visuals().text_color()
    }))
    .fill(if active {
        Color32::from_rgb(0, 105, 127)
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
