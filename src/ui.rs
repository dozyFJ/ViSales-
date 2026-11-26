mod mobile;
mod portrait_pos;

use crate::{
    exchange,
    store::{Product, Sale, Store, money, parse_money},
};
use anyhow::{Context, Result};
use eframe::egui::{self, Color32, Frame, RichText, TextEdit, Ui};
use egui_extras::{Column, TableBuilder};
use std::{collections::BTreeMap, path::PathBuf};

const PAGES: [&str; 6] = [
    "Каса",
    "Каталог",
    "Склад",
    "Поставки",
    "Звіти",
    "Інтеграції",
];
const ACCENT: Color32 = Color32::from_rgb(95, 154, 245);

pub fn visuals(dark: bool) -> egui::Visuals {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = if dark {
        Color32::from_rgb(27, 30, 36)
    } else {
        Color32::from_rgb(238, 241, 246)
    };
    v.window_fill = if dark {
        Color32::from_rgb(35, 39, 46)
    } else {
        Color32::WHITE
    };
    v.extreme_bg_color = if dark {
        Color32::from_rgb(20, 23, 28)
    } else {
        Color32::WHITE
    };
    v.selection.bg_fill = if dark {
        Color32::from_rgb(45, 78, 124)
    } else {
        Color32::from_rgb(174, 204, 249)
    };
    v
}

pub struct ViSales {
    directory: PathBuf,
    store: Option<Store>,
    products: Vec<Product>,
    sales: Vec<Sale>,
    cart: BTreeMap<i64, (i64, i64)>,
    pos_tab: usize,
    portrait_search: portrait_pos::Search,
    page: usize,
    dark: bool,
    search: String,
    catalog_search: String,
    barcode: String,
    name: String,
    cost: String,
    price: String,
    product: i64,
    supplier: i64,
    supplier_name: String,
    reference: String,
    amount: String,
    delivery_cost: String,
    import_path: String,
    refund: Option<i64>,
    notice: Option<(bool, String)>,
}

impl ViSales {
    pub fn open(directory: PathBuf) -> Self {
        let store = std::fs::create_dir_all(&directory)
            .map_err(anyhow::Error::from)
            .and_then(|()| Store::open(&directory.join("visales.sqlite3")));
        let mut app = Self {
            import_path: directory.join("ViSales.xlsx").display().to_string(),
            directory,
            store: None,
            products: Vec::new(),
            sales: Vec::new(),
            cart: BTreeMap::new(),
            pos_tab: 0,
            portrait_search: portrait_pos::Search::Product,
            page: 0,
            dark: true,
            search: String::new(),
            catalog_search: String::new(),
            barcode: String::new(),
            name: String::new(),
            cost: String::new(),
            price: String::new(),
            product: 0,
            supplier: 0,
            supplier_name: String::new(),
            reference: String::new(),
            amount: String::new(),
            delivery_cost: String::new(),
            refund: None,
            notice: None,
        };
        match store {
            Ok(store) => {
                app.store = Some(store);
                if let Err(error) = app.refresh() {
                    app.notice = Some((false, format!("{error:#}")));
                }
            }
            Err(error) => {
                app.notice = Some((false, format!("Не вдалося відкрити сховище: {error:#}")))
            }
        }
        app
    }

    fn store(&mut self) -> Result<&mut Store> {
        self.store.as_mut().context("Локальне сховище недоступне")
    }

    fn refresh(&mut self) -> Result<()> {
        let store = self.store()?;
        let products = store.products()?;
        let sales = store.sales()?;
        self.products = products;
        self.sales = sales;
        Ok(())
    }

    fn result(&mut self, result: Result<String>) {
        self.notice = Some(match result {
            Ok(message) => (true, message),
            Err(error) => (false, format!("{error:#}")),
        });
    }

    fn add(&mut self, p: &Product) {
        let entry = self.cart.entry(p.id).or_insert((0, p.price));
        if entry.0 >= p.stock {
            self.notice = Some((false, "Недостатній залишок".into()));
        } else {
            entry.0 += 1;
            entry.1 = p.price;
        }
    }

    fn pos(&mut self, ui: &mut Ui) {
        let search = ui.add(
            TextEdit::singleline(&mut self.search)
                .desired_width(f32::INFINITY)
                .hint_text("Знайти товар або ввести штрихкод"),
        );
        if search.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.add_barcode();
            search.request_focus();
        }

