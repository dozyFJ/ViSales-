use eframe::egui::{self, Color32, Frame, Margin, Stroke};

pub(super) const SIGNAL: Color32 = Color32::from_rgb(255, 215, 54);
pub(super) const BLACK: Color32 = Color32::from_rgb(19, 20, 20);
pub(super) const LILAC: Color32 = Color32::from_rgb(207, 183, 246);

pub(super) struct Palette {
    pub grid: Color32,
    pub rail: Color32,
    pub line: Color32,
    pub ink: Color32,
    pub action: Color32,
}

impl Palette {
    pub(super) fn new(dark: bool) -> Self {
        if dark {
            Self {
                grid: Color32::from_rgb(27, 24, 30),
                rail: Color32::from_rgb(42, 36, 47),
                line: Color32::from_rgb(203, 190, 215),
                ink: Color32::from_rgb(244, 241, 229),
                action: SIGNAL,
            }
        } else {
            Self {
                grid: Color32::from_rgb(243, 229, 215),
                rail: Color32::from_rgb(255, 247, 232),
                line: BLACK,
                ink: BLACK,
                action: SIGNAL,
            }
        }
    }

    pub(super) fn panel(&self, horizontal: i8, vertical: i8) -> Frame {
        Frame::new()
            .fill(self.rail)
            .stroke(Stroke::new(2.0_f32, self.line))
            .inner_margin(Margin::symmetric(horizontal, vertical))
    }
}

pub(super) fn hard_shadow() -> egui::Shadow {
    egui::Shadow {
        offset: [4, 4],
        blur: 0,
        spread: 0,
        color: BLACK,
    }
}

pub(super) fn visuals(dark: bool) -> egui::Visuals {
    let p = Palette::new(dark);
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = p.grid;
    v.window_fill = p.rail;
    v.extreme_bg_color = if dark { BLACK } else { Color32::WHITE };
    v.override_text_color = Some(p.ink);
    v.hyperlink_color = if dark {
        SIGNAL
    } else {
        Color32::from_rgb(85, 60, 0)
    };
    v.error_fg_color = if dark {
        Color32::from_rgb(255, 120, 98)
    } else {
        Color32::from_rgb(140, 29, 17)
    };
    v.selection.bg_fill = if dark {
        Color32::from_rgb(104, 85, 27)
    } else {
        SIGNAL
    };
    v.selection.stroke = Stroke::new(2.0_f32, p.ink);

    for widget in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::ZERO;
        widget.bg_fill = p.rail;
        widget.weak_bg_fill = p.rail;
        widget.bg_stroke = Stroke::new(2.0_f32, p.line);
        widget.fg_stroke.color = p.ink;
    }
    v.widgets.hovered.weak_bg_fill = p.rail.lerp_to_gamma(p.action, 0.18);
    v.widgets.active.weak_bg_fill = p.rail.lerp_to_gamma(p.action, 0.25);
    v.window_stroke = Stroke::new(2.0_f32, p.line);
    v.window_corner_radius = egui::CornerRadius::ZERO;
    v.menu_corner_radius = egui::CornerRadius::ZERO;
    v.window_shadow = hard_shadow();
    v.popup_shadow = hard_shadow();
    v
}
