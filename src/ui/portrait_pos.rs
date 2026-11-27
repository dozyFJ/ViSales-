use super::{
    PAGES, ViSales, admin, matches, money,
    theme::{self, Palette},
};
use crate::store::Product;
use eframe::egui::{self, Button, Color32, Frame, Label, RichText, Stroke, TextEdit};
use egui_extras::{Column, TableBuilder};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Search {
    Browse,
    Product,
    Barcode,
    Cart,
    Customers,
    Parked,
    Receipts,
}

const ACCENTS: [Color32; 3] = [theme::SIGNAL, theme::LILAC, theme::SIGNAL];

pub(super) fn show(app: &mut ViSales, ctx: &egui::Context) {
    let palette = Palette::new(app.dark);
    let search_id = egui::Id::new("portrait-pos-search");
    egui::TopBottomPanel::top("portrait-pos-top")
        .exact_height(75.)
        .frame(palette.panel(9, 6))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format!("ViSales · {}", PAGES[app.page]))
                            .size(16.)
                            .strong(),
                    );
                    let role = if app.access.unlocked {
                        "Адміністратор"
                    } else {
                        "Касир"
                    };
                    let label = if app.page == 0 {
                        format!("ТЕСТОВИЙ РЕЖИМ · {role}")
                    } else {
                        role.to_owned()
                    };
                    ui.label(RichText::new(label).size(10.).strong().color(palette.muted));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let cart = app.page == 0 && app.portrait_search == Search::Cart;
                    let quantity: i64 = app.cart.values().map(|(quantity, _)| quantity).sum();
                    let (fill, ink, border) = palette.navigation(theme::MINT, cart);
                    let button = Button::new(
                        RichText::new(format!("Кошик · {quantity}"))
                            .strong()
                            .color(ink),
                    )
                    .fill(fill)
                    .stroke(Stroke::new(2.0_f32, border))
                    .min_size(egui::vec2(0., 39.));
                    if ui.add(button).clicked() {
                        app.navigate(0);
                        app.portrait_search = Search::Cart;
                        ctx.memory_mut(|memory| memory.surrender_focus(search_id));
                    }
                });
            });
            if let Some((ok, status)) = &app.notice {
                let color = if *ok {
                    palette.ink
                } else {
                    ui.visuals().error_fg_color
                };
                ui.add(Label::new(RichText::new(status).size(12.).color(color)).truncate())
                    .on_hover_text(status);
            }
        });

    dock(app, ctx, &palette, search_id);

    if app.page == 0 {
        egui::TopBottomPanel::bottom("portrait-pos-total")
            .exact_height(62.)
            .frame(palette.panel(9, 7))
            .show(ctx, |ui| {
                ui.columns(2, |columns| {
                    let total = money(app.cart_total());
                    columns[0].label(RichText::new("До сплати").size(11.));
                    columns[0]
                        .add(
                            Label::new(RichText::new(&total).size(20.).monospace().strong())
                                .truncate(),
                        )
                        .on_hover_text(total);
                    let caption = if ctx.screen_rect().width() < 330. {
                        "Розрахунок"
                    } else {
                        "Тестовий розрахунок"
                    };
                    let checkout = Button::new(
                        RichText::new(caption)
                            .size(12.)
                            .strong()
                            .color(theme::BLACK),
                    )
                    .fill(palette.action)
                    .stroke(Stroke::new(2.0_f32, theme::BLACK))
                    .min_size(egui::vec2(columns[1].available_width(), 42.))
                    .truncate();
                    if columns[1]
                        .add_enabled(!app.cart.is_empty() && app.store.is_some(), checkout)
                        .clicked()
                    {
                        app.open_payment();
                    }
                });
            });
    }

    egui::CentralPanel::default()
        .frame(Frame::new().fill(palette.grid).inner_margin(4))
        .show(ctx, |ui| {
            if app.store.is_none() || !app.login.authenticated {
                return;
            }
            if app.page != 0 {
                super::page_scroll(ui, ("admin", app.page), |ui| {
                    ui.heading(PAGES[app.page]);
                    app.render_page(ui);
                });
                return;
            }
            if app.portrait_search == Search::Cart {
                ui.heading("Кошик");
                super::sale_details::cart_actions(app, ui);
                sale_table(app, ui, &palette);
                return;
            }
            let panel = match app.portrait_search {
                Search::Customers => Some(super::sale_details::Panel::Customers),
                Search::Parked => Some(super::sale_details::Panel::Parked),
                Search::Receipts => Some(super::sale_details::Panel::Receipts),
                _ => None,
            };
            if let Some(panel) = panel {
                let screen = match panel {
                    super::sale_details::Panel::Customers => "customers",
                    super::sale_details::Panel::Parked => "parked",
                    super::sale_details::Panel::Receipts => "receipts",
                };
                let receipt = if screen == "receipts" {
                    app.receipt_id
                } else {
                    None
                };
                super::page_scroll(ui, (screen, receipt), |ui| {
                    super::sale_details::show(app, ui, panel)
                });
                return;
            }
            if app.portrait_search == Search::Browse {
                ui.heading("Вибрати товар");
            }
            let barcode = app.portrait_search == Search::Barcode;
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 42.).max(50.);
                let input = ui.add_sized(
                    [width, 34.],
                    TextEdit::singleline(&mut app.search)
                        .id(search_id)
                        .hint_text(if barcode {
                            "Штрихкод"
                        } else {
                            "Назва або код"
                        }),
                );
                let enter =
                    input.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                let add = ui
                    .add_enabled(
                        !app.search.trim().is_empty(),
                        Button::new("+").min_size(egui::vec2(32., 34.)),
                    )
                    .on_hover_text("Додати товар");
                if enter || add.clicked() {
                    if barcode {
                        app.add_barcode();
                    } else {
                        add_quick_product(app);
                    }
                    if enter {
                        input.request_focus();
                    }
                }
            });
            product_list(app, ui, &palette, barcode);
        });
    if app.dock_menu_open && app.login.authenticated {
        menu_drawer(app, ctx, &palette, search_id);
    }
}

