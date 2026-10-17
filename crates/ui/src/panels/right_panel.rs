use egui::{Color32, CornerRadius, RichText, Stroke, Ui, Vec2};
use crate::app::CanvasApp;
use crate::theme::ThemePalette;
use my_canvas_core::{CoilMode, ContactMode, TypeTag};

pub fn render(ui: &mut Ui, app: &mut CanvasApp) {
    let palette = app.theme.palette();
    let sim_active = app.doc.simulator.is_active();

    // -------------------------------------------------------------------------
    // Top Navigation: Modern Segmented Tab Bar
    // -------------------------------------------------------------------------
    let has_elem_sel = app.doc.selected_element_id.is_some() || !app.doc.selected_element_ids.is_empty();
    let has_rung_sel = app.doc.selected_rung_idx.is_some();

    egui::Frame::new()
        .fill(if palette.is_light() {
            Color32::from_rgb(228, 233, 241)
        } else {
            palette.pill_bg
        })
        .corner_radius(CornerRadius::same(6))
        .stroke(Stroke::new(1.0, palette.card_border.gamma_multiply(0.6)))
        .inner_margin(egui::Margin::symmetric(3, 3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().button_padding = Vec2::new(6.0, 3.0);

            ui.horizontal(|ui| {
                let prop_label = if has_elem_sel {
                    "⚙ Property ●"
                } else if has_rung_sel {
                    "⚙ Network ●"
                } else {
                    "⚙ Property"
                };

                let is_tab0 = app.right_panel_tab == 0;
                let tab0_col = if is_tab0 {
                    Color32::WHITE
                } else if has_elem_sel || has_rung_sel {
                    palette.accent_heading
                } else {
                    palette.rung_text_dim
                };
                let t0 = ui.selectable_label(
                    is_tab0,
                    RichText::new(prop_label).size(11.5).strong().color(tab0_col),
                );
                if t0.clicked() {
                    app.right_panel_tab = 0;
                }

                let tags_count = app.doc.program.collect_all_elements().len();
                let is_tab1 = app.right_panel_tab == 1;
                let tab1_col = if is_tab1 {
                    Color32::WHITE
                } else {
                    palette.rung_text_dim
                };
                let t1 = ui.selectable_label(
                    is_tab1,
                    RichText::new(format!("📋 IEC / Symbol ({})", tags_count))
                        .size(11.5)
                        .strong()
                        .color(tab1_col),
                );
                if t1.clicked() {
                    app.right_panel_tab = 1;
                }

                let in_count = app.doc.program.io_mappings.iter().filter(|m| m.is_input).count();
                let out_count = app.doc.program.io_mappings.iter().filter(|m| !m.is_input).count();
                let is_tab2 = app.right_panel_tab == 2;
                let tab2_col = if is_tab2 {
                    Color32::WHITE
                } else {
                    palette.rung_text_dim
                };
                let t2 = ui.selectable_label(
                    is_tab2,
                    RichText::new(format!("⚡ I/O ({}/{})", in_count, out_count))
                        .size(11.5)
                        .strong()
                        .color(tab2_col),
                );
                if t2.clicked() {
                    app.right_panel_tab = 2;
                }
            });
        });

    ui.add_space(3.0);
    ui.separator();

    // -------------------------------------------------------------------------
    // Tab Contents
    // -------------------------------------------------------------------------
    match app.right_panel_tab {
        0 => render_element_inspector(ui, app, palette, sim_active),
        1 => render_elements_explorer(ui, app, palette, sim_active),
        2 => render_io_ports(ui, app, palette),
        _ => render_element_inspector(ui, app, palette, sim_active),
    }
}

