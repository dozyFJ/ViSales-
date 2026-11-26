use super::{PAGES, ViSales, matches, money, visuals};
use eframe::egui::{self, Button, Color32, Frame, Label, Margin, RichText, Stroke, TextEdit};
use egui_extras::{Column, TableBuilder};

#[derive(PartialEq, Eq)]
pub(super) enum Search {
    Product,
    Barcode,
    Cart,
}

const ACCENTS: [Color32; 3] = [
    Color32::from_rgb(235, 126, 143),
    Color32::from_rgb(102, 179, 213),
    Color32::from_rgb(141, 199, 113),
];

struct Palette {
    grid: Color32,
    rail: Color32,
    line: Color32,
    ink: Color32,
    action: Color32,
}

impl Palette {
    fn new(dark: bool) -> Self {
        let [grid, rail, line, ink, action] = if dark {
            [
                [34, 41, 46],
                [54, 67, 77],
                [82, 94, 102],
                [230, 237, 241],
                [235, 243, 247],
            ]
        } else {
            [
                [239, 244, 247],
                [218, 227, 233],
                [163, 179, 190],
                [32, 44, 54],
                [190, 211, 225],
            ]
        };
        Self {
            grid: Color32::from_rgb(grid[0], grid[1], grid[2]),
            rail: Color32::from_rgb(rail[0], rail[1], rail[2]),
            line: Color32::from_rgb(line[0], line[1], line[2]),
            ink: Color32::from_rgb(ink[0], ink[1], ink[2]),
            action: Color32::from_rgb(action[0], action[1], action[2]),
        }
    }

    fn panel(&self, horizontal: i8, vertical: i8) -> Frame {
        Frame::new()
            .fill(self.rail)
            .stroke(Stroke::new(1.0_f32, self.line))
            .inner_margin(Margin::symmetric(horizontal, vertical))
    }
}

pub(super) fn show(app: &mut ViSales, ctx: &egui::Context) {
    let palette = Palette::new(app.dark);
    let search_id = egui::Id::new("portrait-pos-search");
    egui::TopBottomPanel::top("portrait-pos-top")
        .frame(palette.panel(9, 6))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("ViSales · Каса");
                ui.label(RichText::new("ТЕСТОВИЙ РЕЖИМ").size(11.).strong());
            });
            if let Some((ok, status)) = &app.notice {
                let color = if *ok {
                    palette.ink
                } else {
                    ui.visuals().error_fg_color
                };
                ui.label(RichText::new(status).size(12.).color(color));
            }
        });

    egui::TopBottomPanel::bottom("portrait-pos-total")
        .frame(palette.panel(9, 7))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("До сплати").size(11.));
                    ui.label(RichText::new(money(app.cart_total())).size(18.).strong());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let checkout = Button::new(
                        RichText::new("Тестовий розрахунок")
                            .size(12.)
                            .strong()
                            .color(Color32::from_rgb(28, 42, 51)),
                    )
                    .fill(palette.action)
                    .corner_radius(20)
                    .min_size(egui::vec2(0., 42.));
                    if ui
                        .add_enabled(!app.cart.is_empty() && app.store.is_some(), checkout)
                        .clicked()
                    {
                        app.checkout();
                    }
                });
            });
        });

    egui::SidePanel::right("portrait-pos-actions")
        .exact_width(81.)
        .resizable(false)
        .frame(palette.panel(4, 8))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.menu_button("Розділи", |ui| {
                    for (index, label) in PAGES.iter().enumerate() {
                        if ui.selectable_label(app.page == index, *label).clicked() {
                            app.page = index;
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui
                        .button(if app.dark {
                            "Світла тема"
                        } else {
                            "Темна тема"
                        })
                        .clicked()
                    {
                        app.dark = !app.dark;
                        ctx.set_visuals(visuals(app.dark));
                        ui.close_menu();
                    }
                });
                for (mode, label) in [
                    (Search::Product, "Пошук"),
                    (Search::Barcode, "Штрихкод"),
                    (Search::Cart, "Кошик"),
                ] {
                    let active = app.portrait_search == mode;
                    let ink = if active {
                        Color32::from_rgb(28, 42, 51)
                    } else {
                        palette.ink
                    };
                    let button = Button::new(RichText::new(label).size(12.).strong().color(ink))
                        .corner_radius(19)
                        .fill(if active { palette.action } else { palette.grid });
                    if ui.add_sized([ui.available_width(), 44.], button).clicked() {
                        if mode != Search::Cart {
                            if app.portrait_search != mode {
                                app.search.clear();
                            }
                            ctx.memory_mut(|memory| memory.request_focus(search_id));
                        }
                        app.portrait_search = mode;
                    }
                }
                let count: i64 = app.cart.values().map(|(quantity, _)| quantity).sum();
                ui.label(RichText::new(format!("{count} шт.")).size(11.));
            });
        });

    egui::CentralPanel::default()
        .frame(Frame::new().fill(palette.grid).inner_margin(4))
        .show(ctx, |ui| {
            if app.store.is_none() {
                return;
            }
            if app.portrait_search == Search::Cart {
                ui.heading("Кошик");
                egui::ScrollArea::vertical().show(ui, |ui| app.cart_lines(ui));
                return;
            }
            let barcode = app.portrait_search == Search::Barcode;
            let input = ui.add(
                TextEdit::singleline(&mut app.search)
                    .id(search_id)
                    .desired_width(f32::INFINITY)
                    .hint_text(if barcode {
                        "Введіть штрихкод"
                    } else {
                        "Знайти товар"
                    }),
            );
            if input.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                app.add_barcode();
                input.request_focus();
            }
            grid(app, ui, &palette, barcode);
        });
}