const DOCK_HEIGHT: f32 = 60.;

fn dock(app: &mut ViSales, ctx: &egui::Context, palette: &Palette, search_id: egui::Id) {
    egui::TopBottomPanel::bottom("portrait-dock")
        .exact_height(DOCK_HEIGHT)
        .frame(palette.panel(7, 5))
        .show(ctx, |ui| {
            let count = if app.access.unlocked { 3 } else { 2 };
            ui.columns(count, |columns| {
                let cart = app.page == 0 && app.portrait_search == Search::Cart;
                if dock_button(
                    &mut columns[0],
                    palette,
                    "Каса",
                    theme::LILAC,
                    app.page == 0 && !cart,
                ) {
                    ctx.memory_mut(|memory| memory.surrender_focus(search_id));
                    app.dock_menu_open = true;
                    app.cash_menu_open = true;
                    app.stock_menu_open = false;
                    app.settings_menu_open = false;
                }
                if app.access.unlocked
                    && dock_button(
                        &mut columns[1],
                        palette,
                        "Склад",
                        theme::SKY,
                        (1..=3).contains(&app.page),
                    )
                {
                    ctx.memory_mut(|memory| memory.surrender_focus(search_id));
                    app.dock_menu_open = true;
                    app.cash_menu_open = false;
                    app.stock_menu_open = true;
                    app.settings_menu_open = false;
                }
                if dock_button(
                    &mut columns[count - 1],
                    palette,
                    "Меню",
                    theme::PEACH,
                    app.dock_menu_open,
                ) {
                    ctx.memory_mut(|memory| memory.surrender_focus(search_id));
                    app.dock_menu_open = true;
                    app.cash_menu_open = false;
                    app.stock_menu_open = false;
                    app.settings_menu_open = true;
                }
            });
        });
}

