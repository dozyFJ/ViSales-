use super::ViSales;
use crate::store::{Payment, PaymentBreakdown, Tender, money, parse_money};
use anyhow::Result;
use eframe::egui::{self, Button, TextEdit};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Cash,
    Card,
    Split,
}

#[derive(Default)]
pub(super) struct Form {
    customer: Option<i64>,
    mode: Mode,
    cash: String,
    bonuses: String,
}

impl Form {
    pub(super) fn new(customer: Option<i64>) -> Self {
        Self {
            customer,
            ..Self::default()
        }
    }

    fn payment(&self) -> Result<Payment> {
        let redeemed = if self.bonuses.trim().is_empty() {
            0
        } else {
            parse_money(&self.bonuses)?
        };
        let tender = match self.mode {
            Mode::Cash => Tender::Cash,
            Mode::Card => Tender::Card,
            Mode::Split => Tender::Split {
                cash: parse_money(&self.cash)?,
            },
        };
        Ok(Payment { tender, redeemed })
    }
}

pub(super) fn show(app: &mut ViSales, ctx: &egui::Context) {
    let Some(mut form) = app.payment_form.take() else {
        return;
    };
    if app.cart.is_empty() {
        return;
    }
    let mut action = None;
    let mut cancel = false;
    let mut park = false;
    let response = egui::Modal::new(egui::Id::new("pos-payment")).show(ctx, |ui| {
        ui.set_width((ctx.screen_rect().width() - 40.).clamp(180., 360.));
        let height = (ctx.screen_rect().height() - 36.).clamp(64., 420.);
        egui::ScrollArea::vertical()
            .max_height(height)
            .min_scrolled_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading("Тестовий розрахунок");
                ui.label("Локальний запис · без списання коштів");
                let gross = app.cart_total();
                ui.label(format!("Сума кошика: {}", money(gross)));
                let previous_customer = form.customer;
                let customer_label = app
                    .customers
                    .iter()
                    .find(|(id, _)| Some(*id) == form.customer)
                    .map_or("Без клієнта", |(_, name)| name.as_str());
                ui.label("Клієнт для чека");
                egui::ComboBox::from_id_salt("payment-customer")
                    .selected_text(customer_label)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut form.customer, None, "Без клієнта");
                        for (id, name) in &app.customers {
                            ui.selectable_value(
                                &mut form.customer,
                                Some(*id),
                                format!("{name} · №{id}"),
                            );
                        }
                    });
                if previous_customer != form.customer {
                    form.bonuses.clear();
                }
                let balance = if let Some(customer) = form.customer {
                    match app.store().and_then(|store| store.customer_bonus(customer)) {
                        Ok(balance) => {
                            ui.label(format!("Бонусний баланс: {}", money(balance)));
                            ui.add(
                                TextEdit::singleline(&mut form.bonuses)
                                    .hint_text("Списати бонусів, ₴")
                                    .char_limit(14),
                            );
                            balance
                        }
                        Err(error) => {
                            ui.label(error.to_string());
                            cancel = ui.button("Закрити").clicked();
                            return;
                        }
                    }
                } else {
                    0
                };
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut form.mode, Mode::Cash, "Готівка");
                    ui.selectable_value(&mut form.mode, Mode::Card, "Картка");
                    ui.selectable_value(&mut form.mode, Mode::Split, "Змішана");
                });
                if form.mode == Mode::Split {
                    ui.add(
                        TextEdit::singleline(&mut form.cash)
                            .hint_text("Частина готівкою, ₴")
                            .char_limit(14),
                    );
                }
                let result = form.payment().and_then(|payment| {
                    payment
                        .quote(gross, balance, form.customer.is_some())
                        .map(|quote| (payment, quote))
                });
                match &result {
                    Ok((_, quote)) => {
                        ui.strong(format!("До сплати: {}", money(quote.cash + quote.card)));
                        if quote.cash > 0 {
                            ui.label(format!("Готівка: {}", money(quote.cash)));
                        }
                        if quote.card > 0 {
                            ui.label(format!("Картка: {}", money(quote.card)));
                        }
                        if quote.earned > 0 {
                            ui.label(format!("Нарахується бонусів: {}", money(quote.earned)));
                        }
                    }
                    Err(error) => {
                        ui.weak(error.to_string());
                    }
                }
                if let Some((false, notice)) = &app.notice {
                    ui.colored_label(ui.visuals().error_fg_color, notice);
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(result.is_ok(), Button::new("Записати тестовий продаж"))
                        .clicked()
                    {
                        action = result.ok().map(|(payment, _)| payment);
                    }
                    park = ui.button("Відкласти чек").clicked();
                    cancel = ui.button("Скасувати").clicked();
                });
            });
    });
    if cancel || response.should_close() {
        return;
    }
    if park {
        super::sale_details::park_current(app, form.customer);
    } else if let Some(payment) = action {
        app.checkout(payment, form.customer);
    }
    if !app.cart.is_empty() {
        app.payment_form = Some(form);
    }
}

pub(super) fn label(payment: &PaymentBreakdown) -> String {
    let mut parts = Vec::with_capacity(3);
    if payment.cash > 0 {
        parts.push(format!("Гот. {}", money(payment.cash)));
    }
    if payment.card > 0 {
        parts.push(format!("Карт. {}", money(payment.card)));
    }
    if payment.redeemed > 0 {
        parts.push(format!("Бонуси {}", money(payment.redeemed)));
    }
    if parts.is_empty() {
        return "0,00 ₴".into();
    }
    parts.join(" + ")
}
