use super::{PAGES, ViSales, money};
use anyhow::{Result, ensure};
use eframe::egui::{TextEdit, Ui};

pub(super) const PAGE: usize = 6;

#[derive(Default)]
pub(super) struct Access {
    pub unlocked: bool,
    pin: String,
    new_pin: String,
    confirmation: String,
    cashier_login: String,
    cashier_password: String,
}

impl Access {
    pub fn clear_fields(&mut self) {
        self.pin.clear();
        self.new_pin.clear();
        self.confirmation.clear();
        self.cashier_login.clear();
        self.cashier_password.clear();
    }
}

pub(super) fn show(app: &mut ViSales, ui: &mut Ui) {
    if app.access.unlocked {
        dashboard(app, ui);
    }
}

fn dashboard(app: &mut ViSales, ui: &mut Ui) {
    let completed = app.sales.iter().filter(|sale| sale.returned.is_none());
    let total: i64 = completed.map(|sale| sale.total).sum();
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("Товарів: {}", app.products.len()));
        ui.label(format!("Тестовий виторг: {}", money(total)));
    });
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        for index in 1..PAGE {
            if ui.button(PAGES[index]).clicked() {
                app.navigate(index);
            }
        }
    });
    ui.separator();
    cashier_accounts(app, ui);
    ui.separator();
    ui.collapsing("Змінити PIN адміністратора", |ui| {
        pin_input(ui, &mut app.access.pin, "Поточний PIN");
        pin_input(ui, &mut app.access.new_pin, "Новий PIN");
        pin_input(ui, &mut app.access.confirmation, "Повторіть новий PIN");
        if ui.button("Зберегти новий PIN").clicked() {
            let result = change_pin(app);
            app.access.pin.clear();
            app.access.new_pin.clear();
            app.access.confirmation.clear();
            app.result(result);
        }
    });
    if ui.button("Вийти з облікового запису").clicked() {
        app.lock_screen();
    }
}

fn cashier_accounts(app: &mut ViSales, ui: &mut Ui) {
    ui.heading("Касири");
    ui.label("Логін касира");
    ui.add(TextEdit::singleline(&mut app.access.cashier_login).char_limit(48));
    ui.label("Пароль касира");
    ui.add(
        TextEdit::singleline(&mut app.access.cashier_password)
            .password(true)
            .char_limit(128),
    );
    if ui.button("Додати касира").clicked() {
        let result = (|| {
            ensure!(app.access.unlocked, "Потрібні права адміністратора");
            let store = app
                .store
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Сховище недоступне"))?;
            store.create_cashier(&app.access.cashier_login, &app.access.cashier_password)?;
            Ok(format!("Касира {} додано", app.access.cashier_login.trim()))
        })();
        app.access.cashier_password.clear();
        if result.is_ok() {
            app.access.cashier_login.clear();
        }
        app.result(result);
    }

    match app.store().and_then(|store| store.cashier_logins()) {
        Ok(logins) if logins.is_empty() => {
            ui.weak("Касирів ще немає.");
        }
        Ok(logins) => {
            for login in logins {
                ui.label(login);
            }
        }
        Err(error) => {
            ui.label(error.to_string());
        }
    }
}

fn change_pin(app: &mut ViSales) -> Result<String> {
    ensure!(
        app.access.new_pin == app.access.confirmation,
        "PIN не збігаються"
    );
    let store = app
        .store
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("Сховище недоступне"))?;
    store.change_admin_pin(&app.access.pin, &app.access.new_pin)?;
    Ok("PIN оновлено".into())
}

fn pin_input(ui: &mut Ui, value: &mut String, hint: &str) {
    ui.label(hint);
    ui.add(TextEdit::singleline(value).password(true).char_limit(12));
}