// =============================================================================
// TAB 0: PROPERTY INSPECTOR
// =============================================================================
fn render_element_inspector(
    ui: &mut Ui,
    app: &mut CanvasApp,
    palette: ThemePalette,
    sim_active: bool,
) {
    egui::ScrollArea::vertical()
        .id_salt("element_inspector_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut open_full_dialog = false;
            let mut deleted = false;
            let mut toggle_multi_mode = false;
            let mut clear_multi_sel = false;
            let mut new_contact_mode: Option<ContactMode> = None;
            let mut new_coil_mode: Option<CoilMode> = None;
            let mut quick_assign: Option<(String, String)> = None;
            let mut set_preset: Option<i32> = None;
            let mut toggle_input_var: Option<String> = None;

            if app.doc.selected_element_ids.len() > 1 {
                // Multi-Element Selection Mode
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    let count = app.doc.selected_element_ids.len();
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Multi-Selection ({} elements)", count))
                                .color(palette.accent_heading)
                                .strong(),
                        );
                    });
                    ui.separator();

                    let mut tags = Vec::new();
                    for &id in &app.doc.selected_element_ids {
                        if let Some(el) = app.doc.element_by_id(id) {
                            tags.push(format!("{} ({})", el.name, el.element_type.display_name()));
                        }
                    }
                    ui.label(
                        RichText::new(tags.join(", "))
                            .monospace()
                            .small()
                            .color(palette.text_primary),
                    );
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button(RichText::new("Invert Mode (NO ↔ NC)").color(palette.accent_warn))
                            .clicked()
                        {
                            toggle_multi_mode = true;
                        }
                        if ui
                            .button(
                                RichText::new(format!("Delete All ({})", count))
                                    .color(palette.accent_danger),
                            )
                            .clicked()
                        {
                            deleted = true;
                        }
                        if ui.button("Clear Selection (Esc)").clicked() {
                            clear_multi_sel = true;
                        }
                    });
                });
            } else if let Some(elem) = app.doc.selected_element_mut() {
                // Single Element Selected

                // 1. Header Card: Type Badge & Status
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let func_col = if palette.is_light() {
                            Color32::from_rgb(112, 42, 188)
                        } else {
                            Color32::from_rgb(180, 110, 240)
                        };
                        let (type_color, type_title) = match elem.element_type {
                            TypeTag::Contact => (palette.accent_heading, "CONTACT"),
                            TypeTag::Coil => (palette.accent_ok, "COIL"),
                            TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => (palette.accent_warn, "TIMER"),
                            TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => (palette.accent_warn, "COUNTER"),
                            TypeTag::Sr | TypeTag::Rs => (palette.accent_warn, "BISTABLE FB"),
                            TypeTag::Jump | TypeTag::Label => (palette.accent_heading, "FLOW"),
                            TypeTag::Compare | TypeTag::Math | TypeTag::Move => (func_col, "FUNCTION"),
                            _ => (palette.rung_text_dim, "ELEMENT"),
                        };

                        let badge_frame = egui::Frame::new()
                            .fill(type_color.gamma_multiply(0.15))
                            .stroke(Stroke::new(1.0, type_color))
                            .corner_radius(CornerRadius::same(3))
                            .inner_margin(egui::Margin::symmetric(5, 2));

                        badge_frame.show(ui, |ui| {
                            ui.label(RichText::new(type_title).color(type_color).size(10.5).strong());
                        });

                        ui.label(RichText::new(format!("#{}", elem.id)).monospace().small().color(palette.rung_text_dim));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(RichText::new("Dialog ↗").color(palette.accent_heading).small()).clicked() {
                                open_full_dialog = true;
                            }
                            if sim_active {
                                let is_on = elem.is_done;
                                let status_badge = if is_on {
                                    RichText::new("● ACTIVE").color(palette.accent_ok).size(10.5).strong()
                                } else {
                                    RichText::new("○ OFF").color(palette.rung_text_dim).size(10.5)
                                };
                                ui.label(status_badge);
                            }
                        });
                    });
                });

                ui.add_space(4.0);

                // 2. Identification Card (IEC Address, Symbol, Remark)
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("IEC Address & Symbol").strong().size(11.5).color(palette.accent_heading));
                    ui.separator();

                    egui::Grid::new("elem_id_grid")
                        .num_columns(2)
                        .spacing([8.0, 6.0])
                        .min_col_width(72.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new("IEC Address:").color(palette.text_primary));
                            let field_w = (ui.available_width() - 4.0).max(115.0);
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut elem.name)
                                    .hint_text("e.g. %IX0.0, %QX0.0, %MX0.0")
                                    .desired_width(field_w)
                            );
                            if resp.changed() {
                                elem.name = elem.name.to_ascii_uppercase();
                            }
                            ui.end_row();

                            ui.label(RichText::new("Symbol:").color(palette.text_primary));
                            let field_w = (ui.available_width() - 4.0).max(115.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut elem.symbol)
                                    .hint_text("e.g. START_PB")
                                    .desired_width(field_w)
                            );
                            ui.end_row();

                            ui.label(RichText::new("Remark:").color(palette.text_primary));
                            let field_w = (ui.available_width() - 4.0).max(115.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut elem.comment)
                                    .hint_text("Description...")
                                    .desired_width(field_w)
                            );
                            ui.end_row();
                        });
                });

                ui.add_space(4.0);

                // 3. Mode & Specific Parameters Card
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    match elem.element_type {
                        TypeTag::Contact => {
                            ui.label(RichText::new("Contact Mode").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();

                            egui::Grid::new("contact_mode_grid")
                                .num_columns(2)
                                .spacing([6.0, 6.0])
                                .show(ui, |ui| {
                                    let is_no = elem.contact_mode == ContactMode::NormallyOpen;
                                    if ui
                                        .selectable_label(is_no, "-[ ]- NO (Open)")
                                        .on_hover_text("Normally Open Contact")
                                        .clicked()
                                        && !is_no
                                    {
                                        new_contact_mode = Some(ContactMode::NormallyOpen);
                                    }

                                    let is_nc = elem.contact_mode == ContactMode::NormallyClosed;
                                    if ui
                                        .selectable_label(is_nc, "-[/]- NC (Closed)")
                                        .on_hover_text("Normally Closed Contact")
                                        .clicked()
                                        && !is_nc
                                    {
                                        new_contact_mode = Some(ContactMode::NormallyClosed);
                                    }
                                    ui.end_row();

                                    let is_p = elem.contact_mode == ContactMode::PositiveTransition;
                                    if ui
                                        .selectable_label(is_p, "-[P]- Rising (P)")
                                        .on_hover_text("Positive Transition-Sensing Contact (Rising Edge)")
                                        .clicked()
                                        && !is_p
                                    {
                                        new_contact_mode = Some(ContactMode::PositiveTransition);
                                    }

                                    let is_n = elem.contact_mode == ContactMode::NegativeTransition;
                                    if ui
                                        .selectable_label(is_n, "-[N]- Falling (N)")
                                        .on_hover_text("Negative Transition-Sensing Contact (Falling Edge)")
                                        .clicked()
                                        && !is_n
                                    {
                                        new_contact_mode = Some(ContactMode::NegativeTransition);
                                    }
                                    ui.end_row();
                                });
                        }
                        TypeTag::Coil => {
                            ui.label(RichText::new("Coil Mode").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();

                            egui::Grid::new("coil_mode_grid")
                                .num_columns(2)
                                .spacing([6.0, 6.0])
                                .show(ui, |ui| {
                                    let is_norm = elem.coil_mode == CoilMode::Normal;
                                    if ui
                                        .selectable_label(is_norm, "-( )- Normal")
                                        .on_hover_text("Normal Output Coil")
                                        .clicked()
                                        && !is_norm
                                    {
                                        new_coil_mode = Some(CoilMode::Normal);
                                    }

                                    let is_neg = elem.coil_mode == CoilMode::Negated;
                                    if ui
                                        .selectable_label(is_neg, "-(/)- Negated")
                                        .on_hover_text("Negated / Inverted Output Coil")
                                        .clicked()
                                        && !is_neg
                                    {
                                        new_coil_mode = Some(CoilMode::Negated);
                                    }
                                    ui.end_row();

                                    let is_set = elem.coil_mode == CoilMode::SetLatch;
                                    if ui
                                        .selectable_label(is_set, "-(S)- Set Latch")
                                        .on_hover_text("Set (Latch) Output Coil")
                                        .clicked()
                                        && !is_set
                                    {
                                        new_coil_mode = Some(CoilMode::SetLatch);
                                    }

                                    let is_rst = elem.coil_mode == CoilMode::ResetUnlatch;
                                    if ui
                                        .selectable_label(is_rst, "-(R)- Reset")
                                        .on_hover_text("Reset (Unlatch) Output Coil")
                                        .clicked()
                                        && !is_rst
                                    {
                                        new_coil_mode = Some(CoilMode::ResetUnlatch);
                                    }
                                    ui.end_row();

                                    let is_pos = elem.coil_mode == CoilMode::PositiveTransition;
                                    if ui
                                        .selectable_label(is_pos, "-(P)- Rising (P)")
                                        .on_hover_text("Positive Transition-Sensing Coil (Rising Pulse)")
                                        .clicked()
                                        && !is_pos
                                    {
                                        new_coil_mode = Some(CoilMode::PositiveTransition);
                                    }

                                    let is_neg_p = elem.coil_mode == CoilMode::NegativeTransition;
                                    if ui
                                        .selectable_label(is_neg_p, "-(N)- Falling (N)")
                                        .on_hover_text("Negative Transition-Sensing Coil (Falling Pulse)")
                                        .clicked()
                                        && !is_neg_p
                                    {
                                        new_coil_mode = Some(CoilMode::NegativeTransition);
                                    }
                                    ui.end_row();
                                });
                        }
                        TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => {
                            let fb_name = match elem.element_type {
                                TypeTag::Ton => "On-Delay Timer (TON)",
                                TypeTag::Tof => "Off-Delay Timer (TOF)",
                                _ => "Pulse Timer (TP)",
                            };
                            ui.label(RichText::new(fb_name).strong().size(11.5).color(palette.accent_heading));
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Preset (PT):").color(palette.text_primary));
                                ui.add(
                                    egui::DragValue::new(&mut elem.preset_value)
                                        .range(10..=600000)
                                        .speed(50)
                                        .suffix(" ms"),
                                );
                                let sec = elem.preset_value as f32 / 1000.0;
                                ui.label(RichText::new(format!("({:.2}s)", sec)).small().color(palette.rung_text_dim));
                            });

                            ui.add_space(4.0);
                            ui.label(RichText::new("Quick Presets:").small().color(palette.rung_text_dim));
                            ui.horizontal_wrapped(|ui| {
                                for &(label, ms) in &[
                                    ("100ms", 100),
                                    ("500ms", 500),
                                    ("1.0s", 1000),
                                    ("2.0s", 2000),
                                    ("3.0s", 3000),
                                    ("5.0s", 5000),
                                    ("10s", 10000),
                                ] {
                                    if ui.small_button(label).clicked() {
                                        set_preset = Some(ms);
                                    }
                                }
                            });

                            if sim_active {
                                ui.add_space(4.0);
                                let progress = if elem.preset_value > 0 {
                                    (elem.current_accum as f32 / elem.preset_value as f32).clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };
                                ui.add(
                                    egui::ProgressBar::new(progress)
                                        .text(format!("{}/{} ms", elem.current_accum, elem.preset_value))
                                        .animate(sim_active && !elem.is_done),
                                );
                            }
                        }
                        TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => {
                            let fb_name = match elem.element_type {
                                TypeTag::Ctu => "Up Counter (CTU)",
                                TypeTag::Ctd => "Down Counter (CTD)",
                                _ => "Up/Down Counter (CTUD)",
                            };
                            ui.label(RichText::new(fb_name).strong().size(11.5).color(palette.accent_heading));
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Preset (PV):").color(palette.text_primary));
                                ui.add(
                                    egui::DragValue::new(&mut elem.preset_value)
                                        .range(1..=10000)
                                        .speed(1),
                                );
                            });

                            ui.add_space(4.0);
                            ui.label(RichText::new("Quick Presets:").small().color(palette.rung_text_dim));
                            ui.horizontal_wrapped(|ui| {
                                for &pv in &[1, 3, 5, 10, 20, 50, 100] {
                                    if ui.small_button(format!("{}", pv)).clicked() {
                                        set_preset = Some(pv);
                                    }
                                }
                            });

                            if sim_active {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("Live Count: {} / PV: {}", elem.current_accum, elem.preset_value))
                                        .strong()
                                        .color(if elem.is_done { palette.accent_ok } else { palette.accent_warn }),
                                );
                            }
                        }
                        TypeTag::Compare => {
                            ui.label(RichText::new("Compare Logic").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();

                            egui::Grid::new("cmp_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Register Tag:").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.name);
                                    ui.end_row();

                                    ui.label(RichText::new("Operator:").color(palette.text_primary));
                                    egui::ComboBox::from_id_salt("cmp_op_combo")
                                        .width(60.0)
                                        .selected_text(&elem.compare_op)
                                        .show_ui(ui, |ui| {
                                            for op in ["==", "!=", ">", "<", ">=", "<="] {
                                                ui.selectable_value(&mut elem.compare_op, op.to_string(), op);
                                            }
                                        });
                                    ui.end_row();

                                    ui.label(RichText::new("Target Value:").color(palette.text_primary));
                                    ui.add(egui::DragValue::new(&mut elem.preset_value).speed(1));
                                    ui.end_row();
                                });
                        }
                        TypeTag::Math => {
                            ui.label(RichText::new("Math Operation").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();

                            egui::Grid::new("math_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Target Reg:").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.target_reg);
                                    ui.end_row();

                                    ui.label(RichText::new("Operation:").color(palette.text_primary));
                                    egui::ComboBox::from_id_salt("math_op_combo")
                                        .width(60.0)
                                        .selected_text(&elem.math_op)
                                        .show_ui(ui, |ui| {
                                            for op in ["+", "-", "*", "/", "%"] {
                                                ui.selectable_value(&mut elem.math_op, op.to_string(), op);
                                            }
                                        });
                                    ui.end_row();

                                    ui.label(RichText::new("Value:").color(palette.text_primary));
                                    ui.add(egui::DragValue::new(&mut elem.preset_value).speed(1));
                                    ui.end_row();
                                });
                        }
                        TypeTag::Move => {
                            ui.label(RichText::new("Move Data").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();

                            egui::Grid::new("move_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Target Reg:").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.target_reg);
                                    ui.end_row();

                                    ui.label(RichText::new("Value:").color(palette.text_primary));
                                    ui.add(egui::DragValue::new(&mut elem.preset_value).speed(1));
                                    ui.end_row();
                                });
                        }
                        TypeTag::Sr | TypeTag::Rs => {
                            let fb_name = if elem.element_type == TypeTag::Sr {
                                "SR (Set-Dominant Bistable FB)"
                            } else {
                                "RS (Reset-Dominant Bistable FB)"
                            };
                            ui.label(RichText::new(fb_name).strong().size(11.5).color(palette.accent_heading));
                            ui.separator();

                            egui::Grid::new("bistable_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    let r_label = if elem.element_type == TypeTag::Sr { "Reset (R):" } else { "Reset (R1):" };
                                    ui.label(RichText::new(r_label).color(palette.text_primary));
                                    let field_w = (ui.available_width() - 4.0).max(110.0);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut elem.target_reg)
                                            .hint_text("e.g. X1, M1, %IX0.1")
                                            .desired_width(field_w),
                                    );
                                    ui.end_row();

                                    ui.label(RichText::new("Output (Q1):").color(palette.text_primary));
                                    let field_w = (ui.available_width() - 4.0).max(110.0);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut elem.name)
                                            .hint_text("e.g. M0, Y0, %QX0.0")
                                            .desired_width(field_w),
                                    );
                                    ui.end_row();
                                });

                            ui.add_space(4.0);
                            let formula = if elem.element_type == TypeTag::Sr {
                                "Set-dominant: Q1 = S1 OR (NOT R AND Q1_prev)"
                            } else {
                                "Reset-dominant: Q1 = NOT R1 AND (S OR Q1_prev)"
                            };
                            ui.label(RichText::new(formula).small().color(palette.rung_text_dim));
                        }
                        TypeTag::Jump => {
                            ui.label(RichText::new("Conditional Jump (JMP)").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();

                            egui::Grid::new("jump_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Target Label:").color(palette.text_primary));
                                    let field_w = (ui.available_width() - 4.0).max(110.0);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut elem.name)
                                            .hint_text("e.g. LBL1, BYPASS")
                                            .desired_width(field_w),
                                    );
                                    ui.end_row();
                                });
                            ui.add_space(4.0);
                            ui.label(RichText::new("When power is TRUE, jumps to the matching label network.").small().color(palette.rung_text_dim));
                        }
                        TypeTag::Label => {
                            ui.label(RichText::new("Target Label (LBL)").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();

                            egui::Grid::new("label_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Label Name:").color(palette.text_primary));
                                    let field_w = (ui.available_width() - 4.0).max(110.0);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut elem.name)
                                            .hint_text("e.g. LBL1, BYPASS")
                                            .desired_width(field_w),
                                    );
                                    ui.end_row();
                                });
                            ui.add_space(4.0);
                            ui.label(RichText::new("Designates landing destination for JMP instructions.").small().color(palette.rung_text_dim));
                        }
                        _ => {
                            ui.label(RichText::new("No special parameters").small().color(palette.rung_text_dim));
                        }
                    }
                });

                ui.add_space(4.0);

                // 4. Quick IEC Address Assign Pills (1-Click)
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Quick IEC Address Assign (1-Click)").small().color(palette.rung_text_dim));
                    ui.separator();

                    ui.horizontal_wrapped(|ui| {
                        let presets: &[(&str, &str)] = match elem.element_type {
                            TypeTag::Coil => &[
                                ("%QX0.0", "MOTOR_RUN"),
                                ("%QX0.1", "LAMP_RUN"),
                                ("%QX0.2", "ALARM_BUZZ"),
                                ("%QX0.3", "LAMP_FAULT"),
                                ("%MX0.0", "FLAG_READY"),
                                ("%MX0.1", "FLAG_AUTO"),
                                ("%MX0.2", "FLAG_ERROR"),
                            ],
                            TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => &[
                                ("T0", "TIMER_DELAY"),
                                ("T1", "TIMER_PULSE"),
                                ("T2", "TIMER_CYCLE"),
                                ("T3", "TIMER_SAFETY"),
                            ],
                            TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => &[
                                ("C0", "COUNTER_PART"),
                                ("C1", "COUNTER_BATCH"),
                                ("C2", "COUNTER_CYCLE"),
                            ],
                            _ => &[
                                ("%IX0.0", "START_PB"),
                                ("%IX0.1", "STOP_PB"),
                                ("%IX0.2", "SENS_LIMIT"),
                                ("%IX0.3", "EMERGENCY"),
                                ("%QX0.0", "MOTOR_RUN"),
                                ("%MX0.0", "FLAG_READY"),
                                ("T0", "T0_DONE"),
                                ("C0", "C0_DONE"),
                            ],
                        };

                        for &(addr, sym) in presets {
                            let is_cur = elem.name == addr;
                            if ui
                                .selectable_label(is_cur, RichText::new(addr).monospace().small())
                                .clicked()
                                && !is_cur
                            {
                                quick_assign = Some((addr.to_string(), sym.to_string()));
                            }
                        }
                    });
                });

                ui.add_space(6.0);

                // 5. Action Control Bar
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(RichText::new("⚙ Full Dialog (Enter)").color(palette.accent_heading).strong())
                        .clicked()
                    {
                        open_full_dialog = true;
                    }

                    if ui
                        .button(RichText::new("🗑 Delete (Del)").color(palette.accent_danger))
                        .clicked()
                    {
                        deleted = true;
                    }

                    if sim_active && (elem.name.starts_with('X') || elem.name.starts_with('I') || elem.name.starts_with("%IX")) {
                        if ui
                            .button(RichText::new("⚡ Toggle Input (Space)").color(palette.accent_ok))
                            .clicked()
                        {
                            toggle_input_var = Some(elem.name.clone());
                        }
                    }
                });
            } else if let Some(r_idx) = app.doc.selected_rung_idx {
                // Active Network (Rung) Selected (No element selected)
                if let Some(rung) = app.doc.program.rungs.get_mut(r_idx) {
                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("Active Network #{:02}", r_idx + 1))
                                    .color(palette.accent_heading)
                                    .strong(),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if sim_active {
                                    let status_badge = if rung.is_energized {
                                        RichText::new("● ENERGIZED").color(palette.accent_ok).size(10.5).strong()
                                    } else {
                                        RichText::new("○ DE-ENERGIZED").color(palette.rung_text_dim).size(10.5)
                                    };
                                    ui.label(status_badge);
                                }
                            });
                        });
                        ui.separator();

                        ui.label(RichText::new("Network Title / Comment:").color(palette.text_primary));
                        ui.add(
                            egui::TextEdit::singleline(&mut rung.comment)
                                .hint_text("Enter description for this network...")
                                .desired_width(f32::INFINITY),
                        );

                        ui.add_space(4.0);
                        ui.label(RichText::new(format!("Elements: {} in series / parallel", rung.elements.len())).small().color(palette.rung_text_dim));

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui
                                .button(RichText::new("⚙ Network Dialog (Enter)").color(palette.accent_heading))
                                .clicked()
                            {
                                open_full_dialog = true;
                            }
                        });
                    });

                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("💡 Tip: Click any contact or coil inside this network to view its element properties, or press Enter to edit.")
                            .small()
                            .color(palette.rung_text_dim),
                    );
                }
            } else {
                // Nothing Selected: Project Overview & Guide Card
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("Project Overview")
                            .color(palette.accent_heading)
                            .strong(),
                    );
                    ui.separator();

                    ui.label(RichText::new(&app.doc.program.title).strong().color(palette.text_primary));
                    ui.label(RichText::new(format!("Target Hardware: {}", app.doc.program.hardware)).small().color(palette.rung_text_dim));
                    ui.label(RichText::new(format!("Total Networks: {}", app.doc.program.rungs.len())).small().color(palette.rung_text_dim));

                    let total_elems: usize = app.doc.program.rungs.iter().map(|r| r.elements.len()).sum();
                    ui.label(RichText::new(format!("Total Elements: {}", total_elems)).small().color(palette.rung_text_dim));

                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("💡 Click any contact, coil, or network on the canvas to inspect and edit its properties here.")
                            .small()
                            .color(palette.rung_text_dim),
                    );

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("📋 Open IEC / Symbol").clicked() {
                            app.right_panel_tab = 1;
                        }
                        if ui.button("⚡ Open I/O Monitor").clicked() {
                            app.right_panel_tab = 2;
                        }
                    });
                });
            }

            // Apply Mutations
            if let Some(cm) = new_contact_mode {
                app.doc.push_undo_snapshot();
                if let Some(elem) = app.doc.selected_element_mut() {
                    elem.contact_mode = cm;
                }
            }
            if let Some(cm) = new_coil_mode {
                app.doc.push_undo_snapshot();
                if let Some(elem) = app.doc.selected_element_mut() {
                    elem.coil_mode = cm;
                }
            }
            if let Some((addr, sym)) = quick_assign {
                app.doc.push_undo_snapshot();
                if let Some(elem) = app.doc.selected_element_mut() {
                    elem.name = addr;
                    elem.symbol = sym;
                }
            }
            if let Some(val) = set_preset {
                app.doc.push_undo_snapshot();
                if let Some(elem) = app.doc.selected_element_mut() {
                    elem.preset_value = val;
                }
            }
            if let Some(v) = toggle_input_var {
                app.doc.toggle_input(&v);
            }
            if toggle_multi_mode {
                app.doc.toggle_selected_elements_mode();
            }
            if clear_multi_sel {
                app.doc.clear_element_selection();
            }
            if deleted {
                app.doc.delete_selected_element();
            }
            if open_full_dialog {
                app.open_config_for_selected();
            }
        });
}

