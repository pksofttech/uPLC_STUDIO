use crate::app::{CanvasApp, WorkMode};
use crate::icons::{paint_icon, VectorIcon};
use crate::theme::ThemePalette;
use crate::tools::ActiveTool;
use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};

fn lp_card(palette: ThemePalette) -> egui::Frame {
    egui::Frame::new()
        .fill(palette.card_bg)
        .stroke(Stroke::new(1.0, palette.card_border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(8, 6))
}

fn accent_color(accent_kind: u8, palette: ThemePalette) -> Color32 {
    match accent_kind {
        0 => palette.accent_ok,
        1 => palette.accent_warn,
        2 => palette.accent_heading,
        3 => palette.coil_idle,
        4 => palette.accent_danger,
        _ => palette.wire_idle,
    }
}

pub fn render(ui: &mut Ui, app: &mut CanvasApp) {
    let palette = app.theme.palette();
    let is_edit_mode = app.work_mode == WorkMode::Edit;

    // --- Mode-Locked Banner (Simulator / Online PLC Debug) ---
    if !is_edit_mode {
        let (mode_title, mode_col) = match app.work_mode {
            WorkMode::Simulator => ("Simulator Mode", palette.accent_ok),
            WorkMode::OnlineDebug => ("Online PLC Debug", palette.accent_warn),
            WorkMode::Edit => ("Edit Mode", palette.accent_heading),
        };
        egui::Frame::new()
            .fill(mode_col.gamma_multiply(0.14))
            .stroke(Stroke::new(1.0, mode_col))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(8, 5))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(mode_title).color(mode_col).strong().size(11.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Switch to Edit")
                                        .strong()
                                        .size(10.5)
                                        .color(Color32::WHITE),
                                )
                                .fill(palette.accent_heading)
                                .corner_radius(CornerRadius::same(4)),
                            )
                            .clicked()
                        {
                            app.set_work_mode(WorkMode::Edit);
                        }
                    });
                });
            });
        ui.add_space(5.0);
    }

    ui.add_enabled_ui(is_edit_mode, |ui| {
        // --- 1. Compact 1-Row Network Tools Card ---
        lp_card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());

            let total_rungs = app.doc.program.rungs.len();
            let cur_idx = app.doc.selected_rung_idx;
            let can_move_up = cur_idx.map_or(false, |i| i > 0);
            let can_move_dn = cur_idx.map_or(false, |i| i + 1 < total_rungs);
            let has_rung = cur_idx.is_some();

            // Header row: Network + Rung badge
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Network")
                        .size(11.5)
                        .strong()
                        .color(palette.accent_heading),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(idx) = cur_idx {
                        let badge_text = format!("#{}/{}", idx + 1, total_rungs);
                        let pill_bg = if palette.is_light() {
                            Color32::from_rgb(220, 235, 252)
                        } else {
                            Color32::from_rgb(32, 50, 75)
                        };
                        let pill_border = if palette.is_light() {
                            Color32::from_rgb(70, 130, 220)
                        } else {
                            Color32::from_rgb(80, 140, 230)
                        };
                        egui::Frame::new()
                            .fill(pill_bg)
                            .stroke(Stroke::new(1.0, pill_border))
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(5, 1))
                            .show(ui, |ui| {
                                ui.label(RichText::new(badge_text).size(10.0).strong().color(
                                    if palette.is_light() {
                                        Color32::from_rgb(20, 60, 140)
                                    } else {
                                        Color32::WHITE
                                    },
                                ));
                            });
                    } else {
                        ui.label(
                            RichText::new(format!("{} Rungs", total_rungs))
                                .size(10.0)
                                .color(palette.rung_text_dim),
                        );
                    }
                });
            });

            ui.add_space(4.0);

            let copy_accent = if palette.is_light() {
                Color32::from_rgb(115, 65, 185)
            } else {
                Color32::from_rgb(168, 148, 220)
            };

            let avail_w = ui.available_width();
            let btn_w = ((avail_w - 15.0) / 6.0).floor().max(22.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;

                if network_icon_btn(
                    ui,
                    VectorIcon::NewRung,
                    palette.accent_ok,
                    true,
                    btn_w,
                    palette,
                )
                .on_hover_text("New Rung")
                .clicked()
                {
                    app.doc.add_rung();
                }

                if network_icon_btn(
                    ui,
                    VectorIcon::MoveUp,
                    palette.accent_heading,
                    can_move_up,
                    btn_w,
                    palette,
                )
                .on_hover_text("Move Rung Up")
                .clicked()
                {
                    if let Some(idx) = cur_idx {
                        app.doc.move_rung_up(idx);
                    }
                }

                if network_icon_btn(
                    ui,
                    VectorIcon::MoveDown,
                    palette.accent_heading,
                    can_move_dn,
                    btn_w,
                    palette,
                )
                .on_hover_text("Move Rung Down")
                .clicked()
                {
                    if let Some(idx) = cur_idx {
                        app.doc.move_rung_down(idx);
                    }
                }

                if network_icon_btn(
                    ui,
                    VectorIcon::CopyDown,
                    copy_accent,
                    has_rung,
                    btn_w,
                    palette,
                )
                .on_hover_text("Duplicate Rung Below")
                .clicked()
                {
                    if cur_idx.is_some() {
                        app.paste_network_below();
                    }
                }

                if network_icon_btn(
                    ui,
                    VectorIcon::CopyUp,
                    copy_accent,
                    has_rung,
                    btn_w,
                    palette,
                )
                .on_hover_text("Duplicate Rung Above")
                .clicked()
                {
                    if let Some(idx) = cur_idx {
                        app.doc.copy_rung_at(idx, idx);
                    }
                }

                if network_icon_btn(
                    ui,
                    VectorIcon::DeleteRung,
                    palette.accent_danger,
                    has_rung,
                    btn_w,
                    palette,
                )
                .on_hover_text("Delete Rung (Delete)")
                .clicked()
                {
                    if let Some(idx) = cur_idx {
                        app.doc.delete_rung(idx);
                    }
                }
            });
        });

        ui.add_space(5.0);

        // --- 2. Components Palette Card: Smart Search + Quick Insert + Dynamic Accordion ---
        lp_card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());

            // Top Search Row
            let mut search_has_focus = false;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(RichText::new("Find:").size(10.5).color(palette.rung_text_dim));
                let edit_w = (ui.available_width()
                    - if !app.component_filter.is_empty() {
                        22.0
                    } else {
                        0.0
                    })
                .max(60.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut app.component_filter)
                        .hint_text("Search (TON, PID, NC)...")
                        .desired_width(edit_w),
                );
                search_has_focus = resp.has_focus();
                if !app.component_filter.is_empty() {
                    if ui
                        .small_button("×")
                        .on_hover_text("Clear search (Esc)")
                        .clicked()
                    {
                        app.component_filter.clear();
                    }
                }
            });

            if search_has_focus && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                app.component_filter.clear();
            }

            let filter_query = app.component_filter.trim().to_lowercase();

            if search_has_focus
                && !filter_query.is_empty()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
            {
                if let Some(first_match) = ALL_COMPONENTS.iter().find(|c| {
                    c.name.to_lowercase().contains(&filter_query)
                        || c.badge.to_lowercase().contains(&filter_query)
                        || c.category.to_lowercase().contains(&filter_query)
                }) {
                    app.active_tool = first_match.tool;
                }
            }

            ui.add_space(4.0);

            // Compact 2x3 Quick Insert Grid (hidden while filtering so results get full height)
            if filter_query.is_empty() {
                ui.label(
                    RichText::new("Quick Insert")
                        .size(10.5)
                        .color(palette.rung_text_dim)
                        .strong(),
                );
                ui.add_space(2.0);

                let avail_w = ui.available_width();
                let col_w = ((avail_w - 6.0) / 3.0).floor().max(42.0);

                // Row 1: NO, NC, Coil
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    quick_tool_tile(
                        ui,
                        app,
                        "-[ ]-",
                        "NO",
                        ActiveTool::ContactNo,
                        palette.accent_ok,
                        col_w,
                        palette,
                    );
                    quick_tool_tile(
                        ui,
                        app,
                        "-[/]-",
                        "NC",
                        ActiveTool::ContactNc,
                        palette.accent_warn,
                        col_w,
                        palette,
                    );
                    quick_tool_tile(
                        ui,
                        app,
                        "-( )-",
                        "Coil",
                        ActiveTool::CoilNormal,
                        palette.coil_idle,
                        col_w,
                        palette,
                    );
                });
                ui.add_space(3.0);

                // Row 2: Set, Reset, TON
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    quick_tool_tile(
                        ui,
                        app,
                        "-(S)-",
                        "Set",
                        ActiveTool::CoilSet,
                        palette.accent_heading,
                        col_w,
                        palette,
                    );
                    quick_tool_tile(
                        ui,
                        app,
                        "-(R)-",
                        "Reset",
                        ActiveTool::CoilReset,
                        palette.accent_warn,
                        col_w,
                        palette,
                    );
                    quick_tool_tile(
                        ui,
                        app,
                        "TON",
                        "Timer",
                        ActiveTool::Ton,
                        palette.accent_ok,
                        col_w,
                        palette,
                    );
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
            }

            // Collapsing Accordion or Filtered Results
            egui::ScrollArea::vertical()
                .id_salt("left_components_scroll")
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;

                    if !filter_query.is_empty() {
                        let matches: Vec<&ComponentDef> = ALL_COMPONENTS
                            .iter()
                            .filter(|c| {
                                c.name.to_lowercase().contains(&filter_query)
                                    || c.badge.to_lowercase().contains(&filter_query)
                                    || c.category.to_lowercase().contains(&filter_query)
                            })
                            .collect();

                        if matches.is_empty() {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("No matching components")
                                    .size(10.5)
                                    .color(palette.rung_text_dim),
                            );
                        } else {
                            ui.label(
                                RichText::new(format!("{} matches", matches.len()))
                                    .size(10.0)
                                    .color(palette.rung_text_dim),
                            );
                            ui.add_space(2.0);
                            for comp in matches {
                                component_accordion_item(
                                    ui,
                                    app,
                                    comp.icon,
                                    comp.badge,
                                    comp.name,
                                    comp.tool,
                                    comp.accent(palette),
                                    palette,
                                );
                            }
                        }
                    } else {
                        for &(cat_key, label, accent_kind, default_open) in CATEGORIES {
                            let count = ALL_COMPONENTS
                                .iter()
                                .filter(|c| c.category == cat_key)
                                .count();
                            egui::CollapsingHeader::new(
                                RichText::new(format!("{} ({})", label, count))
                                    .size(11.5)
                                    .strong()
                                    .color(accent_color(accent_kind, palette)),
                            )
                            .id_salt(format!("lp_cat_{}", cat_key))
                            .default_open(default_open)
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                for comp in ALL_COMPONENTS.iter().filter(|c| c.category == cat_key)
                                {
                                    component_accordion_item(
                                        ui,
                                        app,
                                        comp.icon,
                                        comp.badge,
                                        comp.name,
                                        comp.tool,
                                        comp.accent(palette),
                                        palette,
                                    );
                                }
                            });
                        }
                    }
                });
        });
    });
}

