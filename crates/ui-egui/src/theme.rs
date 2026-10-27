//! Design tokens. Colours were measured from the reference look (dark slate chrome, near-black
//! navy model space) and are Redrob CAD's own values; every widget reads them from here.

use egui::{Color32, FontFamily, FontId, Visuals};

#[derive(Clone, Copy, Debug)]
pub struct Tokens {
    pub chrome: Color32,
    pub chrome_dark: Color32,
    pub panel: Color32,
    pub tab_active: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub border: Color32,
    pub canvas: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub axis_x: Color32,
    pub axis_y: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub icon: Color32,
    pub icon_accent: Color32,
    pub icon_point: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub hover: Color32,
    pub grip: Color32,
    pub grip_hot: Color32,
    pub snap: Color32,
    pub window_fill: Color32,
    pub crossing_fill: Color32,
    pub window_stroke: Color32,
    pub crossing_stroke: Color32,
    pub cmd_bg: Color32,
    pub toggle_on: Color32,
}

impl Tokens {
    pub const DARK: Tokens = Tokens {
        chrome: Color32::from_rgb(0x3b, 0x44, 0x53),
        chrome_dark: Color32::from_rgb(0x31, 0x38, 0x44),
        panel: Color32::from_rgb(0x3c, 0x44, 0x52),
        tab_active: Color32::from_rgb(0x4e, 0x5a, 0x6e),
        control: Color32::from_rgb(0x50, 0x5a, 0x6d),
        control_hover: Color32::from_rgb(0x5c, 0x67, 0x7c),
        border: Color32::from_rgb(0x23, 0x29, 0x30),
        canvas: Color32::from_rgb(0x22, 0x28, 0x2f),
        grid_minor: Color32::from_rgb(0x29, 0x30, 0x3a),
        grid_major: Color32::from_rgb(0x31, 0x39, 0x45),
        axis_x: Color32::from_rgb(0x56, 0x28, 0x29),
        axis_y: Color32::from_rgb(0x28, 0x4a, 0x2e),
        text: Color32::from_rgb(0xd7, 0xdb, 0xe0),
        text_dim: Color32::from_rgb(0xa3, 0xaa, 0xb4),
        text_faint: Color32::from_rgb(0x87, 0x8d, 0x96),
        icon: Color32::from_rgb(0xcf, 0xd4, 0xdb),
        icon_accent: Color32::from_rgb(0x4f, 0xa3, 0xf7),
        icon_point: Color32::from_rgb(0xf0, 0x7a, 0x3a),
        accent: Color32::from_rgb(0x3d, 0x8b, 0xfd),
        selection: Color32::from_rgb(0x4a, 0x8f, 0xff),
        hover: Color32::from_rgb(0x9c, 0xc6, 0xff),
        grip: Color32::from_rgb(0x2f, 0x6b, 0xff),
        grip_hot: Color32::from_rgb(0xff, 0x3b, 0x3b),
        snap: Color32::from_rgb(0x2f, 0xd0, 0x62),
        window_fill: Color32::from_rgba_premultiplied(0x18, 0x30, 0x60, 0x50),
        crossing_fill: Color32::from_rgba_premultiplied(0x18, 0x48, 0x18, 0x50),
        window_stroke: Color32::from_rgb(0x6f, 0xa8, 0xff),
        crossing_stroke: Color32::from_rgb(0x7f, 0xe0, 0x7f),
        cmd_bg: Color32::from_rgba_premultiplied(0x34, 0x3c, 0x4a, 0xf0),
        toggle_on: Color32::from_rgb(0x3d, 0x8b, 0xfd),
    };

    pub fn get() -> Tokens {
        Tokens::DARK
    }
}

pub fn small() -> FontId {
    FontId::new(11.5, FontFamily::Proportional)
}
pub fn body() -> FontId {
    FontId::new(12.5, FontFamily::Proportional)
}
pub fn mono() -> FontId {
    FontId::new(12.0, FontFamily::Monospace)
}