// =============================================================================
// TAB 1: IEC & SYMBOL EXPLORER
// =============================================================================
fn render_elements_explorer(
    ui: &mut Ui,
    app: &mut CanvasApp,
    palette: ThemePalette,
    sim_active: bool,
) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());

        // Header & Quick Navigation Help
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("IEC & Symbol Explorer")
                    .color(palette.accent_heading)
                    .strong(),
            );
        });
        ui.separator();

        // Filter Bar: Category Tabs
        ui.horizontal(|ui| {
            for (cat_id, lbl) in [
                (0u8, "All"),
                (1u8, "%IX"),
                (2u8, "%QX"),
                (3u8, "%MX"),
                (4u8, "T/C"),
            ] {
                if ui
                    .selectable_label(app.elem_filter_cat == cat_id, RichText::new(lbl).small())
                    .clicked()
                {
                    app.elem_filter_cat = cat_id;
                }
            }
        });

        // Search Bar with Clear button
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.elem_filter_text)
                    .hint_text("🔍 Search IEC address or symbol...")
                    .desired_width(ui.available_width() - 28.0),
            );
            if !app.elem_filter_text.is_empty() {
                if ui.small_button("✖").on_hover_text("Clear filter").clicked() {
                    app.elem_filter_text.clear();
                }
            }
        });
    });

    ui.add_space(4.0);

    let selected_name = app
        .doc
        .selected_element()
        .map(|e| e.name.clone())
        .unwrap_or_default();

    let elements_summary = app.doc.program.collect_all_iec_elements();
    let mut clicked_name: Option<String> = None;
    let mut double_clicked_name: Option<String> = None;
    let mut toggle_name: Option<String> = None;

    let filter_upper = app.elem_filter_text.trim().to_ascii_uppercase();

    // Cross-Ref Data Table
    egui::ScrollArea::vertical()
        .id_salt("listview_elements_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("listview_elements_grid")
                .striped(true)
                .num_columns(4)
                .min_col_width(36.0)
                .spacing([8.0, 5.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("IEC").strong().small().color(palette.rung_text_dim));
                    ui.label(RichText::new("SYMBOL").strong().small().color(palette.rung_text_dim));
                    ui.label(RichText::new("NETWORKS").strong().small().color(palette.rung_text_dim));
                    ui.label(RichText::new("STATE").strong().small().color(palette.rung_text_dim));
                    ui.end_row();

                    for (raw_name, iec_addr, symbol, el_type, val_str) in &elements_summary {
                        let prefix = iec_addr.trim_start_matches('%').chars().next().unwrap_or(' ').to_ascii_uppercase();
                        let cat_match = match app.elem_filter_cat {
                            1 => prefix == 'I' || prefix == 'X',
                            2 => prefix == 'Q' || prefix == 'Y',
                            3 => prefix == 'M' || prefix == 'R' || prefix == 'V',
                            4 => prefix == 'T' || prefix == 'C',
                            _ => true,
                        };
                        if !cat_match {
                            continue;
                        }
                        if !filter_upper.is_empty()
                            && !iec_addr.to_ascii_uppercase().contains(&filter_upper)
                            && !symbol.to_ascii_uppercase().contains(&filter_upper)
                            && !raw_name.to_ascii_uppercase().contains(&filter_upper)
                        {
                            continue;
                        }

                        let is_active = app.doc.simulator.get_var_value(raw_name);
                        let is_selected_tag = !selected_name.is_empty()
                            && (selected_name == *raw_name || selected_name == *iec_addr);
                        let addr_text = if is_selected_tag {
                            // Let selectable_label use the high-contrast active/selection fg_stroke (white on blue in Light/Dark)
                            RichText::new(iec_addr).monospace().strong()
                        } else if is_active {
                            RichText::new(iec_addr).monospace().color(palette.accent_ok)
                        } else {
                            RichText::new(iec_addr).monospace().color(palette.text_primary)
                        };

                        let resp = ui
                            .selectable_label(is_selected_tag, addr_text)
                            .on_hover_text(format!("{} ({}) — Click: Select on canvas | Double-click: Edit Properties", symbol, el_type.display_name()));
                        if resp.clicked() {
                            clicked_name = Some(raw_name.clone());
                        }
                        if resp.double_clicked() {
                            double_clicked_name = Some(raw_name.clone());
                        }

                        let sym_display = if symbol.is_empty() { "-" } else { symbol.as_str() };
                        ui.label(RichText::new(sym_display).small().color(palette.text_primary));

                        // Cross-reference network usages list (e.g. N01, N02)
                        let usages = app.doc.find_address_usages(raw_name);
                        let rung_str = if usages.is_empty() {
                            "-".to_string()
                        } else {
                            usages
                                .iter()
                                .take(4)
                                .map(|(r, _, _)| format!("N{:02}", r + 1))
                                .collect::<Vec<_>>()
                                .join(",")
                        };
                        ui.label(
                            RichText::new(rung_str)
                                .monospace()
                                .small()
                                .color(palette.rung_text_dim),
                        );

                        let (live_val, val_color) = match el_type {
                            TypeTag::Contact | TypeTag::Coil | TypeTag::Sr | TypeTag::Rs => {
                                if is_active {
                                    ("● ON (1)".to_string(), palette.accent_ok)
                                } else {
                                    ("○ OFF (0)".to_string(), palette.rung_text_dim)
                                }
                            }
                            TypeTag::Ton | TypeTag::Tof | TypeTag::Tp if sim_active => {
                                if let Some(&(accum, done)) = app.doc.simulator.timers.get(&raw_name.to_uppercase()) {
                                    let col = if done { palette.accent_ok } else { palette.accent_warn };
                                    (format!("{}ms / {}", accum, val_str), col)
                                } else {
                                    (val_str.clone(), palette.text_primary)
                                }
                            }
                            TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud if sim_active => {
                                if let Some(&(count, done, _)) = app.doc.simulator.counters.get(&raw_name.to_uppercase()) {
                                    let col = if done { palette.accent_ok } else { palette.accent_warn };
                                    (format!("{} / {}", count, val_str), col)
                                } else {
                                    (val_str.clone(), palette.text_primary)
                                }
                            }
                            TypeTag::Compare | TypeTag::Move | TypeTag::Math if sim_active => {
                                if let Some(&reg_v) = app.doc.simulator.registers.get(&raw_name.to_uppercase()) {
                                    (format!("={} ({})", reg_v, val_str), palette.accent_heading)
                                } else {
                                    (val_str.clone(), palette.text_primary)
                                }
                            }
                            _ => (val_str.clone(), palette.text_primary),
                        };
                        if ui
                            .small_button(RichText::new(&live_val).color(val_color))
                            .on_hover_text("Click to toggle in simulation")
                            .clicked()
                        {
                            toggle_name = Some(raw_name.clone());
                        }
                        ui.end_row();
                    }
                });
        });

    if let Some(n) = clicked_name {
        app.doc.select_by_name(&n);
    }
    if let Some(n) = double_clicked_name {
        app.doc.select_by_name(&n);
        if sim_active {
            app.doc.toggle_input(&n);
        } else {
            app.right_panel_tab = 0; // Switch to property inspector
            app.open_config_for_selected();
        }
    }
    if let Some(n) = toggle_name {
        app.doc.toggle_input(&n);
    }
}