fn network_icon_btn(
    ui: &mut Ui,
    icon: VectorIcon,
    accent: Color32,
    enabled: bool,
    width: f32,
    palette: ThemePalette,
) -> egui::Response {
    let height = 26.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());

    if ui.is_rect_visible(rect) {
        let hovered = enabled && response.hovered();
        let pressed = enabled && response.is_pointer_button_down_on();

        let bg = if !enabled {
            if palette.is_light() {
                Color32::from_rgb(240, 243, 248)
            } else {
                palette.pill_bg.gamma_multiply(0.4)
            }
        } else if pressed {
            accent.gamma_multiply(0.3)
        } else if hovered {
            palette.rung_badge_hov_bg
        } else {
            palette.card_bg
        };

        let border_color = if !enabled {
            palette.rung_badge_border.gamma_multiply(0.35)
        } else if hovered || pressed {
            accent.gamma_multiply(0.85)
        } else {
            palette.rung_badge_border
        };

        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(4), bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(if hovered { 1.2 } else { 1.0 }, border_color),
            StrokeKind::Inside,
        );

        let icon_color = if !enabled {
            palette.rung_text_dim.gamma_multiply(0.35)
        } else if hovered || pressed {
            accent
        } else {
            accent.gamma_multiply(0.85)
        };

        let icon_rect = Rect::from_center_size(rect.center(), Vec2::new(14.0, 14.0));
        paint_icon(painter, icon_rect, icon, icon_color);
    }

    response
}