fn grid(app: &mut ViSales, ui: &mut egui::Ui, palette: &Palette, barcode: bool) {
    const ROW_HEIGHT: f32 = 38.;
    let query = app.search.trim().to_lowercase();
    let products: Vec<_> = app
        .products
        .iter()
        .filter(|product| {
            if barcode {
                product.barcode.starts_with(&query)
            } else {
                matches(product, &query)
            }
        })
        .collect();
    let mut selected = None;
    if products.is_empty() {
        ui.label(if query.is_empty() {
            "Додайте товар у каталозі."
        } else {
            "Товарів не знайдено."
        });
    }
    ui.spacing_mut().item_spacing = egui::vec2(1., 1.);
    let width = (ui.available_width() - 13.).max(90.);
    let rows = products
        .len()
        .max(((ui.available_height() - 28.).max(0.) / ROW_HEIGHT).ceil() as usize);
    TableBuilder::new(ui)
        .auto_shrink([false, false])
        .striped(true)
        .column(Column::exact(width * 0.48).clip(true))
        .column(Column::exact(width * 0.32).clip(true))
        .column(Column::remainder().clip(true))
        .header(29., |mut header| {
            for (index, label) in ["Товар", "Ціна", "Запас"].into_iter().enumerate() {
                header.col(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 0., palette.rail);
                    ui.painter().hline(
                        rect.x_range(),
                        rect.bottom(),
                        Stroke::new(2.0_f32, ACCENTS[index]),
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional(12.),
                        palette.ink,
                    );
                });
            }
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, rows, |mut row| {
                let product = products.get(row.index()).copied();
                let values = product
                    .map(|p| [p.name.clone(), money(p.price), p.stock.to_string()])
                    .unwrap_or_default();
                for (index, text) in values.into_iter().enumerate() {
                    row.col(|ui| {
                        let rect = ui.available_rect_before_wrap();
                        ui.painter().hline(
                            rect.x_range(),
                            rect.bottom(),
                            Stroke::new(1.0_f32, palette.line),
                        );
                        ui.painter().vline(
                            rect.right(),
                            rect.y_range(),
                            Stroke::new(1.0_f32, palette.line),
                        );
                        let response = ui.add_sized(
                            rect.size(),
                            Label::new(RichText::new(text).size(12.).color(
                                if index == 0 || !app.dark {
                                    palette.ink
                                } else {
                                    ACCENTS[index]
                                },
                            ))
                            .truncate()
                            .sense(if product.is_some() {
                                egui::Sense::click()
                            } else {
                                egui::Sense::hover()
                            }),
                        );
                        if response.clicked() {
                            selected = product.cloned();
                        }
                    });
                }
            })
        });
    if let Some(product) = selected {
        app.add(&product);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Product, Store};
    use std::path::Path;

    #[test]
    fn portrait_grid_adds_a_product_and_keeps_checkout_below_it() -> anyhow::Result<()> {
        let ctx = egui::Context::default();
        ctx.set_visuals(visuals(true));
        let mut store = Store::open(Path::new(":memory:"))?;
        store.save_products(&[Product {
            id: 0,
            barcode: "123".into(),
            name: "Кава".into(),
            cost: 4200,
            price: 6900,
            stock: 0,
        }])?;
        store.count(store.products()?[0].id, 3)?;
        let mut app = ViSales::open(
            std::env::temp_dir().join(format!("visales-portrait-{}", std::process::id())),
        );
        let directory = app.directory.clone();
        app.store = Some(store);
        app.refresh()?;
        let mut frame = |events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(360., 760.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| show(&mut app, ctx),
            )
        };
        let _ = frame(Vec::new());
        let output = frame(Vec::new());
        let text_position = |value: &str| {
            output.shapes.iter().find_map(|clipped| {
                if let egui::Shape::Text(text) = &clipped.shape {
                    if text.galley.text() == value {
                        return Some(text.pos + text.galley.size() / 2.);
                    }
                }
                None
            })
        };
        let product = text_position("Кава").expect("catalogue row appears in portrait");
        let checkout = text_position("Тестовий розрахунок").expect("checkout stays visible");
        assert!(checkout.y > 680. && product.y < checkout.y);
        let _ = frame(vec![egui::Event::PointerMoved(product)]);
        for pressed in [true, false] {
            let _ = frame(vec![
                egui::Event::PointerMoved(product),
                egui::Event::PointerButton {
                    pos: product,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        assert_eq!(app.cart_total(), 6900);
        drop(app);
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }
}