// =============================================================================
// TAB 2: I/O PORTS & HARDWARE MONITOR
// =============================================================================
fn render_io_ports(ui: &mut Ui, app: &mut CanvasApp, palette: ThemePalette) {
    let mut toggle_port_var: Option<String> = None;
    let mut add_input_port = false;
    let mut add_output_port = false;

    egui::ScrollArea::vertical()
        .id_salt("io_ports_monitor_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Header: Hardware & Port Summary
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new("I/O Port Mapping & Live Hardware Monitor")
                        .color(palette.accent_heading)
                        .strong(),
                );
                ui.separator();
                ui.label(RichText::new(format!("Platform: {}", app.doc.program.hardware)).small().color(palette.rung_text_dim));
                let in_count = app.doc.program.io_mappings.iter().filter(|m| m.is_input).count();
                let out_count = app.doc.program.io_mappings.iter().filter(|m| !m.is_input).count();
                ui.label(RichText::new(format!("Configured: {} Inputs, {} Outputs", in_count, out_count)).small().color(palette.text_primary));
            });

            ui.add_space(4.0);

            // Digital Inputs Group
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Digital Inputs (%IX)")
                            .strong()
                            .color(palette.accent_warn),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("+ Input").color(palette.accent_warn))
                            .on_hover_text("Add a new Digital Input (%IX)")
                            .clicked()
                        {
                            add_input_port = true;
                        }
                    });
                });
                ui.separator();

                egui::Grid::new("input_ports_grid")
                    .striped(true)
                    .num_columns(3)
                    .spacing([6.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("STATUS").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("IEC").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("SYMBOL").strong().small().color(palette.rung_text_dim));
                        ui.end_row();

                        for (idx, io) in app.doc.program.io_mappings.iter_mut().filter(|m| m.is_input).enumerate() {
                            if io.iec_address.is_empty() {
                                io.iec_address = io.iec_address(idx);
                            }
                            let iec_addr = io.iec_address.clone();
                            let state = app.doc.simulator.get_var_value(&iec_addr);
                            let btn_bg = if state {
                                Color32::from_rgb(28, 140, 62)
                            } else if app.theme.is_light() {
                                Color32::from_rgb(228, 234, 244)
                            } else {
                                Color32::from_rgb(32, 38, 50)
                            };
                            let btn_fg = if state {
                                Color32::WHITE
                            } else {
                                palette.text_primary
                            };

                            let btn = egui::Button::new(
                                RichText::new(if state { "● ON" } else { "○ OFF" })
                                    .size(11.0)
                                    .color(btn_fg),
                            )
                            .min_size(Vec2::new(48.0, 20.0))
                            .fill(btn_bg);

                            if ui
                                .add(btn)
                                .on_hover_text(format!("{} [{}] (Pin {}) — Click to toggle ON/OFF", io.symbol, iec_addr, io.hw_pin))
                                .clicked()
                            {
                                toggle_port_var = Some(iec_addr.clone());
                            }

                            if ui
                                .add_sized(
                                    egui::vec2(85.0, 20.0),
                                    egui::TextEdit::singleline(&mut io.iec_address)
                                        .font(egui::TextStyle::Monospace)
                                        .text_color(palette.accent_warn)
                                        .desired_width(85.0),
                                )
                                .on_hover_text(format!("IEC 61131-3 Input Address (Editable, e.g. %IX0.0, %IX1.0) | Pin: {}", io.hw_pin))
                                .changed()
                            {
                                app.doc.is_dirty = true;
                            }

                            let sym_w = (ui.available_width() - 4.0).max(80.0);
                            if ui.add(egui::TextEdit::singleline(&mut io.symbol).desired_width(sym_w)).changed() {
                                app.doc.is_dirty = true;
                            }
                            ui.end_row();
                        }
                    });
            });

            ui.add_space(4.0);

            // Digital Outputs Group
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Digital Outputs (%QX)")
                            .strong()
                            .color(palette.accent_ok),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("+ Output").color(palette.accent_ok))
                            .on_hover_text("Add a new Digital Output (%QX)")
                            .clicked()
                        {
                            add_output_port = true;
                        }
                    });
                });
                ui.separator();

                egui::Grid::new("output_ports_grid")
                    .striped(true)
                    .num_columns(3)
                    .spacing([6.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("LED").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("IEC").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("SYMBOL").strong().small().color(palette.rung_text_dim));
                        ui.end_row();

                        for (idx, io) in app.doc.program.io_mappings.iter_mut().filter(|m| !m.is_input).enumerate() {
                            if io.iec_address.is_empty() {
                                io.iec_address = io.iec_address(idx);
                            }
                            let iec_addr = io.iec_address.clone();
                            let state = app.doc.simulator.get_var_value(&iec_addr);
                            let badge = if state { "● ON" } else { "○ OFF" };
                            let led_color = if state {
                                palette.wire_energized
                            } else {
                                palette.rung_text_dim
                            };

                            ui.label(
                                RichText::new(badge)
                                    .monospace()
                                    .size(11.0)
                                    .color(led_color),
                            )
                            .on_hover_text(format!("{} [{}] (Pin {})", io.symbol, iec_addr, io.hw_pin));

                            if ui
                                 .add_sized(
                                     egui::vec2(85.0, 20.0),
                                     egui::TextEdit::singleline(&mut io.iec_address)
                                         .font(egui::TextStyle::Monospace)
                                         .text_color(palette.accent_ok)
                                         .desired_width(85.0),
                                 )
                                 .on_hover_text(format!("IEC 61131-3 Output Address (Editable, e.g. %QX0.0, %QX1.0) | Pin: {}", io.hw_pin))
                                 .changed()
                            {
                                app.doc.is_dirty = true;
                            }

                            let sym_w = (ui.available_width() - 4.0).max(80.0);
                            if ui.add(egui::TextEdit::singleline(&mut io.symbol).desired_width(sym_w)).changed() {
                                app.doc.is_dirty = true;
                            }
                            ui.end_row();
                        }
                    });
            });

            ui.add_space(6.0);

            // Live Process Image (%MX, %MW, T, C, ACC)
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Process Image (%MX, %MW, T, C)")
                            .strong()
                            .color(palette.accent_heading),
                    );
                    if app.online_mode {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new("🟢 LIVE")
                                    .size(10.0)
                                    .strong()
                                    .color(palette.accent_ok),
                            );
                        });
                    }
                });
                ui.separator();

                // 1. Accumulator row
                ui.horizontal(|ui| {
                    ui.label(RichText::new("ACC:").strong().small().color(palette.rung_text_dim));
                    let acc_b = app.doc.simulator.acc_bool;
                    let acc_w = app.doc.simulator.acc_word;
                    ui.label(
                        RichText::new(format!("Bit = {}", u8::from(acc_b)))
                            .monospace()
                            .small()
                            .color(if acc_b { palette.accent_ok } else { palette.text_primary }),
                    );
                    ui.separator();
                    ui.label(
                        RichText::new(format!("Word = {}", acc_w))
                            .monospace()
                            .small()
                            .color(palette.accent_heading),
                    );
                });

                ui.add_space(3.0);

                // 2. Internal Markers (%MX0.0 .. %MX0.7)
                ui.label(RichText::new("Markers (%MX0.0..%MX0.7):").small().color(palette.rung_text_dim));
                ui.horizontal_wrapped(|ui| {
                    for i in 0..8 {
                        let addr = format!("%MX0.{}", i);
                        let m_on = app.doc.simulator.get_var_value(&addr);
                        let col = if m_on { palette.accent_ok } else { palette.rung_text_dim };
                        let txt = format!("M{}:{}", i, u8::from(m_on));
                        if ui
                            .selectable_label(m_on, RichText::new(txt).monospace().size(10.5).color(col))
                            .on_hover_text(format!("{} (M{:02}) — Click to toggle in simulation", addr, i))
                            .clicked()
                        {
                            toggle_port_var = Some(addr);
                        }
                    }
                });

                ui.add_space(3.0);

                // 3. Timers (T0..T3) & Counters (C0..C3)
                egui::Grid::new("live_tc_mw_grid")
                    .striped(true)
                    .num_columns(3)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("TIMERS (T)").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("COUNTERS (C)").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("WORDS (%MW)").strong().small().color(palette.rung_text_dim));
                        ui.end_row();

                        for i in 0..4 {
                            let (t_cv, t_q) = app
                                .doc
                                .simulator
                                .timers
                                .get(&format!("T{}", i))
                                .copied()
                                .unwrap_or((0, false));
                            let t_col = if t_q { palette.accent_ok } else { palette.text_primary };
                            ui.label(
                                RichText::new(format!("T{}: {}ms (Q={})", i, t_cv, u8::from(t_q)))
                                    .monospace()
                                    .size(10.5)
                                    .color(t_col),
                            );

                            let (c_cv, c_q, _) = app
                                .doc
                                .simulator
                                .counters
                                .get(&format!("C{}", i))
                                .copied()
                                .unwrap_or((0, false, false));
                            let c_col = if c_q { palette.accent_ok } else { palette.text_primary };
                            ui.label(
                                RichText::new(format!("C{}: {} (Q={})", i, c_cv, u8::from(c_q)))
                                    .monospace()
                                    .size(10.5)
                                    .color(c_col),
                            );

                            let mw_val = app
                                .doc
                                .simulator
                                .registers
                                .get(&format!("%MW{}", i))
                                .copied()
                                .unwrap_or(0);
                            ui.label(
                                RichText::new(format!("%MW{}: {}", i, mw_val))
                                    .monospace()
                                    .size(10.5)
                                    .color(if mw_val != 0 {
                                        palette.accent_heading
                                    } else {
                                        palette.text_primary
                                    }),
                            );
                            ui.end_row();
                        }
                    });
            });

            ui.add_space(6.0);

            // IEC 61131-3 VAR Table Export / Preview Card
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("IEC 61131-3 VAR Table")
                            .strong()
                            .color(palette.accent_heading),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new("📋 Copy VAR Table").small().color(palette.accent_heading))
                            .on_hover_text("Copy IEC 61131-3 VAR ... END_VAR declarations to clipboard for OpenPLC / CODESYS")
                            .clicked()
                        {
                            let table = app.doc.map_io_to_iec_var_table();
                            ui.ctx().copy_text(table);
                        }
                    });
                });
                ui.separator();

                egui::CollapsingHeader::new(RichText::new("Show VAR ... END_VAR Code").small().color(palette.text_primary))
                    .id_salt("iec_var_table_collapsing")
                    .default_open(false)
                    .show(ui, |ui| {
                        let mut table = app.doc.map_io_to_iec_var_table();
                        ui.add(
                            egui::TextEdit::multiline(&mut table)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(ui.available_width())
                                .desired_rows(10),
                        );
                    });
            });
        });

    if add_input_port {
        let n = app.doc.program.io_mappings.iter().filter(|m| m.is_input).count();
        app.doc.program.io_mappings.push(my_canvas_core::IOPortMapping {
            plc_address: format!("X{}", n),
            iec_address: format!("%IX{}.{}", n / 8, n % 8),
            symbol: format!("IN_{}", n),
            hw_pin: format!("D{}", n + 2),
            description: "Digital Input".to_string(),
            is_input: true,
        });
        app.doc.is_dirty = true;
    }
    if add_output_port {
        let n = app.doc.program.io_mappings.iter().filter(|m| !m.is_input).count();
        app.doc.program.io_mappings.push(my_canvas_core::IOPortMapping {
            plc_address: format!("Y{}", n),
            iec_address: format!("%QX{}.{}", n / 8, n % 8),
            symbol: format!("OUT_{}", n),
            hw_pin: format!("D{}", n + 8),
            description: "Digital Output".to_string(),
            is_input: false,
        });
        app.doc.is_dirty = true;
    }
    if let Some(var) = toggle_port_var {
        app.doc.toggle_input(&var);
    }
}