fn quick_tool_tile(
    ui: &mut Ui,
    app: &mut CanvasApp,
    symbol: &str,
    label: &str,
    tool: ActiveTool,
    accent: Color32,
    width: f32,
    palette: ThemePalette,
) -> egui::Response {
    let is_active = app.active_tool == tool;
    let height = 30.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let bg = if is_active {
            if palette.is_light() {
                Color32::from_rgb(218, 232, 252)
            } else {
                Color32::from_rgb(40, 58, 86)
            }
        } else if hovered {
            palette.rung_badge_hov_bg
        } else {
            palette.card_bg
        };

        let border_color = if is_active {
            if palette.is_light() {
                Color32::from_rgb(35, 110, 215)
            } else {
                Color32::from_rgb(85, 145, 230)
            }
        } else if hovered {
            accent.gamma_multiply(0.8)
        } else {
            palette.rung_badge_border
        };

        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(4), bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(if is_active { 1.5 } else { 1.0 }, border_color),
            StrokeKind::Inside,
        );

        // Top line: Symbol in monospace
        let sym_color = if is_active {
            if palette.is_light() {
                Color32::from_rgb(18, 75, 175)
            } else {
                Color32::WHITE
            }
        } else {
            accent
        };
        painter.text(
            Pos2::new(rect.center().x, rect.min.y + 9.5),
            Align2::CENTER_CENTER,
            symbol,
            FontId::monospace(10.5),
            sym_color,
        );

        // Bottom line: Label
        let lbl_color = if is_active {
            if palette.is_light() {
                Color32::from_rgb(20, 28, 45)
            } else {
                palette.text_primary
            }
        } else {
            palette.rung_text_dim
        };
        painter.text(
            Pos2::new(rect.center().x, rect.min.y + 21.5),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(9.0),
            lbl_color,
        );
    }

    if response.drag_started() || (response.dragged() && app.canvas.dragging_tool.is_none()) {
        app.canvas.start_tool_drag(tool, label);
    }
    if response.clicked() {
        if app.active_tool == tool {
            app.active_tool = ActiveTool::Select;
        } else {
            app.active_tool = tool;
        }
    }

    response.on_hover_text(format!("{} (Click to select, drag to canvas)", tool.name()))
}