        if ui.available_width() > 730. {
            ui.columns(2, |columns| {
                self.pos_products(&mut columns[0]);
                self.pos_cart(&mut columns[1]);
            });
        } else {
            let cart_count: i64 = self.cart.values().map(|(quantity, _)| quantity).sum();
            let cart_label = format!("Кошик ({cart_count})");
            mobile::pills(ui, &mut self.pos_tab, &["Товари", &cart_label], self.dark);
            if self.pos_tab == 0 {
                self.pos_products(ui);
            } else {
                self.pos_cart(ui);
            }
        }
    }

    fn pos_products(&mut self, ui: &mut Ui) {
        ui.heading("Товари");
        let query = self.search.trim().to_lowercase();
        let products: Vec<_> = self
            .products
            .iter()
            .filter(|p| matches(p, &query))
            .cloned()
            .collect();
        if products.is_empty() {
            ui.weak("Додайте товар у каталозі або змініть пошук.");
        }
        for p in products {
            if ui
                .add_enabled(
                    p.stock > 0,
                    egui::Button::new(format!(
                        "{}   ·   {}   ·   {} шт.",
                        p.name,
                        money(p.price),
                        p.stock
                    ))
                    .wrap(),
                )
                .clicked()
            {
                self.add(&p);
            }
        }
    }

    fn pos_cart(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.heading("Поточний продаж");
            self.cart_lines(ui);
            ui.separator();
            let total = self.cart_total();
            ui.label(RichText::new(money(total)).size(30.).strong());
            if ui
                .add_enabled(
                    !self.cart.is_empty(),
                    egui::Button::new("Записати тестовий готівковий продаж"),
                )
                .clicked()
            {
                self.checkout();
            }
        });
    }

    fn add_barcode(&mut self) {
        if let Some(product) = self
            .products
            .iter()
            .find(|p| p.barcode == self.search.trim())
            .cloned()
        {
            self.add(&product);
            self.search.clear();
        } else if !self.search.trim().is_empty() {
            self.notice = Some((false, "Штрихкод не знайдено".into()));
        }
    }

    fn cart_total(&self) -> i64 {
        self.cart
            .values()
            .map(|(quantity, price)| quantity * price)
            .sum()
    }

    fn cart_lines(&mut self, ui: &mut Ui) {
        let mut remove = None;
        for (&id, (quantity, price)) in &mut self.cart {
            let Some(product) = self.products.iter().find(|p| p.id == id) else {
                continue;
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(&product.name);
                ui.add(egui::DragValue::new(quantity).range(1..=1_000_000));
                ui.label(money(*quantity * *price));
                if ui.small_button("Видалити").clicked() {
                    remove = Some(id);
                }
            });
        }
        if let Some(id) = remove {
            self.cart.remove(&id);
        }
        if self.cart.is_empty() {
            ui.weak("Кошик порожній");
        }
    }

    fn checkout(&mut self) {
        let result = (|| {
            let id = self
                .store
                .as_mut()
                .context("Сховище недоступне")?
                .sell(&self.cart)?;
            self.cart.clear();
            self.refresh()?;
            Ok(format!("Тестовий продаж №{id} записано локально"))
        })();
        self.result(result);
    }

    fn catalog(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.heading("Картка товару");
            ui.horizontal_wrapped(|ui| {
                field(ui, &mut self.barcode, "Штрихкод");
                field(ui, &mut self.name, "Назва товару");
                field(ui, &mut self.cost, "Собівартість, грн");
                field(ui, &mut self.price, "Ціна продажу, грн");
                if ui.button("Зберегти товар").clicked() {
                    let result = (|| {
                        let product = Product {
                            id: 0,
                            stock: 0,
                            barcode: self.barcode.clone(),
                            name: self.name.clone(),
                            cost: parse_money(&self.cost)?,
                            price: parse_money(&self.price)?,
                        };
                        self.store()?.save_products(&[product])?;
                        self.refresh()?;
                        Ok("Картку товару збережено".into())
                    })();
                    self.result(result);
                }
            });
        });
        ui.add(
            TextEdit::singleline(&mut self.catalog_search)
                .hint_text("Пошук за назвою чи штрихкодом"),
        );
        let query = self.catalog_search.trim().to_lowercase();
        let mut selected = None;
        let products: Vec<_> = self
            .products
            .iter()
            .filter(|p| matches(p, &query))
            .collect();
        table(
            ui,
            &["Штрихкод", "Назва", "Собівартість", "Ціна", "Залишок"],
            products.len(),
            |ui, row, col| {
                let p = products[row];
                match col {
                    0 => {
                        if ui.link(&p.barcode).clicked() {
                            selected = Some(p.clone());
                        }
                    }
                    1 => {
                        ui.label(&p.name);
                    }
                    2 => {
                        ui.label(money(p.cost));
                    }
                    3 => {
                        ui.label(money(p.price));
                    }
                    _ => {
                        ui.label(p.stock.to_string());
                    }
                }
            },
        );
        if let Some(product) = selected {
            self.barcode = product.barcode;
            self.name = product.name;
            self.cost = decimal(product.cost);
            self.price = decimal(product.price);
        }
    }

    fn stock(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.heading("Перерахунок залишку");
            ui.horizontal_wrapped(|ui| {
                product_select(ui, "count-product", &self.products, &mut self.product);
                field(ui, &mut self.amount, "Фактична кількість");
                if ui.button("Зберегти залишок").clicked() {
                    let result = (|| {
                        let quantity = self
                            .amount
                            .trim()
                            .parse()
                            .context("Вкажіть цілу кількість")?;
                        let product = self.product;
                        self.store()?.count(product, quantity)?;
                        self.refresh()?;
                        self.amount.clear();
                        Ok("Залишок оновлено".into())
                    })();
                    self.result(result);
                }
            });
        });
        ui.heading("Залишки");
        table(
            ui,
            &["Штрихкод", "Товар", "Кількість"],
            self.products.len(),
            |ui, row, col| {
                let p = &self.products[row];
                ui.label(match col {
                    0 => p.barcode.clone(),
                    1 => p.name.clone(),
                    _ => p.stock.to_string(),
                });
            },
        );
        ui.heading("Історія перерахунків");
        match self.store().and_then(|store| store.stock_history()) {
            Ok(history) => table(
                ui,
                &["Дата UTC", "Товар", "Було", "Стало"],
                history.len(),
                |ui, row, col| {
                    let (date, name, previous, counted) = &history[row];
                    ui.label(match col {
                        0 => date.clone(),
                        1 => name.clone(),
                        2 => previous.to_string(),
                        _ => counted.to_string(),
                    });
                },
            ),
            Err(error) => {
                ui.label(error.to_string());
            }
        }
    }

    fn supplies(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.heading("Постачальники");
            ui.horizontal_wrapped(|ui| {
                field(ui, &mut self.supplier_name, "Назва постачальника");
                if ui.button("Додати постачальника").clicked() {
                    let result = (|| {
                        self.store
                            .as_ref()
                            .context("Сховище недоступне")?
                            .supplier(&self.supplier_name)?;
                        self.supplier_name.clear();
                        Ok("Постачальника збережено".into())
                    })();
                    self.result(result);
                }
            });
        });
        let suppliers = match self.store().and_then(|store| store.suppliers()) {
            Ok(suppliers) => suppliers,
            Err(error) => {
                ui.label(error.to_string());
                return;
            }
        };
        card(ui, |ui| {
            ui.heading("Надходження за накладною");
            ui.horizontal_wrapped(|ui| {
                let supplier = suppliers.iter().find(|(id, _)| *id == self.supplier);
                egui::ComboBox::from_id_salt("supplier")
                    .selected_text(
                        supplier
                            .map(|(_, name)| name.as_str())
                            .unwrap_or("Постачальник"),
                    )
                    .show_ui(ui, |ui| {
                        for (id, name) in &suppliers {
                            ui.selectable_value(&mut self.supplier, *id, name);
                        }
                    });
                product_select(ui, "delivery-product", &self.products, &mut self.product);
                field(ui, &mut self.reference, "Номер накладної");
                field(ui, &mut self.amount, "Кількість");
                field(ui, &mut self.delivery_cost, "Закупівельна ціна, грн");
                if ui.button("Оприбуткувати").clicked() {
                    let result = (|| {
                        let quantity = self
                            .amount
                            .trim()
                            .parse()
                            .context("Вкажіть цілу кількість")?;
                        let cost = parse_money(&self.delivery_cost)?;
                        anyhow::ensure!(
                            self.supplier != 0 && self.product != 0,
                            "Виберіть постачальника та товар"
                        );
                        self.store.as_mut().context("Сховище недоступне")?.receive(
                            self.supplier,
                            &self.reference,
                            self.product,
                            quantity,
                            cost,
                        )?;
                        self.refresh()?;
                        self.amount.clear();
                        Ok("Позицію накладної проведено".into())
                    })();
                    self.result(result);
                }
            });
        });
        ui.heading("Журнал надходжень");
        match self.store().and_then(|store| store.deliveries()) {
            Ok(deliveries) => table(
                ui,
                &["Накладна", "Постачальник", "Товар", "Кількість", "Ціна"],
                deliveries.len(),
                |ui, row, col| {
                    let (reference, supplier, product, quantity, cost) = &deliveries[row];
                    ui.label(match col {
                        0 => reference.clone(),
                        1 => supplier.clone(),
                        2 => product.clone(),
                        3 => quantity.to_string(),
                        _ => money(*cost),
                    });
                },
            ),
            Err(error) => {
                ui.label(error.to_string());
            }
        }
    }

    fn reports(&mut self, ui: &mut Ui) {
        let active = self.sales.iter().filter(|s| s.returned.is_none());
        let totals = active.fold((0, 0, 0), |(n, total, margin), s| {
            (n + 1, total + s.total, margin + s.margin)
        });
        ui.horizontal_wrapped(|ui| {
            for (label, value) in [
                ("Виторг", money(totals.1)),
                ("Валовий прибуток", money(totals.2)),
                ("Продажі", totals.0.to_string()),
            ] {
                card(ui, |ui| {
                    ui.set_min_width(132.);
                    ui.weak(label);
                    ui.heading(value);
                });
            }
        });
        ui.heading("Журнал тестових продажів");
        table(
            ui,
            &["№", "Дата UTC", "Сума", "Прибуток", "Статус / дія"],
            self.sales.len(),
            |ui, row, col| {
                let s = &self.sales[row];
                match col {
                    0 => {
                        ui.label(s.id.to_string());
                    }
                    1 => {
                        ui.label(&s.created);
                    }
                    2 => {
                        ui.label(money(s.total));
                    }
                    3 => {
                        ui.label(money(s.margin));
                    }
                    _ if s.returned.is_some() => {
                        ui.label("Повернено");
                    }
                    _ => {
                        if ui.small_button("Повернути").clicked() {
                            self.refund = Some(s.id);
                        }
                    }
                }
            },
        );
        if let Some(id) = self.refund {
            card(ui, |ui| {
                ui.label(format!("Повернути весь тестовий продаж №{id}?"));
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Підтвердити").clicked() {
                        let result = (|| {
                            self.store()?.refund(id)?;
                            self.refund = None;
                            self.refresh()?;
                            Ok("Повернення записано, залишки відновлено".into())
                        })();
                        self.result(result);
                    }
                    if ui.button("Скасувати").clicked() {
                        self.refund = None;
                    }
                });
            });
        }
    }

    fn integrations(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.heading("Обмін Excel");
            ui.label("Перший аркуш — каталог. Імпорт оновлює картки за штрихкодом. Залишки змінюються через склад.");
            ui.add(
                TextEdit::singleline(&mut self.import_path)
                    .desired_width(f32::INFINITY)
                    .hint_text("Шлях до книги Excel"),
            );
            ui.horizontal_wrapped(|ui| {
                if ui.button("Імпортувати каталог").clicked() {
                    let result = (|| {
                        let rows = exchange::import(std::path::Path::new(self.import_path.trim()))?;
                        self.store()?.save_products(&rows)?;
                        self.refresh()?;
                        Ok(format!("Імпортовано товарів: {}", rows.len()))
                    })();
                    self.result(result);
                }
                if ui.button("Експортувати XLSX").clicked() {
                    let path = self.directory.join("ViSales.xlsx");
                    let result = exchange::export(&path, &self.products, &self.sales)
                        .map(|()| format!("Файл збережено: {}", path.display()));
                    self.result(result);
                }
            });
        });
        for (title, detail) in [
            (
                "ПРРО · ДПС України",
                "Тестовий режим · Фіскальні документи не надсилаються.",
            ),
            (
                "ПриватБанк · Термінал",
                "Тестовий режим · NFC-оплата недоступна до підключення API.",
            ),
            ("OpenCart", "Підключення магазину недоступне в цій збірці."),
            (
                "Синхронізація пристроїв",
                "Дані зберігаються у SQLite на цьому пристрої.",
            ),
        ] {
            card(ui, |ui| {
                ui.heading(title);
                ui.weak(detail);
            });
        }
    }

    fn desktop_nav(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("ViSales").size(28.).strong());
        ui.add_space(10.);
        for (index, label) in PAGES.iter().enumerate() {
            ui.selectable_value(&mut self.page, index, *label);
        }
        if ui
            .small_button(if self.dark {
                "Світла тема"
            } else {
                "Темна тема"
            })
            .clicked()
        {
            self.dark = !self.dark;
            ui.ctx().set_visuals(visuals(self.dark));
        }
    }
}