fn dock_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    label: &str,
    accent: Color32,
    active: bool,
) -> bool {
    let (fill, ink, border) = palette.navigation(accent, active);
    let button = Button::new(RichText::new(label).strong().color(ink))
        .fill(fill)
        .stroke(Stroke::new(if active { 3.0_f32 } else { 2.0_f32 }, border))
        .truncate();
    ui.add_sized([ui.available_width(), 42.], button).clicked()
}

fn menu_drawer(app: &mut ViSales, ctx: &egui::Context, palette: &Palette, search_id: egui::Id) {
    let id = egui::Id::new("portrait-actions-drawer");
    let area = egui::Modal::default_area(id).anchor(egui::Align2::LEFT_TOP, egui::vec2(0., 75.));
    let mut close = false;
    let response = egui::Modal::new(id)
        .area(area)
        .frame(palette.panel(12, 8))
        .show(ctx, |ui| {
            let width = (ctx.screen_rect().width() - 28.).max(1.);
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.heading("Меню");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close |= ui.button("Закрити").clicked();
                });
            });
            ui.separator();
            let height = (ctx.screen_rect().height() - DOCK_HEIGHT - 130.).max(52.);
            egui::ScrollArea::vertical()
                .max_height(height)
                .min_scrolled_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if nav_button(
                        ui,
                        palette,
                        group_label("Каса", app.cash_menu_open),
                        app.cash_menu_open,
                    ) {
                        app.cash_menu_open = !app.cash_menu_open;
                    }
                    if app.cash_menu_open {
                        ui.indent("cash-actions", |ui| {
                            for (mode, label) in [
                                (Search::Browse, "Вибрати товар"),
                                (Search::Cart, "Кошик"),
                                (Search::Parked, "Відкладені"),
                                (Search::Customers, "Клієнти"),
                                (Search::Receipts, "Чеки"),
                                (Search::Product, "Пошук"),
                                (Search::Barcode, "Штрихкод"),
                            ] {
                                if nav_button(
                                    ui,
                                    palette,
                                    label,
                                    app.page == 0 && app.portrait_search == mode,
                                ) {
                                    app.navigate(0);
                                    if app.portrait_search != mode {
                                        app.search.clear();
                                    }
                                    app.portrait_search = mode;
                                    if matches!(mode, Search::Product | Search::Barcode) {
                                        ctx.memory_mut(|memory| memory.request_focus(search_id));
                                    }
                                    close = true;
                                }
                            }
                            #[cfg(target_os = "android")]
                            ui.add_enabled_ui(
                                app.scan_target.is_none() && app.store.is_some(),
                                |ui| {
                                    if nav_button(ui, palette, "Сканер", false) {
                                        app.navigate(0);
                                        app.start_camera(ctx, super::ScanTarget::Cart);
                                        close = true;
                                    }
                                },
                            );
                        });
                    }
                    if app.access.unlocked {
                        if nav_button(
                            ui,
                            palette,
                            group_label("Склад", app.stock_menu_open),
                            app.stock_menu_open,
                        ) {
                            app.stock_menu_open = !app.stock_menu_open;
                        }
                        if app.stock_menu_open {
                            ui.indent("stock-actions", |ui| {
                                for index in 1..=3 {
                                    let label = if index == 2 {
                                        "Залишки"
                                    } else {
                                        PAGES[index]
                                    };
                                    if nav_button(ui, palette, label, app.page == index) {
                                        app.navigate(index);
                                        close = true;
                                    }
                                }
                            });
                        }
                    }
                    if nav_button(
                        ui,
                        palette,
                        group_label("Налаштування", app.settings_menu_open),
                        app.settings_menu_open,
                    ) {
                        app.settings_menu_open = !app.settings_menu_open;
                    }
                    if app.settings_menu_open {
                        ui.indent("settings-actions", |ui| {
                            if app.access.unlocked {
                                for index in [4, 5, admin::PAGE] {
                                    if nav_button(ui, palette, PAGES[index], app.page == index) {
                                        app.navigate(index);
                                        close = true;
                                    }
                                }
                            }
                            #[cfg(target_os = "android")]
                            if nav_button(ui, palette, "Оновлення", false) {
                                app.check_updates();
                                close = true;
                            }
                            if nav_button(ui, palette, "Заблокувати", false) {
                                app.lock_screen();
                                close = true;
                            }
                            if nav_button(
                                ui,
                                palette,
                                if app.dark {
                                    "Світла тема"
                                } else {
                                    "Темна тема"
                                },
                                false,
                            ) {
                                app.toggle_theme(ctx);
                                close = true;
                            }
                        });
                    }
                });
        });
    if close || response.should_close() {
        app.dock_menu_open = false;
    }
}