fn component_accordion_item(
    ui: &mut Ui,
    app: &mut CanvasApp,
    icon: Option<VectorIcon>,
    badge: &str,
    name: &str,
    tool: ActiveTool,
    accent: Color32,
    palette: ThemePalette,
) {
    let is_active = app.active_tool == tool;
    let width = ui.available_width();
    let height = 26.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let bg = if is_active {
            if palette.is_light() {
                Color32::from_rgb(220, 234, 252)
            } else {
                Color32::from_rgb(38, 55, 82)
            }
        } else if hovered {
            palette.rung_badge_hov_bg
        } else {
            Color32::TRANSPARENT
        };

        let border_color = if is_active {
            if palette.is_light() {
                Color32::from_rgb(45, 120, 225)
            } else {
                Color32::from_rgb(85, 145, 230)
            }
        } else if hovered {
            palette.card_border
        } else {
            Color32::TRANSPARENT
        };

        let painter = ui.painter();
        if is_active || hovered {
            painter.rect_filled(rect, CornerRadius::same(4), bg);
            painter.rect_stroke(
                rect,
                CornerRadius::same(4),
                Stroke::new(1.0, border_color),
                StrokeKind::Inside,
            );
        }

        // Left Symbol Badge
        let badge_w = 34.0;
        let badge_rect = Rect::from_min_max(
            Pos2::new(rect.min.x + 3.0, rect.min.y + 3.0),
            Pos2::new(rect.min.x + 3.0 + badge_w, rect.max.y - 3.0),
        );
        let badge_bg = if is_active {
            accent.gamma_multiply(0.25)
        } else {
            palette.pill_bg
        };
        painter.rect_filled(badge_rect, CornerRadius::same(3), badge_bg);
        painter.rect_stroke(
            badge_rect,
            CornerRadius::same(3),
            Stroke::new(
                0.8,
                if is_active {
                    accent
                } else {
                    palette.banner_border.gamma_multiply(0.6)
                },
            ),
            StrokeKind::Inside,
        );

        if let Some(ico) = icon {
            let ico_color = if is_active {
                accent
            } else {
                accent.gamma_multiply(0.85)
            };
            paint_icon(painter, badge_rect.shrink(2.0), ico, ico_color);
        } else {
            let badge_text_color = if is_active {
                accent
            } else if palette.is_light() {
                Color32::from_rgb(30, 40, 55)
            } else {
                palette.rung_text_num
            };
            painter.text(
                badge_rect.center(),
                Align2::CENTER_CENTER,
                badge,
                FontId::monospace(10.0),
                badge_text_color,
            );
        }

        // Right Name text
        let text_color = if is_active {
            if palette.is_light() {
                Color32::from_rgb(18, 25, 40)
            } else {
                Color32::WHITE
            }
        } else if hovered {
            palette.text_primary
        } else {
            palette.rung_text_num
        };

        painter.text(
            Pos2::new(rect.min.x + badge_w + 8.0, rect.center().y),
            Align2::LEFT_CENTER,
            name,
            FontId::proportional(11.0),
            text_color,
        );
    }

    if response.drag_started() || (response.dragged() && app.canvas.dragging_tool.is_none()) {
        app.canvas.start_tool_drag(tool, name);
    }
    if response.clicked() {
        if app.active_tool == tool {
            app.active_tool = ActiveTool::Select;
        } else {
            app.active_tool = tool;
        }
    }

    response.on_hover_text(format!("{} (Click to select, drag to canvas)", tool.name()));
}