impl eframe::App for ViSales {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let screen = ctx.screen_rect();
        let portrait = screen.width() < screen.height();
        if portrait && self.page == 0 {
            portrait_pos::show(self, ctx);
            return;
        }
        let mobile_layout = cfg!(target_os = "android") || portrait || screen.width() < 840.;
        if mobile_layout {
            mobile::shell(self, ctx, portrait || screen.width() < 520.);
        } else {
            egui::SidePanel::left("nav")
                .resizable(false)
                .exact_width(173.)
                .show(ctx, |ui| self.desktop_nav(ui));
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10., 9.);
            if mobile_layout {
                ui.spacing_mut().interact_size.y = 38.;
            }
            ui.label(
                RichText::new("ТЕСТОВИЙ РЕЖИМ  ·  ОФЛАЙН  ·  UAH")
                    .color(if self.dark {
                        ACCENT
                    } else {
                        Color32::from_rgb(36, 79, 142)
                    })
                    .strong(),
            );
            ui.label(RichText::new(PAGES[self.page]).size(30.).strong());
            if let Some((ok, status)) = &self.notice {
                ui.label(RichText::new(status).color(if *ok {
                    ui.visuals().text_color()
                } else {
                    ui.visuals().error_fg_color
                }));
            }
            egui::ScrollArea::vertical()
                .id_salt(self.page)
                .show(ui, |ui| {
                    if self.store.is_none() {
                        return;
                    }
                    match self.page {
                        0 => self.pos(ui),
                        1 => self.catalog(ui),
                        2 => self.stock(ui),
                        3 => self.supplies(ui),
                        4 => self.reports(ui),
                        _ => self.integrations(ui),
                    }
                });
        });
    }
}