fn group_label(label: &str, expanded: bool) -> String {
    format!("{} {label}", if expanded { "−" } else { "+" })
}

fn nav_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    label: impl Into<String>,
    active: bool,
) -> bool {
    let label = label.into();
    let (fill, ink, border) = palette.navigation(palette.grid, active);
    let button = Button::new(RichText::new(label).size(12.).strong().color(ink))
        .fill(fill)
        .stroke(Stroke::new(2.0_f32, border));
    ui.add_sized([ui.available_width(), 42.], button).clicked()
}

fn sale_table(app: &mut ViSales, ui: &mut egui::Ui, palette: &Palette) {
    if app.cart.is_empty() {
        ui.weak("Кошик порожній");
        return;
    }

    let ids: Vec<_> = app.cart.keys().copied().collect();
    let now = ui.input(|input| input.time);
    let highlighted = app.scanned_row.filter(|(_, time)| now - time < 1.8);
    if let Some((_, start)) = highlighted {
        if now - start >= 1.3 {
            ui.ctx().request_repaint();
        } else {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(1.3 - (now - start)));
        }
    } else {
        app.scanned_row = None;
    }
    ui.spacing_mut().item_spacing.x = 1.;
    let width = (ui.available_width() - 12.).max(150.);
    let highlight_fill = highlighted.map(|(_, start)| {
        let opacity = ((1.8 - (now - start)) / 0.5).clamp(0., 1.) as f32;
        ui.visuals().selection.bg_fill.linear_multiply(opacity)
    });
    if let Some(fill) = highlight_fill {
        ui.visuals_mut().selection.bg_fill = fill;
    }
    let mut table = TableBuilder::new(ui)
        .striped(true)
        .auto_shrink([false, false])
        .column(Column::exact(24.).clip(true))
        .column(Column::remainder().clip(true))
        .column(Column::exact(width * 0.18).clip(true))
        .column(Column::exact(width * 0.28).clip(true))
        .column(Column::exact(30.));
    if let Some(row) = app
        .scroll_to_scan
        .take()
        .and_then(|id| ids.iter().position(|candidate| *candidate == id))
    {
        table = table.scroll_to_row(row, Some(egui::Align::Center));
    }

    let mut remove = None;
    table
        .header(30., |mut header| {
            for (index, label) in ["№", "Товар", "К-сть", "Сума", ""].into_iter().enumerate()
            {
                header.col(|ui| {
                    sheet_header(
                        ui,
                        palette,
                        label,
                        index.checked_sub(1).and_then(|i| ACCENTS.get(i).copied()),
                    )
                });
            }
        })
        .body(|body| {
            body.rows(44., ids.len(), |mut row| {
                let id = ids[row.index()];
                let Some(product) = app.products.iter().find(|product| product.id == id) else {
                    return;
                };
                let Some((quantity, unit_price)) = app.cart.get_mut(&id) else {
                    return;
                };
                row.set_selected(highlighted.is_some_and(|(scanned, _)| scanned == id));
                let number = row.index() + 1;
                row.col(|ui| {
                    sheet_border(ui, palette);
                    ui.centered_and_justified(|ui| {
                        ui.weak(number.to_string());
                    });
                });
                row.col(|ui| {
                    sheet_border(ui, palette);
                    ui.add(Label::new(&product.name).truncate())
                        .on_hover_text(&product.name);
                });
                row.col(|ui| {
                    sheet_border(ui, palette);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(egui::DragValue::new(quantity).range(1..=product.stock.max(1)));
                    });
                });
                row.col(|ui| {
                    sheet_border(ui, palette);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(Label::new(money(*quantity * *unit_price)).truncate());
                    });
                });
                row.col(|ui| {
                    sheet_border(ui, palette);
                    if ui.small_button("×").on_hover_text("Видалити").clicked() {
                        remove = Some(id);
                    }
                });
            });
        });
    if let Some(id) = remove {
        app.cart.remove(&id);
    }
}

