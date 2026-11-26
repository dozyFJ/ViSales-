use super::{ViSales, admin};
use anyhow::{Context, Result, ensure};
use eframe::egui::{self, Button, Color32, Frame, Label, RichText, TextEdit};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Role {
    #[default]
    Cashier,
    Admin,
}

#[derive(Default)]
pub(super) struct Login {
    pub authenticated: bool,
    role: Role,
    login: String,
    password: String,
    confirmation: String,
    error: Option<String>,
    retry_at: f64,
}

pub(super) fn show(app: &mut ViSales, ctx: &egui::Context) {
    let store_status = app
        .store
        .as_ref()
        .context("Локальне сховище недоступне")
        .and_then(|store| Ok((store.has_cashier_account()?, store.has_admin_pin()?)));
    let mut submit = false;
    let now = ctx.input(|input| input.time);
    egui::CentralPanel::default().show(ctx, |ui| {
        let width = (ui.available_width() - 20.).clamp(180., 390.);
        ui.vertical_centered(|ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space((ui.available_height() - 440.).max(0.) * 0.28 + 12.);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("ViSales").size(42.).strong());
                        ui.add_space(12.);
                        Frame::new()
                            .fill(ui.visuals().window_fill)
                            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                            .inner_margin(16)
                            .show(ui, |ui| {
                                ui.set_width((width - 32.).max(140.));
                                let (cashier_ready, admin_ready) = match &store_status {
                                    Ok(status) => *status,
                                    Err(error) => {
                                        ui.colored_label(
                                            ui.visuals().error_fg_color,
                                            error.to_string(),
                                        );
                                        return;
                                    }
                                };
                                let setup = !admin_ready;
                                ui.heading(if setup {
                                    "Налаштування адміністратора"
                                } else {
                                    "Вхід"
                                });
                                let previous = app.login.role;
                                if setup || !cashier_ready {
                                    app.login.role = Role::Admin;
                                    ui.strong("Адміністратор");
                                } else {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.selectable_value(
                                            &mut app.login.role,
                                            Role::Cashier,
                                            "Касир",
                                        );
                                        ui.selectable_value(
                                            &mut app.login.role,
                                            Role::Admin,
                                            "Адміністратор",
                                        );
                                    });
                                }
                                if previous != app.login.role {
                                    app.login.login.clear();
                                    app.login.password.clear();
                                    app.login.confirmation.clear();
                                    app.login.error = None;
                                }
                                ui.label("Логін");
                                if app.login.role == Role::Admin {
                                    ui.strong("admin");
                                    if setup {
                                        ui.label("Задайте PIN адміністратора з 6–12 цифр.");
                                    }
                                } else {
                                    ui.add(
                                        TextEdit::singleline(&mut app.login.login)
                                            .desired_width(f32::INFINITY)
                                            .char_limit(48),
                                    );
                                }
                                ui.label(if app.login.role == Role::Admin {
                                    "PIN"
                                } else {
                                    "Пароль"
                                });
                                let password = ui.add(
                                    TextEdit::singleline(&mut app.login.password)
                                        .password(true)
                                        .desired_width(f32::INFINITY)
                                        .char_limit(128),
                                );
                                let mut enter = password.lost_focus()
                                    && ui.input(|input| input.key_pressed(egui::Key::Enter));
                                if setup {
                                    ui.label("Повторіть PIN");
                                    let confirmation = ui.add(
                                        TextEdit::singleline(&mut app.login.confirmation)
                                            .password(true)
                                            .desired_width(f32::INFINITY)
                                            .char_limit(128),
                                    );
                                    enter = confirmation.lost_focus()
                                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                                }
                                if let Some(error) = &app.login.error {
                                    ui.add(
                                        Label::new(
                                            RichText::new(error).color(ui.visuals().error_fg_color),
                                        )
                                        .wrap(),
                                    );
                                }
                                let waiting = now < app.login.retry_at;
                                if waiting {
                                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                                        app.login.retry_at - now,
                                    ));
                                }
                                let enabled = !waiting
                                    && !app.login.password.is_empty()
                                    && (app.login.role == Role::Admin
                                        || !app.login.login.trim().is_empty());
                                let title = if setup {
                                    "Створити й увійти"
                                } else {
                                    "Увійти"
                                };
                                let button = Button::new(
                                    RichText::new(title).strong().color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(36, 79, 142))
                                .min_size(egui::vec2(ui.available_width(), 42.));
                                submit =
                                    ui.add_enabled(enabled, button).clicked() || (enabled && enter);
                            });
                    });
                });
        });
    });

    if submit {
        let result = authenticate(app);
        app.login.password.clear();
        app.login.confirmation.clear();
        match result {
            Ok(()) => {
                app.access = admin::Access::default();
                app.access.unlocked = app.login.role == Role::Admin;
                app.login.authenticated = true;
                app.login.error = None;
                app.navigate(if app.access.unlocked { admin::PAGE } else { 0 });
                ctx.request_repaint();
            }
            Err(error) => {
                app.login.error = Some(error.to_string());
                app.login.retry_at = now + 2.;
            }
        }
    }
}

fn authenticate(app: &ViSales) -> Result<()> {
    let store = app.store.as_ref().context("Локальне сховище недоступне")?;
    let form = &app.login;
    if !store.has_admin_pin()? {
        ensure!(
            form.role == Role::Admin,
            "Спочатку налаштуйте адміністратора"
        );
        ensure!(form.password == form.confirmation, "PIN не збігаються");
        return store.set_initial_admin_pin(&form.password);
    }
    let valid = match form.role {
        Role::Admin => store.verify_admin_pin(&form.password)?,
        Role::Cashier => store.verify_cashier(&form.login, &form.password)?,
    };
    ensure!(valid, "Неправильний логін або пароль");
    Ok(())
}