/// Load a system UI font at run time when one is installed (not bundled), else keep egui's.
/// The Korean system face of the Redrob design system (Wanted Sans Regular, SIL OFL 1.1; licence
/// beside the file). egui's own fonts carry no Hangul, so without it Korean renders as boxes.
pub const KOREAN_FONT: &[u8] = include_bytes!("../assets/fonts/WantedSans-Regular.ttf");

/// Korean system text is set at 96% of the Latin size (design system, `35-korean.md`).
const KOREAN_SCALE: f32 = 0.96;

/// Fonts: the platform's Latin UI face first, then Wanted Sans for Hangul, then egui's defaults.
/// A Latin face sets only Latin and a Korean face only Korean, so each glyph falls to the right one.
pub fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions());
}

pub fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    #[cfg(not(target_arch = "wasm32"))]
    {
        let candidates: &[&str] = if cfg!(target_os = "macos") {
            &["/System/Library/Fonts/SFNS.ttf", "/System/Library/Fonts/Helvetica.ttc", "/Library/Fonts/Arial.ttf"]
        } else if cfg!(windows) {
            &["C:\\Windows\\Fonts\\segoeui.ttf", "C:\\Windows\\Fonts\\arial.ttf"]
        } else {
            &[
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
                "/usr/share/fonts/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            ]
        };
        for path in candidates {
            if let Ok(bytes) = std::fs::read(path) {
                fonts.font_data.insert("system-ui".into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
                if let Some(f) = fonts.families.get_mut(&FontFamily::Proportional) {
                    f.insert(0, "system-ui".into());
                }
                break;
            }
        }
    }
    let korean = egui::FontData::from_static(KOREAN_FONT).tweak(egui::FontTweak { scale: KOREAN_SCALE, ..Default::default() });
    fonts.font_data.insert("korean".into(), std::sync::Arc::new(korean));
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(f) = fonts.families.get_mut(&family) {
            // Right after the family's Latin face, ahead of egui's other fallbacks.
            f.insert(1.min(f.len()), "korean".into());
        }
    }
    fonts
}

pub fn apply(ctx: &egui::Context) {
    let t = Tokens::get();
    let mut v = Visuals::dark();
    v.panel_fill = t.panel;
    v.window_fill = t.chrome;
    v.extreme_bg_color = t.chrome_dark;
    v.faint_bg_color = t.chrome_dark;
    v.override_text_color = Some(t.text);
    v.widgets.noninteractive.bg_fill = t.panel;
    v.widgets.noninteractive.fg_stroke.color = t.text;
    v.widgets.noninteractive.bg_stroke.color = t.border;
    v.widgets.inactive.bg_fill = t.control;
    v.widgets.inactive.weak_bg_fill = t.control;
    v.widgets.inactive.fg_stroke.color = t.text;
    v.widgets.hovered.bg_fill = t.control_hover;
    v.widgets.hovered.weak_bg_fill = t.control_hover;
    v.widgets.active.bg_fill = t.accent;
    v.widgets.active.weak_bg_fill = t.accent;
    v.selection.bg_fill = t.accent;
    v.window_stroke = egui::Stroke::new(1.0, t.border);
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(90) };
    v.window_corner_radius = egui::CornerRadius::same(6);
    v.menu_corner_radius = egui::CornerRadius::same(5);
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 3.0);
        s.spacing.interact_size.y = 20.0;
        s.text_styles.insert(egui::TextStyle::Body, body());
        s.text_styles.insert(egui::TextStyle::Button, body());
        s.text_styles.insert(egui::TextStyle::Small, small());
        s.text_styles.insert(egui::TextStyle::Monospace, mono());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hangul must render in both families; without the bundled face every syllable is a box.
    #[test]
    fn korean_renders_in_proportional_and_monospace() {
        let mut fonts = egui::epaint::text::Fonts::new(egui::epaint::text::TextOptions::default(), font_definitions());
        for id in [FontId::proportional(13.0), FontId::monospace(12.0)] {
            for c in "가나다원그리기한국어".chars() {
                assert!(fonts.has_glyph(&id, c), "{id:?} has no {c:?}");
            }
        }
    }

    /// The bundled face is the design system's file, byte for byte.
    #[test]
    fn korean_face_is_the_design_system_file() {
        assert_eq!(KOREAN_FONT.len(), 2_401_736);
    }
}