fn add_quick_product(app: &mut ViSales) {
    let query = app.search.trim().to_lowercase();
    if query.is_empty() {
        return;
    }

    let mut scoped = app
        .products
        .iter()
        .filter(|product| in_browse_category(app, product));
    let exact_barcode = scoped
        .clone()
        .find(|product| product.barcode.eq_ignore_ascii_case(&query));
    let product = if exact_barcode.is_some() {
        exact_barcode
    } else {
        let first = scoped.find(|product| matches(product, &query));
        if scoped.any(|product| matches(product, &query)) {
            app.notice = Some((false, "Виберіть потрібний товар зі списку".into()));
            return;
        }
        first
    };
    if let Some(product) = product.cloned() {
        if app.add(&product) {
            app.notice = None;
            app.search.clear();
        }
    } else {
        app.notice = Some((false, "Товарів не знайдено".into()));
    }
}

fn in_browse_category(app: &ViSales, product: &Product) -> bool {
    app.portrait_search != Search::Browse
        || app
            .browse_category
            .as_ref()
            .is_none_or(|category| product.category.as_ref() == Some(category))
}

fn category_sidebar(app: &mut ViSales, ui: &mut egui::Ui, palette: &Palette) {
    let categories: std::collections::BTreeSet<_> = app
        .products
        .iter()
        .filter_map(|product| product.category.as_deref())
        .collect();
    if app
        .browse_category
        .as_deref()
        .is_some_and(|selected| !categories.contains(selected))
    {
        app.browse_category = None;
    }
    let choices =
        std::iter::once((None, "Усі")).chain(categories.into_iter().map(|name| (Some(name), name)));
    let width = (ui.available_width() * 0.27).clamp(76., 126.);
    egui::SidePanel::left("product-categories")
        .exact_width(width)
        .resizable(false)
        .frame(palette.panel(4, 6))
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (value, title) in choices {
                        if nav_button(ui, palette, title, app.browse_category.as_deref() == value) {
                            app.browse_category = value.map(str::to_owned);
                        }
                    }
                });
        });
}

