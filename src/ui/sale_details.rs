use super::{ViSales, money, portrait_pos::Search};
use anyhow::{Context, ensure};
use eframe::egui::{self, Ui};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Panel {
    Customers,
    Parked,
}

pub(super) fn show(app: &mut ViSales, ui: &mut Ui, panel: Panel) {
    match panel {
        Panel::Customers => customers(app, ui),
        Panel::Parked => parked(app, ui),
    }
}

fn customer_name(app: &ViSales, id: Option<i64>) -> &str {
    id.and_then(|id| app.customers.iter().find(|(candidate, _)| *candidate == id))
        .map_or("Без клієнта", |(_, name)| name.as_str())
}

pub(super) fn cart_actions(app: &mut ViSales, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        if let Some(id) = app.customer {
            ui.label(format!("Клієнт: {}", customer_name(app, Some(id))));
        }
        if ui
            .add_enabled(!app.cart.is_empty(), egui::Button::new("Відкласти кошик"))
            .clicked()
        {
            let result = (|| {
                let id = app
                    .store
                    .as_mut()
                    .context("Сховище недоступне")?
                    .park_cart(&app.cart, app.customer, app.parked_cart)?;
                app.cart.clear();
                app.customer = None;
                app.parked_cart = None;
                app.portrait_search = Search::Browse;
                app.search.clear();
                app.pos_tab = 0;
                Ok(format!("Кошик №{id} відкладено"))
            })();
            app.result(result);
        }
    });
}

fn customers(app: &mut ViSales, ui: &mut Ui) {
    ui.heading("Клієнти");
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.customer_name)
                .desired_width(125.)
                .hint_text("Ім’я клієнта")
                .char_limit(100),
        );
        if ui
            .add_enabled(
                !app.customer_name.trim().is_empty(),
                egui::Button::new("Створити"),
            )
            .clicked()
        {
            let result = (|| {
                let store = app.store.as_ref().context("Сховище недоступне")?;
                let id = store.create_customer(&app.customer_name)?;
                app.customer = Some(id);
                app.customer_name.clear();
                app.customers = store.customers()?;
                Ok("Клієнта створено й вибрано".into())
            })();
            app.result(result);
        }
    });
    if ui
        .selectable_label(app.customer.is_none(), "Без клієнта")
        .clicked()
    {
        app.customer = None;
        return_to_cart(app);
    }
    let query = app.customer_name.trim().to_lowercase();
    let mut selected = None;
    egui::ScrollArea::vertical()
        .id_salt("pos-customers")
        .show(ui, |ui| {
            for (id, name) in &app.customers {
                if (query.is_empty() || name.to_lowercase().contains(&query))
                    && ui
                        .selectable_label(app.customer == Some(*id), format!("{name} · №{id}"))
                        .clicked()
                {
                    selected = Some(*id);
                }
            }
        });
    if let Some(id) = selected {
        app.customer = Some(id);
        app.customer_name.clear();
        return_to_cart(app);
    }
}

fn return_to_cart(app: &mut ViSales) {
    app.portrait_search = Search::Cart;
    app.desktop_sale_panel = None;
    app.pos_tab = 1;
}

fn parked(app: &mut ViSales, ui: &mut Ui) {
    ui.heading("Відкладені кошики");
    let can_resume = app.cart.is_empty();
    if !can_resume {
        ui.weak("Спочатку відкладіть або завершіть поточний кошик.");
        cart_actions(app, ui);
    }
    let carts = match app.store().and_then(|store| store.parked_carts()) {
        Ok(carts) => carts,
        Err(error) => {
            ui.label(error.to_string());
            return;
        }
    };
    if carts.is_empty() {
        ui.weak("Відкладених кошиків ще немає.");
    }
    let mut resume = None;
    egui::ScrollArea::vertical()
        .id_salt("parked-carts")
        .show(ui, |ui| {
            for cart in carts {
                ui.group(|ui| {
                    ui.strong(format!(
                        "#{} · {} · {} шт.",
                        cart.id,
                        money(cart.total),
                        cart.units
                    ));
                    ui.label(format!(
                        "{} · {} UTC",
                        customer_name(app, cart.customer),
                        cart.saved_at
                    ));
                    if ui
                        .add_enabled(can_resume, egui::Button::new("Відновити"))
                        .clicked()
                    {
                        resume = Some(cart.id);
                    }
                });
            }
        });
    if let Some(id) = resume {
        let result = (|| {
            ensure!(app.cart.is_empty(), "Поточний кошик ще не порожній");
            let (parked, customer, cart) = app.store()?.load_parked_cart(id)?;
            app.cart = cart;
            app.customer = customer;
            app.parked_cart = Some(parked);
            return_to_cart(app);
            Ok(format!("Кошик №{id} відновлено"))
        })();
        app.result(result);
    }
}
