mod admin;
mod login;
mod mobile;
mod payment;
mod portrait_pos;
mod sale_details;
mod theme;

use crate::{
    exchange,
    store::{Product, Sale, Store, money, parse_money},
};
use anyhow::{Context, Result};
use eframe::egui::{self, Color32, Frame, RichText, TextEdit, Ui};
use egui_extras::{Column, TableBuilder};
use std::{collections::BTreeMap, path::PathBuf};

const PAGES: [&str; 7] = [
    "Каса",
    "Каталог",
    "Склад",
    "Поставки",
    "Звіти",
    "Інтеграції",
    "Адмінка",
];
const ACCENT: Color32 = theme::SIGNAL;
pub const DEFAULT_DARK: bool = false;

pub fn visuals(dark: bool) -> egui::Visuals {
    theme::visuals(dark)
}

pub struct ViSales {
    directory: PathBuf,
    splash_started: std::time::Instant,
    store: Option<Store>,
    products: Vec<Product>,
    sales: Vec<Sale>,
    cart: BTreeMap<i64, (i64, i64)>,
    customer: Option<i64>,
    payment_form: Option<payment::Form>,
    customers: Vec<(i64, String)>,
    customer_name: String,
    parked_cart: Option<crate::store::ParkedRef>,
    desktop_sale_panel: Option<sale_details::Panel>,
    receipt_id: Option<i64>,
    pos_tab: usize,
    scanned_row: Option<(i64, f64)>,
    scroll_to_scan: Option<i64>,
    #[cfg(target_os = "android")]
    android_app: Option<winit::platform::android::activity::AndroidApp>,
    #[cfg(target_os = "android")]
    scanning: bool,
    portrait_search: portrait_pos::Search,
    page: usize,
    access: admin::Access,
    login: login::Login,
    dock_menu_open: bool,
    cash_menu_open: bool,
    settings_menu_open: bool,
    stock_menu_open: bool,
    pub(crate) dark: bool,
    search: String,
    catalog_search: String,
    barcode: String,
    product_category: String,
    browse_category: Option<String>,
    name: String,
    cost: String,
    price: String,
    product: i64,
    supplier: i64,
    supplier_name: String,
    reference: String,
    amount: String,
    delivery_cost: String,
    fiscal_request_path: String,
    fiscal_status: Option<std::sync::mpsc::Receiver<Result<String>>>,
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
            fiscal_request_path: String::new(),
            fiscal_status: None,
            import_path: directory.join("ViSales.xlsx").display().to_string(),
            directory,
            splash_started: std::time::Instant::now(),
            store: None,
            products: Vec::new(),
            sales: Vec::new(),
            cart: BTreeMap::new(),
            customer: None,
            payment_form: None,
            customers: Vec::new(),
            customer_name: String::new(),
            parked_cart: None,
            desktop_sale_panel: None,
            receipt_id: None,
            pos_tab: 0,
            scanned_row: None,
            scroll_to_scan: None,
            #[cfg(target_os = "android")]
            android_app: None,
            #[cfg(target_os = "android")]
            scanning: false,
            portrait_search: portrait_pos::Search::Browse,
            page: 0,
            access: admin::Access::default(),
            login: login::Login::default(),
            dock_menu_open: false,
            cash_menu_open: true,
            settings_menu_open: false,
            stock_menu_open: false,
            dark: DEFAULT_DARK,
            search: String::new(),
            catalog_search: String::new(),
            barcode: String::new(),
            product_category: String::new(),
            browse_category: None,
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
                match store.dark_theme() {
                    Ok(dark) => app.dark = dark.unwrap_or(DEFAULT_DARK),
                    Err(error) => {
                        app.notice = Some((false, format!("Не вдалося прочитати тему: {error}")))
                    }
                }
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
        let customers = store.customers()?;
        self.products = products;
        self.sales = sales;
        self.customers = customers;
        Ok(())
    }

    fn result(&mut self, result: Result<String>) {
        self.notice = Some(match result {
            Ok(message) => (true, message),
            Err(error) => (false, format!("{error:#}")),
        });
    }

    fn add(&mut self, p: &Product) -> bool {
        let quantity = self.cart.get(&p.id).map_or(0, |entry| entry.0);
        if quantity >= p.stock {
            self.notice = Some((false, "Недостатній залишок".into()));
            return false;
        }
        self.cart.insert(p.id, (quantity + 1, p.price));
        true
    }

    #[cfg(target_os = "android")]
    pub fn with_android_app(
        mut self,
        app: Option<winit::platform::android::activity::AndroidApp>,
    ) -> Self {
        self.android_app = app;
        self
    }

    #[cfg(target_os = "android")]
    fn start_camera(&mut self, ctx: &egui::Context) {
        if self.scanning || self.store.is_none() {
            return;
        }
        if self
            .android_app
            .as_ref()
            .is_some_and(|app| crate::android_scanner::start(app).is_ok())
        {
            self.scanning = true;
            ctx.request_repaint();
        } else {
            self.notice = Some((false, "Не вдалося запустити сканер".into()));
        }
    }

    #[cfg(target_os = "android")]
    fn poll_camera(&mut self, ctx: &egui::Context) {
        if !self.scanning {
            return;
        }
        let Some(app) = &self.android_app else {
            return;
        };
        let result = match crate::android_scanner::take(app) {
            Ok(None) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(80));
                return;
            }
            Ok(Some(result)) => result,
            Err(_) => "error:Не вдалося прочитати результат сканування".into(),
        };
        self.scanning = false;
        if let Some(barcode) = result.strip_prefix("code:") {
            self.receive_scan(barcode, ctx.input(|input| input.time));
        } else if let Some(error) = result.strip_prefix("error:") {
            self.notice = Some((false, error.into()));
        }
    }

    fn pos(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            for (panel, label) in [
                (sale_details::Panel::Customers, "Клієнти"),
                (sale_details::Panel::Parked, "Відкладені"),
                (sale_details::Panel::Receipts, "Чеки"),
            ] {
                if ui
                    .selectable_label(self.desktop_sale_panel == Some(panel), label)
                    .clicked()
                {
                    self.desktop_sale_panel = if self.desktop_sale_panel == Some(panel) {
                        None
                    } else {
                        Some(panel)
                    };
                }
            }
        });
        if let Some(panel) = self.desktop_sale_panel {
            sale_details::show(self, ui, panel);
            return;
        }
        #[cfg(target_os = "android")]
        if ui
            .add_enabled(!self.scanning, egui::Button::new("Сканер"))
            .clicked()
        {
            self.start_camera(ui.ctx());
        }
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
            sale_details::cart_actions(self, ui);
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
                self.open_payment();
            }
        });
    }

    fn add_barcode(&mut self) -> Option<i64> {
        if let Some(product) = self
            .products
            .iter()
            .find(|p| p.barcode == self.search.trim())
            .cloned()
        {
            self.search.clear();
            return self.add(&product).then_some(product.id);
        }
        if !self.search.trim().is_empty() {
            self.notice = Some((false, "Штрихкод не знайдено".into()));
        }
        None
    }

    #[cfg(any(target_os = "android", test))]
    fn receive_scan(&mut self, barcode: &str, time: f64) {
        self.search = barcode.into();
        self.notice = None;
        if let Some(id) = self.add_barcode() {
            self.portrait_search = portrait_pos::Search::Cart;
            self.pos_tab = 1;
            self.scanned_row = Some((id, time));
            self.scroll_to_scan = Some(id);
            self.notice = Some((true, "Товар додано до поточного продажу".into()));
        } else {
            self.portrait_search = portrait_pos::Search::Barcode;
            self.pos_tab = 0;
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

    fn open_payment(&mut self) {
        self.notice = None;
        self.payment_form = Some(payment::Form::new(self.customer));
    }

    fn checkout(&mut self, payment: crate::store::Payment, customer: Option<i64>) {
        let result = (|| {
            let id = self.store.as_mut().context("Сховище недоступне")?.sell(
                &self.cart,
                customer,
                self.parked_cart,
                payment,
            )?;
            self.cart.clear();
            self.customer = None;
            self.parked_cart = None;
            self.refresh()?;
            self.receipt_id = Some(id);
            self.desktop_sale_panel = Some(sale_details::Panel::Receipts);
            self.portrait_search = portrait_pos::Search::Receipts;
            Ok(format!("Тестовий продаж №{id} записано локально"))
        })();
        self.result(result);
    }

    fn catalog(&mut self, ui: &mut Ui) {
        if ui.button("Додати тестові товари").clicked() {
            let result = (|| {
                let added = self.store()?.add_demo_products()?;
                self.refresh()?;
                Ok(if added == 0 {
                    "Тестові товари вже є в каталозі".into()
                } else {
                    format!("Додано тестових товарів: {added}")
                })
            })();
            self.result(result);
        }
        card(ui, |ui| {
            ui.heading("Картка товару");
            ui.horizontal_wrapped(|ui| {
                field(ui, &mut self.barcode, "Штрихкод");
                field(ui, &mut self.name, "Назва товару");
                field(ui, &mut self.product_category, "Категорія");
                field(ui, &mut self.cost, "Собівартість, грн");
                field(ui, &mut self.price, "Ціна продажу, грн");
                if ui.button("Зберегти товар").clicked() {
                    let result = (|| {
                        let product = Product {
                            id: 0,
                            stock: 0,
                            barcode: self.barcode.clone(),
                            name: self.name.clone(),
                            category: Some(self.product_category.clone()),
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
            self.product_category = product.category.unwrap_or_default();
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
            &[
                "№",
                "Дата UTC",
                "Сума",
                "Прибуток",
                "Клієнт",
                "Оплата",
                "Статус / дія",
            ],
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
                    4 => {
                        ui.label(s.customer.as_deref().unwrap_or("—"));
                    }
                    5 => {
                        let text = s
                            .payment
                            .as_ref()
                            .map_or_else(|| "—".into(), payment::label);
                        ui.label(&text).on_hover_text(text);
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

    #[cfg(target_os = "android")]
    fn check_updates(&mut self) {
        if !self
            .android_app
            .as_ref()
            .is_some_and(|app| crate::android_scanner::check_updates(app).is_ok())
        {
            self.notice = Some((false, "Не вдалося запустити перевірку оновлень".into()));
        }
    }

    fn integrations(&mut self, ui: &mut Ui) {
        #[cfg(target_os = "android")]
        if ui.button("Перевірити оновлення ViSales").clicked() {
            self.check_updates();
        }
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
        card(ui, |ui| {
            ui.heading("ПРРО · Тестовий сервер ДПС");
            ui.label("Перевірка стану зміни за підписаним номером ПРРО.");
            ui.add(
                TextEdit::singleline(&mut self.fiscal_request_path)
                    .desired_width(f32::INFINITY)
                    .hint_text("Шлях до CMS SignedData"),
            );
            ui.weak("Виберіть підписаний запит. Файл ключа та пароль залишаються на пристрої.");
            let enabled =
                self.fiscal_status.is_none() && !self.fiscal_request_path.trim().is_empty();
            if ui
                .add_enabled(enabled, egui::Button::new("Запитати статус"))
                .clicked()
            {
                let path = PathBuf::from(self.fiscal_request_path.trim());
                let (send, receive) = std::sync::mpsc::channel();
                let ctx = ui.ctx().clone();
                self.notice = None;
                self.fiscal_status = Some(receive);
                std::thread::spawn(move || {
                    let _ = send.send(crate::fiscal::test_status(&path));
                    ctx.request_repaint();
                });
            }
            if self.fiscal_status.is_some() {
                ui.spinner();
            }
        });
        for (title, detail) in [
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

    fn visible_pages(&self) -> impl Iterator<Item = usize> + use<> {
        let admin = self.access.unlocked;
        (0..PAGES.len()).filter(move |&page| page == 0 || admin)
    }

    fn navigate(&mut self, page: usize) {
        self.access.clear_fields();
        self.page = if page != 0 && !self.access.unlocked {
            0
        } else {
            page
        };
        self.notice = None;
    }

    fn render_page(&mut self, ui: &mut Ui) {
        // Enforce role access at rendering as well as navigation.
        if self.page != 0 && !self.access.unlocked {
            return;
        }
        match self.page {
            0 => self.pos(ui),
            1 => self.catalog(ui),
            2 => self.stock(ui),
            3 => self.supplies(ui),
            4 => self.reports(ui),
            5 => self.integrations(ui),
            _ => admin::show(self, ui),
        }
    }

    fn lock_screen(&mut self) {
        self.login = login::Login::default();
        self.access = admin::Access::default();
        self.payment_form = None;
        self.dock_menu_open = false;
        self.refund = None;
        self.navigate(0);
    }

    fn toggle_theme(&mut self, ctx: &egui::Context) {
        let dark = !self.dark;
        if let Some(store) = &self.store {
            if let Err(error) = store.set_dark_theme(dark) {
                self.notice = Some((false, format!("Не вдалося зберегти тему: {error}")));
                return;
            }
        }
        self.dark = dark;
        ctx.set_visuals(visuals(dark));
    }

    fn desktop_nav(&mut self, ui: &mut Ui) {
        if ui.small_button("Заблокувати").clicked() {
            self.lock_screen();
        }
        ui.label(RichText::new("ViSales").size(28.).strong());
        ui.add_space(10.);
        for index in self.visible_pages() {
            if ui
                .selectable_label(self.page == index, PAGES[index])
                .clicked()
            {
                self.navigate(index);
            }
        }
        if ui
            .small_button(if self.dark {
                "Світла тема"
            } else {
                "Темна тема"
            })
            .clicked()
        {
            self.toggle_theme(ui.ctx());
        }
    }
}

impl eframe::App for ViSales {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        #[cfg(target_os = "android")]
        self.poll_camera(ctx);
        if let Some(receiver) = &self.fiscal_status {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err(anyhow::anyhow!("Запит до ДПС перервався")))
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                self.fiscal_status = None;
                if self.access.unlocked && self.page == 5 {
                    self.result(result);
                }
            }
        }
        if mobile::startup_splash(ctx, self.splash_started.elapsed().as_secs_f32(), self.dark) {
            return;
        }
        if !self.login.authenticated {
            login::show(self, ctx);
            return;
        }
        let screen = ctx.screen_rect();
        let portrait = screen.width() < screen.height();
        #[cfg(target_os = "android")]
        let portrait = match self
            .android_app
            .as_ref()
            .map(|app| app.config().orientation())
        {
            Some(ndk::configuration::Orientation::Port) => true,
            Some(ndk::configuration::Orientation::Land) => false,
            _ => portrait,
        };
        if portrait {
            portrait_pos::show(self, ctx);
            if self.login.authenticated {
                payment::show(self, ctx);
            }
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
        if !self.login.authenticated {
            return;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10., 9.);
            if mobile_layout {
                ui.spacing_mut().interact_size.y = 38.;
            }
            ui.label(
                RichText::new("ТЕСТОВИЙ РЕЖИМ  ·  ОФЛАЙН  ·  UAH")
                    .color(if self.dark { ACCENT } else { theme::BLACK })
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
            let desk_panel = self.desktop_sale_panel.map(|panel| match panel {
                sale_details::Panel::Customers => "customers",
                sale_details::Panel::Parked => "parked",
                sale_details::Panel::Receipts => "receipts",
            });
            page_scroll(ui, (self.page, desk_panel, self.receipt_id), |ui| {
                if self.store.is_some() {
                    self.render_page(ui);
                }
            });
        });
        payment::show(self, ctx);
    }
}

fn page_scroll(ui: &mut Ui, id: impl std::hash::Hash, content: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            content(ui);
            ui.add_space(16.);
        });
}

fn card(ui: &mut Ui, content: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
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
        .width(ui.available_width().min(250.))
        .truncate()
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
        if ui.available_width() < 520. {
            for row in 0..rows {
                ui.push_id(row, |ui| {
                    card(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        for (column, label) in columns.iter().enumerate() {
                            ui.horizontal_wrapped(|ui| {
                                ui.weak(*label);
                                cell(ui, row, column);
                            });
                        }
                    });
                });
            }
            return;
        }
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

#[cfg(test)]
mod scroll_tests {
    use super::*;

    #[test]
    fn narrow_records_reach_the_last_action_with_vertical_scroll() {
        let ctx = egui::Context::default();
        let pointer = egui::pos2(110., 140.);
        let mut time = 0.;
        let mut frame = |events| {
            time += 0.033;
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(216., 320.),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        page_scroll(ui, "narrow-stock", |ui| {
                            table(
                                ui,
                                &["Товар", "Кількість", "Дія"],
                                30,
                                |ui, row, column| match column {
                                    0 => {
                                        ui.label(format!("Товар №{}", row + 1));
                                    }
                                    1 => {
                                        ui.label("24");
                                    }
                                    _ => {
                                        let _ = ui.button(format!("Дія №{}", row + 1));
                                    }
                                },
                            );
                        });
                    });
                },
            )
        };
        let visible = |output: &egui::FullOutput, label: &str| {
            output.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    shape.clip_rect.contains(text.pos + text.galley.size() / 2.)
                }
                _ => false,
            })
        };
        let _ = frame(vec![egui::Event::PointerMoved(pointer)]);
        let before = frame(Vec::new());
        assert!(visible(&before, "Товар №1"));
        assert!(!visible(&before, "Дія №30"));
        let _ = frame(vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., -12_000.),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        for _ in 0..20 {
            let _ = frame(Vec::new());
        }
        let after = frame(Vec::new());
        assert!(visible(&after, "Дія №30"));
        assert!(!visible(&after, "Товар №1"));
    }
}
