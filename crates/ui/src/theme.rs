use std::sync::Arc;
use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppTheme {
    #[default]
    Modern,
    Dark,
    Light,
}

impl AppTheme {
    pub const ALL: [AppTheme; 3] = [AppTheme::Modern, AppTheme::Dark, AppTheme::Light];

    pub fn label(&self) -> &'static str {
        match self {
            AppTheme::Modern => "Modern",
            AppTheme::Dark => "Dark",
            AppTheme::Light => "Light",
        }
    }

    pub fn is_light(&self) -> bool {
        matches!(self, AppTheme::Light)
    }

    pub fn apply_visuals(&self, ctx: &egui::Context) {
        match self {
            AppTheme::Modern => {
                let mut visuals = Visuals::dark();
                // Soft warm silver-gray text to reduce eye strain
                let text_col = Color32::from_rgb(214, 218, 226);
                visuals.override_text_color = None;
                visuals.weak_text_alpha = 0.68;
                visuals.panel_fill = Color32::from_rgb(33, 35, 39);
                visuals.window_fill = Color32::from_rgb(36, 38, 42);
                visuals.extreme_bg_color = Color32::from_rgb(22, 23, 26);
                visuals.faint_bg_color = Color32::from_rgb(40, 42, 47);

                visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(36, 38, 43);
                visuals.widgets.noninteractive.bg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(58, 61, 68));
                visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text_col);
                visuals.widgets.noninteractive.corner_radius = CornerRadius::same(5);

                visuals.widgets.inactive.bg_fill = Color32::from_rgb(43, 45, 51);
                visuals.widgets.inactive.bg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(66, 70, 78));
                visuals.widgets.inactive.fg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(208, 212, 220));
                visuals.widgets.inactive.corner_radius = CornerRadius::same(5);

                visuals.widgets.hovered.bg_fill = Color32::from_rgb(54, 57, 65);
                visuals.widgets.hovered.bg_stroke =
                    Stroke::new(1.2, Color32::from_rgb(115, 160, 205));
                visuals.widgets.hovered.fg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(238, 241, 246));
                visuals.widgets.hovered.corner_radius = CornerRadius::same(5);

                visuals.widgets.active.bg_fill = Color32::from_rgb(62, 92, 128);
                visuals.widgets.active.bg_stroke =
                    Stroke::new(1.5, Color32::from_rgb(135, 180, 222));
                visuals.widgets.active.fg_stroke =
                    Stroke::new(1.2, Color32::from_rgb(245, 247, 250));
                visuals.widgets.active.corner_radius = CornerRadius::same(5);

                visuals.widgets.open.bg_fill = Color32::from_rgb(50, 54, 62);
                visuals.widgets.open.bg_stroke =
                    Stroke::new(1.2, Color32::from_rgb(115, 160, 205));
                visuals.widgets.open.fg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(238, 241, 246));
                visuals.widgets.open.corner_radius = CornerRadius::same(5);

                visuals.selection.bg_fill = Color32::from_rgb(58, 88, 122);
                visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(240, 245, 252));

                ctx.set_visuals(visuals);
            }
            AppTheme::Dark => {
                let mut visuals = Visuals::dark();
                let text_col = Color32::from_rgb(230, 232, 238);
                visuals.override_text_color = None;
                visuals.weak_text_alpha = 0.68;
                visuals.panel_fill = Color32::from_rgb(24, 24, 27);
                visuals.window_fill = Color32::from_rgb(28, 28, 32);
                visuals.extreme_bg_color = Color32::from_rgb(15, 15, 17);
                visuals.faint_bg_color = Color32::from_rgb(32, 32, 36);

                visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(28, 28, 32);
                visuals.widgets.noninteractive.bg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(52, 52, 58));
                visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text_col);
                visuals.widgets.noninteractive.corner_radius = CornerRadius::same(4);

                visuals.widgets.inactive.bg_fill = Color32::from_rgb(38, 38, 43);
                visuals.widgets.inactive.bg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(64, 64, 72));
                visuals.widgets.inactive.fg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(220, 222, 228));
                visuals.widgets.inactive.corner_radius = CornerRadius::same(4);

                visuals.widgets.hovered.bg_fill = Color32::from_rgb(54, 54, 62);
                visuals.widgets.hovered.bg_stroke =
                    Stroke::new(1.2, Color32::from_rgb(160, 160, 175));
                visuals.widgets.hovered.fg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(246, 247, 250));
                visuals.widgets.hovered.corner_radius = CornerRadius::same(4);

                visuals.widgets.active.bg_fill = Color32::from_rgb(70, 95, 135);
                visuals.widgets.active.bg_stroke =
                    Stroke::new(1.5, Color32::from_rgb(160, 195, 240));
                visuals.widgets.active.fg_stroke = Stroke::new(1.2, Color32::WHITE);
                visuals.widgets.active.corner_radius = CornerRadius::same(4);

                visuals.widgets.open.bg_fill = Color32::from_rgb(46, 46, 54);
                visuals.widgets.open.bg_stroke =
                    Stroke::new(1.2, Color32::from_rgb(140, 150, 170));
                visuals.widgets.open.fg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(246, 247, 250));
                visuals.widgets.open.corner_radius = CornerRadius::same(4);

                visuals.selection.bg_fill = Color32::from_rgb(65, 95, 145);
                visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);

                ctx.set_visuals(visuals);
            }
            AppTheme::Light => {
                let mut visuals = Visuals::light();
                let text_col = Color32::from_rgb(22, 28, 40);
                visuals.override_text_color = None;
                visuals.weak_text_alpha = 0.72;
                visuals.panel_fill = Color32::from_rgb(238, 242, 247);
                visuals.window_fill = Color32::from_rgb(248, 250, 253);
                visuals.extreme_bg_color = Color32::from_rgb(255, 255, 255);
                visuals.faint_bg_color = Color32::from_rgb(228, 234, 242);

                visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(244, 247, 251);
                visuals.widgets.noninteractive.bg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(194, 204, 218));
                visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text_col);
                visuals.widgets.noninteractive.corner_radius = CornerRadius::same(5);

                visuals.widgets.inactive.bg_fill = Color32::from_rgb(250, 252, 255);
                visuals.widgets.inactive.bg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(178, 190, 208));
                visuals.widgets.inactive.fg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(28, 36, 50));
                visuals.widgets.inactive.corner_radius = CornerRadius::same(5);

                visuals.widgets.hovered.bg_fill = Color32::from_rgb(222, 234, 252);
                visuals.widgets.hovered.bg_stroke =
                    Stroke::new(1.2, Color32::from_rgb(45, 115, 208));
                visuals.widgets.hovered.fg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(12, 22, 42));
                visuals.widgets.hovered.corner_radius = CornerRadius::same(5);

                visuals.widgets.active.bg_fill = Color32::from_rgb(30, 102, 198);
                visuals.widgets.active.bg_stroke =
                    Stroke::new(1.5, Color32::from_rgb(18, 75, 158));
                visuals.widgets.active.fg_stroke = Stroke::new(1.2, Color32::WHITE);
                visuals.widgets.active.corner_radius = CornerRadius::same(5);

                visuals.widgets.open.bg_fill = Color32::from_rgb(226, 236, 250);
                visuals.widgets.open.bg_stroke =
                    Stroke::new(1.2, Color32::from_rgb(45, 115, 208));
                visuals.widgets.open.fg_stroke =
                    Stroke::new(1.0, Color32::from_rgb(12, 22, 42));
                visuals.widgets.open.corner_radius = CornerRadius::same(5);

                visuals.selection.bg_fill = Color32::from_rgb(30, 102, 198);
                visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);

                ctx.set_visuals(visuals);
            }
        }
    }

    pub fn palette(&self) -> ThemePalette {
        match self {
            AppTheme::Modern => ThemePalette {
                canvas_bg: Color32::from_rgb(26, 27, 30),
                grid_dot: Color32::from_rgb(48, 50, 56),
                banner_bg: Color32::from_rgb(36, 38, 43),
                banner_border: Color32::from_rgb(66, 70, 78),
                banner_title: Color32::from_rgb(135, 180, 218),
                banner_sub: Color32::from_rgb(140, 192, 158),
                wire_idle: Color32::from_rgb(112, 118, 128),
                wire_energized: Color32::from_rgb(225, 105, 85),
                wire_glow: Color32::from_rgba_unmultiplied(225, 105, 85, 45),
                l1_rail: Color32::from_rgb(212, 142, 75),
                l2_rail: Color32::from_rgb(100, 155, 212),
                rung_badge_bg: Color32::from_rgb(36, 38, 43),
                rung_badge_sel_bg: Color32::from_rgb(62, 54, 38),
                rung_badge_hov_bg: Color32::from_rgb(50, 53, 60),
                rung_badge_border: Color32::from_rgb(62, 65, 73),
                rung_badge_sel_border: Color32::from_rgb(210, 165, 85),
                rung_text_dim: Color32::from_rgb(154, 160, 172),
                rung_text_num: Color32::from_rgb(206, 210, 218),
                rung_text_sel: Color32::from_rgb(235, 200, 120),
                symbol_idle: Color32::from_rgb(202, 207, 216),
                symbol_closed: Color32::from_rgb(105, 195, 135),
                symbol_subtext: Color32::from_rgb(162, 170, 182),
                coil_idle: Color32::from_rgb(118, 168, 215),
                card_bg: Color32::from_rgb(41, 43, 48),
                card_border: Color32::from_rgb(76, 80, 88),
                pill_bg: Color32::from_rgb(30, 32, 36),
                pill_active_bg: Color32::from_rgba_unmultiplied(48, 92, 62, 215),
                comment_bg: Color32::from_rgba_unmultiplied(42, 62, 50, 160),
                comment_fg: Color32::from_rgb(138, 195, 155),
                accent_heading: Color32::from_rgb(122, 172, 218),
                accent_warn: Color32::from_rgb(218, 172, 95),
                accent_ok: Color32::from_rgb(108, 188, 138),
                accent_danger: Color32::from_rgb(215, 115, 110),
                text_primary: Color32::from_rgb(214, 218, 226),
            },
            AppTheme::Dark => ThemePalette {
                canvas_bg: Color32::from_rgb(18, 18, 20),
                grid_dot: Color32::from_rgb(38, 38, 42),
                banner_bg: Color32::from_rgb(28, 28, 32),
                banner_border: Color32::from_rgb(60, 60, 68),
                banner_title: Color32::from_rgb(230, 230, 235),
                banner_sub: Color32::from_rgb(175, 180, 190),
                wire_idle: Color32::from_rgb(115, 118, 125),
                wire_energized: Color32::from_rgb(255, 70, 50),
                wire_glow: Color32::from_rgba_unmultiplied(255, 60, 40, 50),
                l1_rail: Color32::from_rgb(235, 135, 35),
                l2_rail: Color32::from_rgb(80, 150, 235),
                rung_badge_bg: Color32::from_rgb(26, 26, 30),
                rung_badge_sel_bg: Color32::from_rgb(55, 45, 24),
                rung_badge_hov_bg: Color32::from_rgb(38, 38, 44),
                rung_badge_border: Color32::from_rgb(58, 58, 66),
                rung_badge_sel_border: Color32::from_rgb(245, 175, 45),
                rung_text_dim: Color32::from_rgb(160, 162, 172),
                rung_text_num: Color32::from_rgb(224, 226, 232),
                rung_text_sel: Color32::from_rgb(255, 205, 65),
                symbol_idle: Color32::from_rgb(220, 220, 225),
                symbol_closed: Color32::from_rgb(60, 225, 105),
                symbol_subtext: Color32::from_rgb(170, 175, 186),
                coil_idle: Color32::from_rgb(120, 180, 245),
                card_bg: Color32::from_rgb(26, 26, 30),
                card_border: Color32::from_rgb(85, 88, 98),
                pill_bg: Color32::from_rgb(30, 30, 35),
                pill_active_bg: Color32::from_rgba_unmultiplied(40, 95, 55, 225),
                comment_bg: Color32::from_rgba_unmultiplied(35, 60, 42, 165),
                comment_fg: Color32::from_rgb(125, 225, 145),
                accent_heading: Color32::from_rgb(130, 190, 250),
                accent_warn: Color32::from_rgb(245, 190, 65),
                accent_ok: Color32::from_rgb(70, 225, 110),
                accent_danger: Color32::from_rgb(245, 95, 85),
                text_primary: Color32::from_rgb(230, 232, 238),
            },
            AppTheme::Light => ThemePalette {
                canvas_bg: Color32::from_rgb(247, 249, 252),
                grid_dot: Color32::from_rgb(210, 218, 230),
                banner_bg: Color32::from_rgb(235, 241, 248),
                banner_border: Color32::from_rgb(185, 198, 218),
                banner_title: Color32::from_rgb(18, 85, 165),
                banner_sub: Color32::from_rgb(25, 125, 65),
                wire_idle: Color32::from_rgb(90, 105, 128),
                wire_energized: Color32::from_rgb(220, 32, 20),
                wire_glow: Color32::from_rgba_unmultiplied(235, 45, 30, 42),
                l1_rail: Color32::from_rgb(215, 95, 10),
                l2_rail: Color32::from_rgb(25, 105, 205),
                rung_badge_bg: Color32::from_rgb(236, 241, 248),
                rung_badge_sel_bg: Color32::from_rgb(255, 240, 200),
                rung_badge_hov_bg: Color32::from_rgb(222, 232, 246),
                rung_badge_border: Color32::from_rgb(180, 194, 214),
                rung_badge_sel_border: Color32::from_rgb(225, 135, 15),
                rung_text_dim: Color32::from_rgb(78, 92, 115),
                rung_text_num: Color32::from_rgb(24, 34, 50),
                rung_text_sel: Color32::from_rgb(168, 82, 0),
                symbol_idle: Color32::from_rgb(26, 38, 56),
                symbol_closed: Color32::from_rgb(16, 145, 58),
                symbol_subtext: Color32::from_rgb(68, 82, 106),
                coil_idle: Color32::from_rgb(18, 94, 185),
                card_bg: Color32::from_rgb(255, 255, 255),
                card_border: Color32::from_rgb(135, 158, 190),
                pill_bg: Color32::from_rgb(236, 242, 250),
                pill_active_bg: Color32::from_rgba_unmultiplied(22, 150, 68, 225),
                comment_bg: Color32::from_rgba_unmultiplied(215, 242, 222, 220),
                comment_fg: Color32::from_rgb(16, 108, 44),
                accent_heading: Color32::from_rgb(16, 88, 178),
                accent_warn: Color32::from_rgb(180, 102, 0),
                accent_ok: Color32::from_rgb(16, 135, 55),
                accent_danger: Color32::from_rgb(198, 32, 26),
                text_primary: Color32::from_rgb(22, 28, 40),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub canvas_bg: Color32,
    pub grid_dot: Color32,
    pub banner_bg: Color32,
    pub banner_border: Color32,
    pub banner_title: Color32,
    pub banner_sub: Color32,
    pub wire_idle: Color32,
    pub wire_energized: Color32,
    pub wire_glow: Color32,
    pub l1_rail: Color32,
    pub l2_rail: Color32,
    pub rung_badge_bg: Color32,
    pub rung_badge_sel_bg: Color32,
    pub rung_badge_hov_bg: Color32,
    pub rung_badge_border: Color32,
    pub rung_badge_sel_border: Color32,
    pub rung_text_dim: Color32,
    pub rung_text_num: Color32,
    pub rung_text_sel: Color32,
    pub symbol_idle: Color32,
    pub symbol_closed: Color32,
    pub symbol_subtext: Color32,
    pub coil_idle: Color32,
    pub card_bg: Color32,
    pub card_border: Color32,
    pub pill_bg: Color32,
    pub pill_active_bg: Color32,
    pub comment_bg: Color32,
    pub comment_fg: Color32,
    pub accent_heading: Color32,
    pub accent_warn: Color32,
    pub accent_ok: Color32,
    pub accent_danger: Color32,
    pub text_primary: Color32,
}

impl ThemePalette {
    #[inline]
    pub fn is_light(&self) -> bool {
        self.canvas_bg.r() > 200
    }
}

/// Load system fonts (Noto Sans, Noto Sans Thai, DejaVu Sans, Noto Symbols) so Thai text
/// and technical symbols never render as missing-glyph squares (tofu).
pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let font_candidates: &[(&str, &str, bool)] = &[
        ("noto_sans", "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf", false),
        ("noto_thai", "/usr/share/fonts/truetype/noto/NotoSansThai-Regular.ttf", false),
        ("dejavu_sans", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", false),
        ("noto_sym", "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf", false),
        ("noto_sym2", "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf", false),
        ("dejavu_mono", "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", true),
        ("noto_mono", "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf", true),
    ];

    for &(name, path, is_mono) in font_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));

            if is_mono {
                if let Some(mono_list) = fonts.families.get_mut(&FontFamily::Monospace) {
                    mono_list.push(name.to_owned());
                }
            } else {
                if let Some(prop_list) = fonts.families.get_mut(&FontFamily::Proportional) {
                    prop_list.push(name.to_owned());
                }
                if let Some(mono_list) = fonts.families.get_mut(&FontFamily::Monospace) {
                    mono_list.push(name.to_owned());
                }
            }
        }
    }

    ctx.set_fonts(fonts);
}
