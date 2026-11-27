use eframe::egui::{self, Color32, Frame, Margin, Stroke};

pub(super) const SIGNAL: Color32 = Color32::from_rgb(255, 215, 54);
pub(super) const BLACK: Color32 = Color32::from_rgb(19, 20, 20);
pub(super) const LILAC: Color32 = Color32::from_rgb(207, 183, 246);
pub(super) const MINT: Color32 = Color32::from_rgb(176, 235, 199);
pub(super) const PEACH: Color32 = Color32::from_rgb(255, 193, 166);
pub(super) const SKY: Color32 = Color32::from_rgb(174, 218, 255);

pub(super) struct Palette {
    pub grid: Color32,
    pub rail: Color32,
    pub line: Color32,
    pub ink: Color32,
    pub action: Color32,
    pub muted: Color32,
    pub dark: bool,
}

impl Palette {
    pub(super) fn new(dark: bool) -> Self {
        if dark {
            Self {
                grid: Color32::from_rgb(17, 20, 26),
                rail: Color32::from_rgb(29, 34, 43),
                line: Color32::from_rgb(103, 115, 135),
                ink: Color32::from_rgb(239, 242, 248),
                action: Color32::from_rgb(242, 202, 104),
                muted: Color32::from_rgb(170, 182, 200),
                dark,
            }
        } else {
            Self {
                grid: Color32::from_rgb(243, 229, 215),
                rail: Color32::from_rgb(255, 247, 232),
                line: BLACK,
                ink: BLACK,
                action: SIGNAL,
                muted: Color32::from_rgb(91, 77, 66),
                dark,
            }
        }
    }

    pub(super) fn navigation(&self, accent: Color32, active: bool) -> (Color32, Color32, Color32) {
        if self.dark {
            if active {
                (
                    self.rail.lerp_to_gamma(self.action, 0.15),
                    self.action,
                    self.action,
                )
            } else {
                (self.rail, self.ink, self.line)
            }
        } else if active {
            (self.action, BLACK, BLACK)
        } else {
            (accent, self.ink, self.line)
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
    v.override_text_color = None;
    v.faint_bg_color = if dark {
        Color32::from_rgb(38, 44, 55)
    } else {
        v.faint_bg_color
    };
    v.hyperlink_color = if dark {
        p.action
    } else {
        Color32::from_rgb(85, 60, 0)
    };
    v.error_fg_color = if dark {
        Color32::from_rgb(255, 120, 98)
    } else {
        Color32::from_rgb(140, 29, 17)
    };
    v.selection.bg_fill = if dark {
        Color32::from_rgb(72, 61, 38)
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
    v.widgets.hovered.weak_bg_fill = p
        .rail
        .lerp_to_gamma(if dark { p.ink } else { p.action }, 0.12);
    v.widgets.active.weak_bg_fill = p
        .rail
        .lerp_to_gamma(if dark { p.ink } else { p.action }, 0.18);
    v.widgets.active.bg_stroke.color = if dark { p.action } else { p.line };
    v.window_stroke = Stroke::new(2.0_f32, p.line);
    v.window_corner_radius = egui::CornerRadius::ZERO;
    v.menu_corner_radius = egui::CornerRadius::ZERO;
    v.window_shadow = hard_shadow();
    v.popup_shadow = hard_shadow();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Color32) -> f64 {
        let linear = |channel: u8| {
            let srgb = f64::from(channel) / 255.;
            if srgb <= 0.04045 {
                srgb / 12.92
            } else {
                ((srgb + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }

    #[test]
    fn dark_navigation_and_labels_keep_readable_contrast() {
        let palette = Palette::new(true);
        let contrast = |a, b| {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        };
        for active in [false, true] {
            let (surface, ink, _) = palette.navigation(LILAC, active);
            assert!(contrast(ink, surface) >= 4.5);
            assert_ne!(surface, LILAC);
        }
        assert!(contrast(palette.muted, palette.rail) >= 4.5);
        assert!(contrast(BLACK, palette.action) >= 4.5);
    }
}
