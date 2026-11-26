use super::{PAGES, ViSales, money};
use anyhow::{Result, ensure};
use eframe::egui::{Button, TextEdit, Ui};

pub(super) const PAGE: usize = 6;

#[derive(Default)]
pub(super) struct Access {
    pub unlocked: bool,
    pin: String,
    new_pin: String,
    confirmation: String,
    retry_at: f64,
}

impl Access {
    pub fn clear_fields(&mut self) {
        self.pin.clear();
        self.new_pin.clear();
        self.confirmation.clear();
    }
}

pub(super) fn show(app: &mut ViSales, ui: &mut Ui) {
    if app.access.unlocked {
        dashboard(app, ui);
        return;
    }

    let configured = match app.store().and_then(|store| store.has_admin_pin()) {
        Ok(configured) => configured,
        Err(error) => {
            ui.label(error.to_string());
            return;
        }
    };
    ui.heading(if configured {
        "Вхід адміністратора"
    } else {
        "Створення PIN адміністратора"
    });
    if !configured {
        ui.label("Під час першого налаштування задайте PIN із 6–12 цифр.");
    }
    pin_input(ui, &mut app.access.pin, "PIN адміністратора");
    if !configured {
        pin_input(ui, &mut app.access.confirmation, "Повторіть PIN");
    }
    let now = ui.input(|input| input.time);
    let waiting = now < app.access.retry_at;
    if waiting {
        ui.weak("Зачекайте перед наступною спробою.");
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(
                app.access.retry_at - now,
            ));
    }
    if ui
        .add_enabled(
            !waiting,
            Button::new(if configured {
                "Увійти"
            } else {
                "Зберегти й увійти"
            }),
        )
        .clicked()
    {
        let result = (|| {
            let store = app
                .store
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Сховище недоступне"))?;
            if configured {
                ensure!(store.verify_admin_pin(&app.access.pin)?, "Неправильний PIN");
            } else {
                ensure!(
                    app.access.pin == app.access.confirmation,
                    "PIN не збігаються"
                );
                store.set_initial_admin_pin(&app.access.pin)?;
            }
            Ok("Вхід адміністратора виконано".to_owned())
        })();
        app.access.pin.clear();
        app.access.confirmation.clear();
        if result.is_ok() {
            app.access.unlocked = true;
        } else if configured {
            app.access.retry_at = now + 3.;
        }
        app.result(result);
    }
    if ui.button("Повернутися до каси").clicked() {
        app.navigate(0);
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
    if ui.button("Вийти з режиму адміністратора").clicked() {
        app.access = Access::default();
        app.refund = None;
        app.navigate(0);
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