fn product_list(app: &mut ViSales, ui: &mut egui::Ui, palette: &Palette, barcode: bool) {
    if app.portrait_search == Search::Browse && !app.products.is_empty() {
        category_sidebar(app, ui, palette);
    }
    let query = app.search.trim().to_lowercase();
    let state_id = ui.make_persistent_id("pos-list-query");
    let query_changed = ui.data_mut(|data| {
        let current = (
            app.portrait_search,
            query.clone(),
            app.browse_category.clone(),
        );
        let changed = data
            .get_temp::<(Search, String, Option<String>)>(state_id)
            .as_ref()
            != Some(&current);
        if changed {
            data.insert_temp(state_id, current);
        }
        changed
    });
    let products: Vec<_> = app
        .products
        .iter()
        .filter(|product| in_browse_category(app, product))
        .filter(|product| {
            if barcode {
                product.barcode.starts_with(&query)
            } else {
                query.is_empty() || matches(product, &query)
            }
        })
        .collect();
    if products.is_empty() {
        ui.weak(if app.products.is_empty() {
            "Додайте товар у каталозі."
        } else {
            "Товарів не знайдено."
        });
        return;
    }

    let mut selected = None;
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt("pos-products")
        .auto_shrink([false, false]);
    if query_changed {
        scroll = scroll.vertical_scroll_offset(0.);
    }
    let columns = if app.portrait_search == Search::Browse {
        ((ui.available_width() / 105.) as usize).clamp(1, 4)
    } else {
        1
    };
    const HEIGHT: f32 = 91.;
    scroll.show_rows(ui, HEIGHT, products.len().div_ceil(columns), |ui, range| {
        for row in range {
            let start = row * columns;
            ui.horizontal(|ui| {
                let gap = ui.spacing().item_spacing.x;
                let width =
                    ((ui.available_width() - gap * (columns - 1) as f32) / columns as f32).max(1.);
                for product in products.iter().skip(start).take(columns) {
                    if product_card(ui, palette, product, egui::vec2(width, HEIGHT)) {
                        selected = Some((*product).clone());
                    }
                }
            });
        }
    });
    if let Some(product) = selected {
        app.add(&product);
    }
}

fn product_card(ui: &mut egui::Ui, palette: &Palette, product: &Product, size: egui::Vec2) -> bool {
    let stock = if product.stock > 0 {
        format!("{} шт.", product.stock)
    } else {
        "Немає в наявності".into()
    };
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let clickable = product.stock > 0;
    let response = ui.interact(
        rect,
        ui.make_persistent_id(("product", product.id)),
        if clickable {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let light = !ui.visuals().dark_mode;
    let surface = if light && clickable {
        let colors = [theme::LILAC, theme::MINT, theme::PEACH, theme::SKY];
        let category = product.category.as_deref().unwrap_or_default();
        let index = category
            .bytes()
            .fold(0_usize, |sum, byte| sum.wrapping_add(byte as usize));
        palette
            .rail
            .lerp_to_gamma(colors[index % colors.len()], 0.38)
    } else {
        palette.rail
    };
    let fill = if clickable && response.hovered() {
        surface.lerp_to_gamma(if light { palette.action } else { palette.ink }, 0.12)
    } else {
        surface
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0., fill);
    painter.rect_stroke(
        rect.shrink(1.),
        0.,
        Stroke::new(2.0_f32, palette.line),
        egui::StrokeKind::Inside,
    );
    let content = rect.shrink2(egui::vec2(8., 9.));
    let mut details = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(product.id)
            .max_rect(content)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    details.set_clip_rect(details.clip_rect().intersect(content));
    details.add(
        Label::new(RichText::new(&product.name).strong().color(palette.ink))
            .truncate()
            .selectable(false),
    );
    details.add(
        Label::new(
            RichText::new(money(product.price))
                .size(15.)
                .monospace()
                .strong(),
        )
        .truncate()
        .selectable(false),
    );
    details.add(
        Label::new(RichText::new(&stock).color(palette.muted))
            .truncate()
            .selectable(false),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            clickable,
            format!("{} · {} · {}", product.name, money(product.price), stock),
        )
    });
    response.on_hover_text(&product.name).clicked()
}

fn sheet_border(ui: &egui::Ui, palette: &Palette) {
    let rect = ui.available_rect_before_wrap();
    let stroke = Stroke::new(1.0_f32, palette.line);
    ui.painter().hline(rect.x_range(), rect.bottom(), stroke);
    ui.painter().vline(rect.right(), rect.y_range(), stroke);
}