fn card(ui: &mut Ui, content: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(9)
        .inner_margin(13)
        .show(ui, content);
}

fn field(ui: &mut Ui, value: &mut String, label: &str) {
    ui.vertical(|ui| {
        ui.label(label);
        ui.add(TextEdit::singleline(value).desired_width(148.));
    });
}

fn product_select(ui: &mut Ui, id: &str, products: &[Product], selected: &mut i64) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(
            products
                .iter()
                .find(|p| p.id == *selected)
                .map(|p| p.name.as_str())
                .unwrap_or("Виберіть товар"),
        )
        .show_ui(ui, |ui| {
            for product in products {
                ui.selectable_value(
                    selected,
                    product.id,
                    format!("{} · {}", product.name, product.barcode),
                );
            }
        });
}

fn table(ui: &mut Ui, columns: &[&str], rows: usize, mut cell: impl FnMut(&mut Ui, usize, usize)) {
    ui.push_id(columns, |ui| {
        egui::ScrollArea::horizontal().show(ui, |ui| {
            let width = (ui.available_width() / columns.len() as f32 - 10.).max(105.);
            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .vscroll(false)
                .columns(
                    Column::initial(width).at_least(85.).clip(true),
                    columns.len(),
                )
                .header(26., |mut header| {
                    for label in columns {
                        header.col(|ui| {
                            ui.strong(*label);
                        });
                    }
                })
                .body(|body| {
                    body.rows(26., rows, |mut row| {
                        let index = row.index();
                        for col in 0..columns.len() {
                            row.col(|ui| cell(ui, index, col));
                        }
                    });
                });
        });
    });
    if rows == 0 {
        ui.weak("Записів ще немає.");
    }
}

fn matches(p: &Product, query: &str) -> bool {
    p.barcode.to_lowercase().contains(query) || p.name.to_lowercase().contains(query)
}

fn decimal(value: i64) -> String {
    format!("{},{:02}", value / 100, value % 100)
}