const CATEGORIES: &[(&str, &str, u8, bool)] = &[
    ("Bit Logic", "Bit Logic", 2, true),
    ("Timers & Counters", "Timers & Counters", 0, false),
    ("Edge Triggers", "Edge Triggers", 0, false),
    ("Math & Compare", "Math & Compare", 1, false),
    ("Program Flow", "Program Flow", 2, false),
    ("Process Control", "Process Control", 0, false),
    ("Bitwise & Shift", "Bitwise & Shift", 1, false),
    ("Communication", "Communication", 2, false),
];

struct ComponentDef {
    icon: Option<VectorIcon>,
    badge: &'static str,
    name: &'static str,
    tool: ActiveTool,
    category: &'static str,
    accent_kind: u8,
}

impl ComponentDef {
    fn accent(&self, palette: ThemePalette) -> Color32 {
        accent_color(self.accent_kind, palette)
    }
}

const ALL_COMPONENTS: &[ComponentDef] = &[
    // Bit Logic (17)
    ComponentDef {
        icon: Some(VectorIcon::ContactNo),
        badge: "-[ ]-",
        name: "NO Contact [ ]",
        tool: ActiveTool::ContactNo,
        category: "Bit Logic",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::ContactNc),
        badge: "-[/]-",
        name: "NC Contact [/]",
        tool: ActiveTool::ContactNc,
        category: "Bit Logic",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::PulseEdge),
        badge: "-[P]-",
        name: "P Contact (Rising)",
        tool: ActiveTool::ContactP,
        category: "Bit Logic",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::PulseEdge),
        badge: "-[N]-",
        name: "N Contact (Falling)",
        tool: ActiveTool::ContactN,
        category: "Bit Logic",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::Coil),
        badge: "-( )-",
        name: "Coil (Normal Out)",
        tool: ActiveTool::CoilNormal,
        category: "Bit Logic",
        accent_kind: 3,
    },
    ComponentDef {
        icon: Some(VectorIcon::Coil),
        badge: "-(/)-",
        name: "Negated Coil (/)",
        tool: ActiveTool::CoilNegated,
        category: "Bit Logic",
        accent_kind: 4,
    },
    ComponentDef {
        icon: Some(VectorIcon::CoilSet),
        badge: "-(S)-",
        name: "Set Latch Coil (S)",
        tool: ActiveTool::CoilSet,
        category: "Bit Logic",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::CoilReset),
        badge: "-(R)-",
        name: "Reset Coil (R)",
        tool: ActiveTool::CoilReset,
        category: "Bit Logic",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::PulseEdge),
        badge: "-(P)-",
        name: "Positive Edge Coil",
        tool: ActiveTool::CoilP,
        category: "Bit Logic",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::PulseEdge),
        badge: "-(N)-",
        name: "Negative Edge Coil",
        tool: ActiveTool::CoilN,
        category: "Bit Logic",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "SR",
        name: "SR Bistable FB",
        tool: ActiveTool::Sr,
        category: "Bit Logic",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "RS",
        name: "RS Bistable FB",
        tool: ActiveTool::Rs,
        category: "Bit Logic",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::MasterRelay),
        badge: "MCR",
        name: "Master Relay (MC)",
        tool: ActiveTool::MasterRelay,
        category: "Bit Logic",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::ParallelBranch),
        badge: "-[∥]-",
        name: "Parallel Branch",
        tool: ActiveTool::Branch,
        category: "Bit Logic",
        accent_kind: 0,
    },
    ComponentDef {
        icon: None,
        badge: "───",
        name: "Short Circuit Wire",
        tool: ActiveTool::Wire,
        category: "Bit Logic",
        accent_kind: 5,
    },
    ComponentDef {
        icon: None,
        badge: "─ ─",
        name: "Open Circuit",
        tool: ActiveTool::OpenCircuit,
        category: "Bit Logic",
        accent_kind: 4,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "ALT",
        name: "ALT (Toggle / Flip-Flop)",
        tool: ActiveTool::Toggle,
        category: "Bit Logic",
        accent_kind: 1,
    },
    // Timers & Counters (6)
    ComponentDef {
        icon: Some(VectorIcon::Timer),
        badge: "TON",
        name: "TON (On-Delay)",
        tool: ActiveTool::Ton,
        category: "Timers & Counters",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::Timer),
        badge: "TOF",
        name: "TOF (Off-Delay)",
        tool: ActiveTool::Tof,
        category: "Timers & Counters",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::Timer),
        badge: "TP",
        name: "TP (Pulse Timer)",
        tool: ActiveTool::Tp,
        category: "Timers & Counters",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::Counter),
        badge: "CTU",
        name: "CTU (Count Up)",
        tool: ActiveTool::Ctu,
        category: "Timers & Counters",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::Counter),
        badge: "CTD",
        name: "CTD (Count Down)",
        tool: ActiveTool::Ctd,
        category: "Timers & Counters",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::Counter),
        badge: "CTUD",
        name: "CTUD (Up/Down)",
        tool: ActiveTool::Ctud,
        category: "Timers & Counters",
        accent_kind: 1,
    },
    // Edge Triggers (2)
    ComponentDef {
        icon: Some(VectorIcon::PulseEdge),
        badge: "R_TRG",
        name: "R_TRIG (Rising Edge)",
        tool: ActiveTool::RTrig,
        category: "Edge Triggers",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::PulseEdge),
        badge: "F_TRG",
        name: "F_TRIG (Falling Edge)",
        tool: ActiveTool::FTrig,
        category: "Edge Triggers",
        accent_kind: 1,
    },
    // Math & Compare (10)
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: ":=",
        name: "Move [:=]",
        tool: ActiveTool::Move,
        category: "Math & Compare",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "CMP",
        name: "Compare [==, !=, ...]",
        tool: ActiveTool::Compare,
        category: "Math & Compare",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "+=",
        name: "Add [+=]",
        tool: ActiveTool::Add,
        category: "Math & Compare",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "-=",
        name: "Sub [-=]",
        tool: ActiveTool::Sub,
        category: "Math & Compare",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "*=",
        name: "Mul [*=]",
        tool: ActiveTool::Mul,
        category: "Math & Compare",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "/=",
        name: "Div [/=]",
        tool: ActiveTool::Div,
        category: "Math & Compare",
        accent_kind: 4,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "%=",
        name: "Mod [%=]",
        tool: ActiveTool::Mod,
        category: "Math & Compare",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "RNG",
        name: "IN_RANGE (Window Compare)",
        tool: ActiveTool::InRange,
        category: "Math & Compare",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "SEL",
        name: "SEL (Binary Selector)",
        tool: ActiveTool::Sel,
        category: "Math & Compare",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "±1",
        name: "INC / DEC (1-Shot Step)",
        tool: ActiveTool::IncDec,
        category: "Math & Compare",
        accent_kind: 1,
    },
    // Program Flow (3)
    ComponentDef {
        icon: Some(VectorIcon::MoveDown),
        badge: "JMP",
        name: "JMP (Jump to Label)",
        tool: ActiveTool::Jump,
        category: "Program Flow",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::MoveUp),
        badge: "LBL",
        name: "LBL (Target Label)",
        tool: ActiveTool::Label,
        category: "Program Flow",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "RET",
        name: "RET (Conditional Return)",
        tool: ActiveTool::Return,
        category: "Program Flow",
        accent_kind: 4,
    },
    // Process Control (5)
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "PID",
        name: "PID Controller",
        tool: ActiveTool::Pid,
        category: "Process Control",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "SCL",
        name: "Scale (Linear Map)",
        tool: ActiveTool::Scale,
        category: "Process Control",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "LIM",
        name: "Limit (Clamp Min/Max)",
        tool: ActiveTool::Limit,
        category: "Process Control",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "ADC",
        name: "A/D Converter Read",
        tool: ActiveTool::Adc,
        category: "Process Control",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "PWM",
        name: "PWM Output (%QW)",
        tool: ActiveTool::Pwm,
        category: "Process Control",
        accent_kind: 0,
    },
    // Bitwise & Shift (10)
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "AND",
        name: "Bitwise AND [&]",
        tool: ActiveTool::BitAnd,
        category: "Bitwise & Shift",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "OR",
        name: "Bitwise OR [|]",
        tool: ActiveTool::BitOr,
        category: "Bitwise & Shift",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "XOR",
        name: "Bitwise XOR [^]",
        tool: ActiveTool::BitXor,
        category: "Bitwise & Shift",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "NOT",
        name: "Bitwise NOT [~]",
        tool: ActiveTool::BitNot,
        category: "Bitwise & Shift",
        accent_kind: 4,
    },
    ComponentDef {
        icon: Some(VectorIcon::ShiftReg),
        badge: "SHL",
        name: "Shift Left (SHL)",
        tool: ActiveTool::ShiftLeft,
        category: "Bitwise & Shift",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::ShiftReg),
        badge: "SHR",
        name: "Shift Right (SHR)",
        tool: ActiveTool::ShiftRight,
        category: "Bitwise & Shift",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::ShiftReg),
        badge: "SFT",
        name: "Shift Register (Array)",
        tool: ActiveTool::ShiftReg,
        category: "Bitwise & Shift",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::ShiftReg),
        badge: "SBIT",
        name: "Bit Shift In (SFT_BIT)",
        tool: ActiveTool::ShiftRegInbit,
        category: "Bitwise & Shift",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::ShiftReg),
        badge: "SHV",
        name: "Shift Variable (SHV)",
        tool: ActiveTool::ShiftVariable,
        category: "Bitwise & Shift",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::ShiftReg),
        badge: "SRST",
        name: "Shift Reg Reset (SRST)",
        tool: ActiveTool::ShiftRegReset,
        category: "Bitwise & Shift",
        accent_kind: 4,
    },
    // Communication (4)
    ComponentDef {
        icon: Some(VectorIcon::SerialComm),
        badge: "TX",
        name: "UART Send (TX)",
        tool: ActiveTool::UartSend,
        category: "Communication",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::SerialComm),
        badge: "RX",
        name: "UART Recv (RX)",
        tool: ActiveTool::UartRecv,
        category: "Communication",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::SerialComm),
        badge: "MB",
        name: "Modbus Master (RS-485)",
        tool: ActiveTool::MbMaster,
        category: "Communication",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::SerialComm),
        badge: "MBS",
        name: "Modbus Slave (RS-485)",
        tool: ActiveTool::MbSlave,
        category: "Communication",
        accent_kind: 0,
    },
];