fn sheet_header(ui: &mut egui::Ui, palette: &Palette, label: &str, accent: Option<Color32>) {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0., palette.rail);
    ui.painter().vline(
        rect.right(),
        rect.y_range(),
        Stroke::new(1.0_f32, palette.line),
    );
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(2.0_f32, accent.unwrap_or(palette.line)),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(12.),
        palette.ink,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Product, Store};
    use std::path::Path;

    #[test]
    fn major_portrait_pages_render_in_a_short_viewport() -> anyhow::Result<()> {
        let directory = std::env::temp_dir().join(format!("visales-pages-{}", std::process::id()));
        let mut app = ViSales::open(directory.clone());
        app.login.authenticated = true;
        app.access.unlocked = true;
        app.store()?.add_demo_products()?;
        app.refresh()?;
        let ctx = egui::Context::default();
        let frame = |app: &mut ViSales, height| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(320., height),
                    )),
                    ..Default::default()
                },
                |ctx| show(app, ctx),
            )
        };
        let contains = |output: &egui::FullOutput, label: &str| {
            output.shapes.iter().any(|shape|
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == label))
        };
        for (page, expected) in [
            (1, "Картка товару"),
            (2, "Перерахунок залишку"),
            (3, "Постачальники"),
            (4, "Виторг"),
            (5, "Обмін Excel"),
            (6, "Касири"),
        ] {
            app.navigate(page);
            let _ = frame(&mut app, 460.);
            assert!(
                contains(&frame(&mut app, 460.), expected),
                "{page}: {expected}"
            );
        }
        for (page, labels) in [
            (
                1,
                &[
                    "Штрихкод",
                    "Назва товару",
                    "Категорія",
                    "Собівартість, грн",
                    "Ціна продажу, грн",
                    "Зберегти товар",
                ][..],
            ),
            (2, &["Фактична кількість", "Зберегти залишок"][..]),
            (
                3,
                &[
                    "Назва постачальника",
                    "Номер накладної",
                    "Закупівельна ціна, грн",
                    "Оприбуткувати",
                ][..],
            ),
            (5, &["Експортувати XLSX"][..]),
            (6, &["Пароль касира", "Додати касира"][..]),
        ] {
            app.navigate(page);
            let _ = frame(&mut app, 700.);
            let output = frame(&mut app, 700.);
            for label in labels {
                assert!(
                    output.shapes.iter().any(|shape| {
                        matches!(&shape.shape, egui::Shape::Text(text)
                        if text.galley.text() == *label
                            && shape.clip_rect.contains(text.pos)
                            && shape.clip_rect.contains(text.pos + text.galley.size()))
                    }),
                    "page {page}: {label} is missing or clipped"
                );
            }
        }
        app.navigate(1);
        app.name = "Назва з чернетки".into();
        app.price = "49,90".into();
        app.apply_camera_scan(super::super::ScanTarget::Catalog, "0001234567890", 0.);
        assert_eq!(app.barcode, "0001234567890");
        assert_eq!(app.name, "Назва з чернетки");
        assert_eq!(app.price, "49,90");
        assert!(app.cart.is_empty());
        app.navigate(0);
        app.apply_camera_scan(super::super::ScanTarget::Cart, "0001234567890", 0.);
        assert_eq!(app.search, "0001234567890");
        app.search.clear();
        for (mode, expected) in [
            (Search::Browse, "Назва або код"),
            (Search::Barcode, "Штрихкод"),
            (Search::Cart, "Кошик порожній"),
            (Search::Customers, "Без клієнта"),
            (Search::Parked, "Відкладені кошики"),
            (Search::Receipts, "Тестові чеки"),
        ] {
            app.portrait_search = mode;
            let _ = frame(&mut app, 460.);
            assert!(contains(&frame(&mut app, 460.), expected), "{expected}");
        }
        drop(app);
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn portrait_chrome_stays_in_place_when_notice_and_total_change() -> anyhow::Result<()> {
        let directory = std::env::temp_dir().join(format!("visales-layout-{}", std::process::id()));
        let mut app = ViSales::open(directory.clone());
        app.login.authenticated = true;
        for width in [320., 360., 430.] {
            let ctx = egui::Context::default();
            let frame = |app: &mut ViSales| {
                ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 720.),
                        )),
                        ..Default::default()
                    },
                    |ctx| show(app, ctx),
                )
            };
            let position = |output: &egui::FullOutput, label: &str| {
                output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos),
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("Відсутня мітка {label} при ширині {width}"))
            };
            app.cart.clear();
            app.notice = None;
            let _ = frame(&mut app);
            let before = frame(&mut app);
            app.cart.insert(42, (1, 123_456_789));
            app.notice = Some((false, "Довге повідомлення про помилку має залишатися в шапці та не зміщувати кнопки і поле пошуку".into()));
            let _ = frame(&mut app);
            let after = frame(&mut app);
            let checkout = if width < 330. {
                "Розрахунок"
            } else {
                "Тестовий розрахунок"
            };
            for label in ["Каса", "Назва або код", checkout] {
                assert_eq!(
                    position(&before, label),
                    position(&after, label),
                    "{label}, {width}"
                );
            }
        }
        drop(app);
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn portrait_list_adds_a_product_and_keeps_checkout_below_it() -> anyhow::Result<()> {
        let ctx = egui::Context::default();
        ctx.set_visuals(crate::ui::visuals(true));
        let mut store = Store::open(Path::new(":memory:"))?;
        store.save_products(&[Product {
            id: 0,
            barcode: "123".into(),
            name: "Кава".into(),
            category: Some("Продукти".into()),
            cost: 4200,
            price: 6900,
            stock: 0,
        }])?;
        store.count(store.products()?[0].id, 3)?;
        let mut app = ViSales::open(
            std::env::temp_dir().join(format!("visales-portrait-{}", std::process::id())),
        );
        let directory = app.directory.clone();
        app.login.authenticated = true;
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
        assert!(text_position("№").is_none());
        assert!(text_position("3 шт.").is_some());
        let category = text_position("Продукти").expect("category appears in the left sidebar");
        let product = text_position("Кава").expect("catalogue row appears in portrait");
        assert!(category.x < product.x);
        let checkout = text_position("Тестовий розрахунок").expect("checkout stays visible");
        assert!(checkout.y > 620. && product.y < checkout.y);
        let dock_cash = text_position("Каса").expect("dock cash button appears");
        assert!(dock_cash.y > checkout.y);
        let header_cart = text_position("Кошик · 0").expect("cart button appears in the header");
        assert!(header_cart.y < product.y);

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
        for pressed in [true, false] {
            let _ = frame(vec![
                egui::Event::PointerMoved(dock_cash),
                egui::Event::PointerButton {
                    pos: dock_cash,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        let _ = frame(Vec::new());
        let sheet = frame(Vec::new());
        assert!(sheet.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Відкладені")
        }));
        assert!(app.dock_menu_open);
        app.dock_menu_open = false;
        assert_eq!(app.cart_total(), 6900);
        app.search = "кав".into();
        add_quick_product(&mut app);
        assert_eq!(app.cart_total(), 13_800);
        app.cart.clear();
        app.search = "123".into();
        add_quick_product(&mut app);
        assert_eq!(app.cart_total(), 6900);
        app.navigate(2);
        assert_eq!(app.page, 0);
        assert_eq!(app.visible_pages().collect::<Vec<_>>(), vec![0]);
        app.navigate(0);
        app.receive_scan("123", 1.0);
        assert!(app.portrait_search == Search::Cart);
        assert_eq!(app.cart_total(), 13_800);
        assert_eq!(app.scanned_row, Some((app.products[0].id, 1.0)));
        app.receive_scan("невідомий", 1.1);
        assert!(app.portrait_search == Search::Barcode);
        assert_eq!(app.cart_total(), 13_800);

        drop(app);
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }
}
