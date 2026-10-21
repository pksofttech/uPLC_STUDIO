use egui::{Color32, CornerRadius, RichText, Stroke, Ui, Vec2};
use crate::app::{CanvasApp, WatchDisplayFormat};
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
                    RichText::new(prop_label).size(11.0).strong().color(tab0_col),
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
                    RichText::new(format!("📋 IEC ({})", tags_count))
                        .size(11.0)
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
                        .size(11.0)
                        .strong()
                        .color(tab2_col),
                );
                if t2.clicked() {
                    app.right_panel_tab = 2;
                }

                let is_tab3 = app.right_panel_tab == 3;
                let tab3_col = if is_tab3 {
                    Color32::WHITE
                } else {
                    palette.rung_text_dim
                };
                let t3 = ui.selectable_label(
                    is_tab3,
                    RichText::new("📟 Special")
                        .size(11.0)
                        .strong()
                        .color(tab3_col),
                ).on_hover_text("Special System Markers (%SM / M8000) & Special Registers (%SD / D8000) references");
                if t3.clicked() {
                    app.right_panel_tab = 3;
                }

                let watch_count = app.watch_rows.len();
                let is_tab4 = app.right_panel_tab == 4;
                let tab4_col = if is_tab4 {
                    Color32::WHITE
                } else {
                    palette.rung_text_dim
                };
                let t4 = ui.selectable_label(
                    is_tab4,
                    RichText::new(format!("📊 Watch ({})", watch_count))
                        .size(11.0)
                        .strong()
                        .color(tab4_col),
                ).on_hover_text("Real-time Data Watch Table for monitoring and modifying PLC variables");
                if t4.clicked() {
                    app.right_panel_tab = 4;
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
        2 => render_io_ports(ui, app, palette, sim_active),
        3 => render_special_registers(ui, app, palette, sim_active),
        4 => render_watch_table(ui, app, palette, sim_active),
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
    let initial_elem = app.doc.selected_element().cloned();
    let initial_comment = app.doc.selected_rung_idx
        .and_then(|i| app.doc.program.rungs.get(i))
        .map(|r| r.comment.clone());

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
                            TypeTag::Contact | TypeTag::RTrig | TypeTag::FTrig => (palette.accent_heading, "CONTACT"),
                            TypeTag::Coil => (palette.accent_ok, "COIL"),
                            TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => (palette.accent_warn, "TIMER"),
                            TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => (palette.accent_warn, "COUNTER"),
                            TypeTag::Sr | TypeTag::Rs => (palette.accent_warn, "BISTABLE FB"),
                            TypeTag::Jump | TypeTag::Label => (palette.accent_heading, "FLOW"),
                            TypeTag::Compare | TypeTag::Math | TypeTag::Move | TypeTag::Bitwise => (func_col, "FUNCTION"),
                            TypeTag::Pid | TypeTag::Scale | TypeTag::Limit | TypeTag::Adc => (palette.accent_ok, "PROCESS FB"),
                            TypeTag::ShiftRegisters
                            | TypeTag::ShiftRegInbit
                            | TypeTag::ShiftRegReset
                            | TypeTag::ShiftLeft
                            | TypeTag::ShiftRight
                            | TypeTag::ShiftVariable => (palette.accent_warn, "SHIFT FB"),
                            TypeTag::UartSend | TypeTag::UartRecv => (palette.accent_heading, "SERIAL FB"),
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
                        TypeTag::Adc => {
                            ui.label(RichText::new("A/D Converter Read (ADC)").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();
                            egui::Grid::new("insp_adc_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("ADC Channel:").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.name);
                                    ui.end_row();
                                    ui.label(RichText::new("Dest Reg (OUT):").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.target_reg);
                                    ui.end_row();
                                });
                            if sim_active {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("Live ADC Value: {} (0..4095)", elem.current_accum))
                                        .monospace()
                                        .strong()
                                        .color(palette.accent_ok),
                                );
                            }
                        }
                        TypeTag::Pid => {
                            ui.label(RichText::new("PID Controller (PID)").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();
                            egui::Grid::new("insp_pid_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("PV Input:").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.fb.in1);
                                    ui.end_row();
                                    ui.label(RichText::new("Setpoint (SP):").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.fb.in2);
                                    ui.end_row();
                                    ui.label(RichText::new("Output (OUT):").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.target_reg);
                                    ui.end_row();
                                    ui.label(RichText::new("Kp (×100):").color(palette.text_primary));
                                    ui.add(egui::DragValue::new(&mut elem.fb.k[0]).range(0..=100_000));
                                    ui.end_row();
                                    ui.label(RichText::new("Ki (×100):").color(palette.text_primary));
                                    ui.add(egui::DragValue::new(&mut elem.fb.k[1]).range(0..=100_000));
                                    ui.end_row();
                                    ui.label(RichText::new("Kd (×100):").color(palette.text_primary));
                                    ui.add(egui::DragValue::new(&mut elem.fb.k[2]).range(0..=100_000));
                                    ui.end_row();
                                    ui.label(RichText::new("Out Max:").color(palette.text_primary));
                                    ui.add(egui::DragValue::new(&mut elem.fb.k[3]).range(1..=100_000));
                                    ui.end_row();
                                });
                            if sim_active {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("Live PID OUT: {}", elem.current_accum))
                                        .monospace()
                                        .strong()
                                        .color(palette.accent_ok),
                                );
                            }
                        }
                        TypeTag::Scale => {
                            ui.label(RichText::new("Linear Scale (SCALE)").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();
                            egui::Grid::new("insp_scale_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Input (IN):").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.fb.in1);
                                    ui.end_row();
                                    ui.label(RichText::new("Output (OUT):").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.target_reg);
                                    ui.end_row();
                                    ui.label(RichText::new("In Min..Max:").color(palette.text_primary));
                                    ui.horizontal(|ui| {
                                        ui.add(egui::DragValue::new(&mut elem.fb.k[0]));
                                        ui.add(egui::DragValue::new(&mut elem.fb.k[1]));
                                    });
                                    ui.end_row();
                                    ui.label(RichText::new("Out Min..Max:").color(palette.text_primary));
                                    ui.horizontal(|ui| {
                                        ui.add(egui::DragValue::new(&mut elem.fb.k[2]));
                                        ui.add(egui::DragValue::new(&mut elem.fb.k[3]));
                                    });
                                    ui.end_row();
                                });
                            if sim_active {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("Live Scaled OUT: {}", elem.current_accum))
                                        .monospace()
                                        .strong()
                                        .color(palette.accent_ok),
                                );
                            }
                        }
                        TypeTag::Limit => {
                            ui.label(RichText::new("Clamp Limit (LIMIT)").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();
                            egui::Grid::new("insp_limit_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Input (IN):").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.fb.in1);
                                    ui.end_row();
                                    ui.label(RichText::new("Output (OUT):").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.target_reg);
                                    ui.end_row();
                                    ui.label(RichText::new("Min..Max:").color(palette.text_primary));
                                    ui.horizontal(|ui| {
                                        ui.add(egui::DragValue::new(&mut elem.fb.k[0]));
                                        ui.add(egui::DragValue::new(&mut elem.fb.k[1]));
                                    });
                                    ui.end_row();
                                });
                            if sim_active {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("Live Clamped OUT: {}", elem.current_accum))
                                        .monospace()
                                        .strong()
                                        .color(palette.accent_ok),
                                );
                            }
                        }
                        TypeTag::Bitwise => {
                            ui.label(RichText::new("Bitwise Logic").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();
                            egui::Grid::new("insp_bitwise_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Operator:").color(palette.text_primary));
                                    egui::ComboBox::from_id_salt("insp_bitwise_op")
                                        .width(70.0)
                                        .selected_text(&elem.math_op)
                                        .show_ui(ui, |ui| {
                                            for op in ["AND", "OR", "XOR", "NOT"] {
                                                ui.selectable_value(&mut elem.math_op, op.to_string(), op);
                                            }
                                        });
                                    ui.end_row();
                                    ui.label(RichText::new("IN1:").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.fb.in1);
                                    ui.end_row();
                                    if !elem.math_op.eq_ignore_ascii_case("NOT") {
                                        ui.label(RichText::new("IN2 / Mask:").color(palette.text_primary));
                                        ui.text_edit_singleline(&mut elem.fb.in2);
                                        ui.end_row();
                                    }
                                    ui.label(RichText::new("OUT:").color(palette.text_primary));
                                    if ui.text_edit_singleline(&mut elem.target_reg).changed() {
                                        elem.name = elem.target_reg.clone();
                                    }
                                    ui.end_row();
                                });
                            if sim_active {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("Live OUT: 0x{:04X} ({})", elem.current_accum as u16, elem.current_accum))
                                        .monospace()
                                        .strong()
                                        .color(palette.accent_ok),
                                );
                            }
                        }
                        TypeTag::ShiftRegisters
                        | TypeTag::ShiftRegInbit
                        | TypeTag::ShiftRegReset
                        | TypeTag::ShiftLeft
                        | TypeTag::ShiftRight
                        | TypeTag::ShiftVariable => {
                            ui.label(RichText::new(elem.element_type.display_name()).strong().size(11.5).color(palette.accent_heading));
                            ui.separator();
                            egui::Grid::new("insp_shift_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Target Word:").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.name);
                                    ui.end_row();
                                    match elem.element_type {
                                        TypeTag::ShiftRegisters => {
                                            ui.label(RichText::new("Stages (LEN):").color(palette.text_primary));
                                            ui.add(egui::DragValue::new(&mut elem.preset_value).range(1..=16));
                                            ui.end_row();
                                            ui.label(RichText::new("Source (IN):").color(palette.text_primary));
                                            ui.text_edit_singleline(&mut elem.fb.in1);
                                            ui.end_row();
                                        }
                                        TypeTag::ShiftRegInbit => {
                                            ui.label(RichText::new("Bit In:").color(palette.text_primary));
                                            ui.text_edit_singleline(&mut elem.fb.in1);
                                            ui.end_row();
                                        }
                                        TypeTag::ShiftLeft | TypeTag::ShiftRight => {
                                            ui.label(RichText::new("Bits (0..15):").color(palette.text_primary));
                                            ui.add(egui::DragValue::new(&mut elem.preset_value).range(0..=15));
                                            ui.end_row();
                                        }
                                        TypeTag::ShiftVariable => {
                                            ui.label(RichText::new("Count Reg:").color(palette.text_primary));
                                            ui.text_edit_singleline(&mut elem.fb.in1);
                                            ui.end_row();
                                        }
                                        _ => {}
                                    }
                                });
                            if sim_active {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("Live Value: 0x{:04X} ({})", elem.current_accum as u16, elem.current_accum))
                                        .monospace()
                                        .strong()
                                        .color(palette.accent_ok),
                                );
                            }
                        }
                        TypeTag::UartSend => {
                            ui.label(RichText::new("UART Send (TX)").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();
                            egui::Grid::new("insp_uart_tx_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Byte Source:").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.fb.in1);
                                    ui.end_row();
                                    ui.label(RichText::new("CRLF:").color(palette.text_primary));
                                    let mut crlf = elem.fb.mode != 0;
                                    if ui.checkbox(&mut crlf, "Append \\r\\n").changed() {
                                        elem.fb.mode = u8::from(crlf);
                                    }
                                    ui.end_row();
                                });
                        }
                        TypeTag::UartRecv => {
                            ui.label(RichText::new("UART Receive (RX)").strong().size(11.5).color(palette.accent_heading));
                            ui.separator();
                            egui::Grid::new("insp_uart_rx_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Dest Reg (OUT):").color(palette.text_primary));
                                    ui.text_edit_singleline(&mut elem.target_reg);
                                    ui.end_row();
                                });
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
                    app.doc.is_dirty = true;
                }
            }
            if let Some(cm) = new_coil_mode {
                app.doc.push_undo_snapshot();
                if let Some(elem) = app.doc.selected_element_mut() {
                    elem.coil_mode = cm;
                    app.doc.is_dirty = true;
                }
            }
            if let Some((addr, sym)) = quick_assign {
                app.doc.push_undo_snapshot();
                if let Some(elem) = app.doc.selected_element_mut() {
                    elem.name = addr.clone();
                    elem.symbol = sym.clone();
                    app.doc.is_dirty = true;
                }
                app.doc.set_global_symbol(&addr, &sym, None);
            }
            if let Some(val) = set_preset {
                app.doc.push_undo_snapshot();
                if let Some(elem) = app.doc.selected_element_mut() {
                    elem.preset_value = val;
                    app.doc.is_dirty = true;
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

            // Detect any inline DragValue / TextEdit changes to active element or network comment
            if let Some(ref init_el) = initial_elem {
                let sym_diff = if let Some(curr_el) = app.doc.selected_element() {
                    if curr_el != init_el {
                        let diff = if curr_el.symbol != init_el.symbol || curr_el.name != init_el.name {
                            Some((curr_el.name.clone(), curr_el.symbol.clone(), curr_el.comment.clone()))
                        } else {
                            None
                        };
                        Some(diff)
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(diff) = sym_diff {
                    app.doc.is_dirty = true;
                    if let Some((c_name, c_sym, c_rem)) = diff {
                        app.doc.set_global_symbol(&c_name, &c_sym, Some(&c_rem));
                    }
                }
            }
            if let Some(ref init_c) = initial_comment {
                if let Some(r_idx) = app.doc.selected_rung_idx {
                    if let Some(rung) = app.doc.program.rungs.get(r_idx) {
                        if &rung.comment != init_c {
                            app.doc.is_dirty = true;
                        }
                    }
                }
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
                            TypeTag::Compare
                            | TypeTag::Move
                            | TypeTag::Math
                            | TypeTag::Adc
                            | TypeTag::Bitwise
                            | TypeTag::ShiftRegisters
                            | TypeTag::ShiftRegInbit
                            | TypeTag::ShiftRegReset
                            | TypeTag::ShiftLeft
                            | TypeTag::ShiftRight
                            | TypeTag::ShiftVariable
                                if sim_active =>
                            {
                                let reg_v = app.doc.simulator.read_word(raw_name);
                                (format!("={} ({})", reg_v, val_str), palette.accent_heading)
                            }
                            TypeTag::Pid | TypeTag::Scale | TypeTag::Limit if sim_active => {
                                (val_str.clone(), palette.accent_ok)
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
fn render_io_ports(ui: &mut Ui, app: &mut CanvasApp, palette: ThemePalette, _sim_active: bool) {
    let mut toggle_port_var: Option<String> = None;
    let mut add_input_port = false;
    let mut add_output_port = false;
    let mut clear_all_forces = false;
    let mut force_action: Option<(u8, Option<bool>)> = None;
    let mut write_word_val: Option<(u8, i32)> = None;

    egui::ScrollArea::vertical()
        .id_salt("io_ports_monitor_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Active Forces Warning Banner
            if !app.doc.simulator.forced_bits.is_empty() {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(230, 126, 34, 30))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(230, 126, 34)))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "⚡ ACTIVE FORCES: {} override(s)",
                                    app.doc.simulator.forced_bits.len()
                                ))
                                .strong()
                                .size(11.0)
                                .color(Color32::from_rgb(230, 126, 34)),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let rel_btn = egui::Button::new(
                                    RichText::new("✖ Release All Forces")
                                        .size(10.5)
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(192, 57, 43));
                                if ui.add(rel_btn).clicked() {
                                    clear_all_forces = true;
                                }
                            });
                        });
                    });
                ui.add_space(3.0);
            }

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

                let mut input_sym_changed: Option<(String, String, String, String)> = None;

                egui::Grid::new("input_ports_grid")
                    .striped(true)
                    .num_columns(4)
                    .spacing([5.0, 5.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("STATUS").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("FORCE").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("IEC").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("SYMBOL").strong().small().color(palette.rung_text_dim));
                        ui.end_row();

                        for (idx, io) in app.doc.program.io_mappings.iter_mut().filter(|m| m.is_input).enumerate() {
                            if io.iec_address.is_empty() {
                                io.iec_address = io.iec_address(idx);
                            }
                            let iec_addr = io.iec_address.clone();
                            let vm_addr = idx as u8;
                            let is_forced = app.doc.simulator.is_bit_forced(vm_addr);
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
                            .min_size(Vec2::new(44.0, 20.0))
                            .fill(btn_bg);

                            if ui
                                .add(btn)
                                .on_hover_text(format!("{} [{}] (Pin {}) — Click to toggle ON/OFF", io.symbol, iec_addr, io.hw_pin))
                                .clicked()
                            {
                                toggle_port_var = Some(iec_addr.clone());
                            }

                            // FORCE Column
                            if let Some(forced_val) = is_forced {
                                let f_btn = egui::Button::new(
                                    RichText::new(if forced_val { "⚡ F: 1" } else { "⚡ F: 0" })
                                        .size(10.5)
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(211, 84, 0))
                                .min_size(Vec2::new(42.0, 20.0));
                                if ui
                                    .add(f_btn)
                                    .on_hover_text("Bit is FORCED. Click to RELEASE force override.")
                                    .clicked()
                                {
                                    force_action = Some((vm_addr, None));
                                }
                            } else {
                                ui.menu_button("⚡", |ui| {
                                    if ui.button("⚡ Force ON (1)").clicked() {
                                        force_action = Some((vm_addr, Some(true)));
                                        ui.close();
                                    }
                                    if ui.button("⚡ Force OFF (0)").clicked() {
                                        force_action = Some((vm_addr, Some(false)));
                                        ui.close();
                                    }
                                })
                                .response
                                .on_hover_text("Force override Input ON/OFF");
                            }

                            if ui
                                .add_sized(
                                    egui::vec2(78.0, 20.0),
                                    egui::TextEdit::singleline(&mut io.iec_address)
                                        .font(egui::TextStyle::Monospace)
                                        .text_color(palette.accent_warn)
                                        .desired_width(78.0),
                                )
                                .on_hover_text(format!("IEC 61131-3 Input Address (Editable, e.g. %IX0.0, %IX1.0) | Pin: {}", io.hw_pin))
                                .changed()
                            {
                                app.doc.is_dirty = true;
                            }

                            let sym_w = (ui.available_width() - 4.0).max(70.0);
                            if ui.add(egui::TextEdit::singleline(&mut io.symbol).desired_width(sym_w)).changed() {
                                app.doc.is_dirty = true;
                                input_sym_changed = Some((io.iec_address.clone(), io.plc_address.clone(), io.symbol.clone(), io.description.clone()));
                            }
                            ui.end_row();
                        }
                    });

                if let Some((iec_addr, plc_addr, new_sym, desc)) = input_sym_changed {
                    app.doc.set_global_symbol(&iec_addr, &new_sym, Some(&desc));
                    if !plc_addr.is_empty() {
                        app.doc.set_global_symbol(&plc_addr, &new_sym, Some(&desc));
                    }
                }
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

                let mut output_sym_changed: Option<(String, String, String, String)> = None;

                egui::Grid::new("output_ports_grid")
                    .striped(true)
                    .num_columns(4)
                    .spacing([5.0, 5.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("LED").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("FORCE").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("IEC").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("SYMBOL").strong().small().color(palette.rung_text_dim));
                        ui.end_row();

                        for (idx, io) in app.doc.program.io_mappings.iter_mut().filter(|m| !m.is_input).enumerate() {
                            if io.iec_address.is_empty() {
                                io.iec_address = io.iec_address(idx);
                            }
                            let iec_addr = io.iec_address.clone();
                            let vm_addr = 0x20 + (idx as u8);
                            let is_forced = app.doc.simulator.is_bit_forced(vm_addr);
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

                            // FORCE Column
                            if let Some(forced_val) = is_forced {
                                let f_btn = egui::Button::new(
                                    RichText::new(if forced_val { "⚡ F: 1" } else { "⚡ F: 0" })
                                        .size(10.5)
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(211, 84, 0))
                                .min_size(Vec2::new(42.0, 20.0));
                                if ui
                                    .add(f_btn)
                                    .on_hover_text("Output is FORCED. Click to RELEASE force override.")
                                    .clicked()
                                {
                                    force_action = Some((vm_addr, None));
                                }
                            } else {
                                ui.menu_button("⚡", |ui| {
                                    if ui.button("⚡ Force ON (1)").clicked() {
                                        force_action = Some((vm_addr, Some(true)));
                                        ui.close();
                                    }
                                    if ui.button("⚡ Force OFF (0)").clicked() {
                                        force_action = Some((vm_addr, Some(false)));
                                        ui.close();
                                    }
                                })
                                .response
                                .on_hover_text("Force override Output ON/OFF");
                            }

                            if ui
                                 .add_sized(
                                     egui::vec2(78.0, 20.0),
                                     egui::TextEdit::singleline(&mut io.iec_address)
                                         .font(egui::TextStyle::Monospace)
                                         .text_color(palette.accent_ok)
                                         .desired_width(78.0),
                                 )
                                 .on_hover_text(format!("IEC 61131-3 Output Address (Editable, e.g. %QX0.0, %QX1.0) | Pin: {}", io.hw_pin))
                                 .changed()
                            {
                                app.doc.is_dirty = true;
                            }

                            let sym_w = (ui.available_width() - 4.0).max(70.0);
                            if ui.add(egui::TextEdit::singleline(&mut io.symbol).desired_width(sym_w)).changed() {
                                app.doc.is_dirty = true;
                                output_sym_changed = Some((io.iec_address.clone(), io.plc_address.clone(), io.symbol.clone(), io.description.clone()));
                            }
                            ui.end_row();
                        }
                    });

                if let Some((iec_addr, plc_addr, new_sym, desc)) = output_sym_changed {
                    app.doc.set_global_symbol(&iec_addr, &new_sym, Some(&desc));
                    if !plc_addr.is_empty() {
                        app.doc.set_global_symbol(&plc_addr, &new_sym, Some(&desc));
                    }
                }
            });

            ui.add_space(4.0);

            // Analog Inputs Group (%IW0..%IW3 / ADC0..ADC2 + Internal Temp)
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("🎚 Analog Inputs (%IW0..%IW3)")
                            .strong()
                            .color(palette.accent_heading),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("12-bit (0..4095) / 0..3.3V")
                                .size(10.0)
                                .color(palette.rung_text_dim),
                        );
                    });
                });
                ui.separator();

                const ANALOG_CHANNELS: &[(&str, &str, &str)] = &[
                    ("%IW0", "AI0", "GP26 (ADC0)"),
                    ("%IW1", "AI1", "GP27 (ADC1)"),
                    ("%IW2", "AI2", "GP28 (ADC2)"),
                    ("%IW3", "AI3", "RP2040 Temp Sensor"),
                ];

                egui::Grid::new("analog_inputs_grid")
                    .striped(true)
                    .num_columns(4)
                    .spacing([6.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("CHAN").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("RAW").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("VOLTS / TEMP").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("SLIDER / LEVEL").strong().small().color(palette.rung_text_dim));
                        ui.end_row();

                        for (idx, &(iec_addr, canon_name, hw_desc)) in ANALOG_CHANNELS.iter().enumerate() {
                            let val = app.doc.simulator.read_word(iec_addr);

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(iec_addr)
                                        .monospace()
                                        .strong()
                                        .size(11.0)
                                        .color(palette.accent_heading),
                                );
                            })
                            .response
                            .on_hover_text(format!("{} [{}] — {}", canon_name, iec_addr, hw_desc));

                            // In local / simulator mode (not live hardware online), allow adjusting via DragValue + Slider
                            let sim_allow_drag = !(app.online_mode && app.online_target_hw);
                            if sim_allow_drag {
                                let mut edit_raw = val.clamp(0, 4095);
                                if ui.add(egui::DragValue::new(&mut edit_raw).range(0..=4095).speed(5.0)).changed() {
                                    app.doc.simulator.write_word(iec_addr, edit_raw);
                                }
                            } else {
                                ui.label(
                                    RichText::new(format!("{}", val))
                                        .monospace()
                                        .size(11.0)
                                        .color(palette.text_primary),
                                );
                            }

                            // Voltage / Temperature display
                            let volt = (val.clamp(0, 4095) as f32 * 3.3) / 4095.0;
                            if idx == 3 {
                                let temp_c01 = app
                                    .doc
                                    .simulator
                                    .registers
                                    .get("%SD6")
                                    .copied()
                                    .unwrap_or_else(|| {
                                        let vbe_uv = (val.clamp(0, 4095) as u32 * 3_300_000) / 4096;
                                        270i32 - (((vbe_uv as i32) - 706_000) / 1721)
                                    });
                                ui.label(
                                    RichText::new(format!("{:.2}V | {:.1}°C", volt, (temp_c01 as f32) / 10.0))
                                        .monospace()
                                        .size(10.5)
                                        .color(palette.accent_ok),
                                );
                            } else {
                                ui.label(
                                    RichText::new(format!("{:.2} V", volt))
                                        .monospace()
                                        .size(10.5)
                                        .color(palette.accent_ok),
                                );
                            }

                            // Interactive Slider in Simulator mode, or Level Bar in Hardware Online mode
                            if sim_allow_drag {
                                let mut edit_raw = val.clamp(0, 4095);
                                let slider_w = (ui.available_width() - 8.0).max(60.0);
                                ui.spacing_mut().slider_width = slider_w;
                                if ui
                                    .add(
                                        egui::Slider::new(&mut edit_raw, 0..=4095)
                                            .show_value(false),
                                    )
                                    .on_hover_text("Drag to simulate 12-bit ADC input (0..4095)")
                                    .changed()
                                {
                                    app.doc.simulator.write_word(iec_addr, edit_raw);
                                }
                            } else {
                                let frac = (val.clamp(0, 4095) as f32 / 4095.0).clamp(0.0, 1.0);
                                let bar_w = (ui.available_width() - 8.0).max(60.0);
                                ui.add_sized(
                                    egui::vec2(bar_w, 14.0),
                                    egui::ProgressBar::new(frac)
                                        .text(format!("{:.0}%", frac * 100.0)),
                                );
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
                        let is_matched = app.online_program_match_info().map_or(true, |(m, _, _, _, _)| m);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if app.online_target_hw && !is_matched {
                                ui.label(
                                    RichText::new("⚠ MISMATCH (PAUSED)")
                                        .size(10.0)
                                        .strong()
                                        .color(palette.accent_warn),
                                );
                            } else {
                                ui.label(
                                    RichText::new("🟢 LIVE")
                                        .size(10.0)
                                        .strong()
                                        .color(palette.accent_ok),
                                );
                            }
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
                        let m_addr = 0x40 + (i as u8);
                        let m_forced = app.doc.simulator.is_bit_forced(m_addr);
                        let m_on = app.doc.simulator.get_var_value(&addr);

                        let (col, txt) = if let Some(f_val) = m_forced {
                            (
                                Color32::from_rgb(230, 126, 34),
                                format!("M{}:{}⚡F:{}", i, u8::from(m_on), if f_val { 1 } else { 0 }),
                            )
                        } else {
                            (
                                if m_on { palette.accent_ok } else { palette.rung_text_dim },
                                format!("M{}:{}", i, u8::from(m_on)),
                            )
                        };

                        let resp = ui.selectable_label(m_on, RichText::new(txt).monospace().size(10.5).color(col));
                        if resp.clicked() {
                            toggle_port_var = Some(addr.clone());
                        }
                        resp.context_menu(|ui| {
                            ui.label(RichText::new(format!("Marker %MX0.{} (M{})", i, i)).strong());
                            if ui.button("⚡ Force ON (1)").clicked() {
                                force_action = Some((m_addr, Some(true)));
                                ui.close();
                            }
                            if ui.button("⚡ Force OFF (0)").clicked() {
                                force_action = Some((m_addr, Some(false)));
                                ui.close();
                            }
                            if m_forced.is_some() {
                                if ui.button("✖ Release Force").clicked() {
                                    force_action = Some((m_addr, None));
                                    ui.close();
                                }
                            }
                        });
                    }
                });

                ui.add_space(3.0);

                // 3. Timers (T0..T3) & Counters (C0..C3) & Word Registers (%MW0..%MW3)
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

                            ui.menu_button(
                                RichText::new(format!("%MW{}: {}", i, mw_val))
                                    .monospace()
                                    .size(10.5)
                                    .color(if mw_val != 0 {
                                        palette.accent_heading
                                    } else {
                                        palette.text_primary
                                    }),
                                |ui| {
                                    ui.label(RichText::new(format!("Direct Write to %MW{} (D{})", i, i)).strong());
                                    let mut edit_val = mw_val;
                                    ui.horizontal(|ui| {
                                        ui.add(egui::DragValue::new(&mut edit_val).speed(1.0));
                                        if ui.button("✍ Write").clicked() {
                                            write_word_val = Some((i as u8, edit_val));
                                            ui.close();
                                        }
                                    });
                                },
                            )
                            .response
                            .on_hover_text(format!("%MW{} (D{}) — Click to direct write 32-bit register value", i, i));

                            ui.end_row();
                        }
                    });
            });

            // UART Terminal Card (Simulator TX / RX)
            ui.add_space(6.0);
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("📡 UART Terminal")
                            .strong()
                            .color(palette.accent_heading),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("🗑 Clear").on_hover_text("Clear UART TX log").clicked() {
                            app.uart_tx_history.clear();
                            app.doc.simulator.uart_tx_log.clear();
                        }
                        ui.checkbox(&mut app.uart_hex_view, "Hex");
                        let rx_q = app.doc.simulator.uart_rx_queue.len();
                        ui.label(
                            RichText::new(format!("RX FIFO: {}B", rx_q))
                                .monospace()
                                .size(10.0)
                                .color(if rx_q > 0 { palette.accent_ok } else { palette.rung_text_dim }),
                        );
                    });
                });
                ui.separator();

                let mut tx_display = if app.uart_tx_history.is_empty() {
                    String::from("(No UART TX bytes transmitted yet)")
                } else if app.uart_hex_view {
                    app.uart_tx_history
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    let mut s = String::with_capacity(app.uart_tx_history.len());
                    for &b in &app.uart_tx_history {
                        if b == b'\r' {
                            continue;
                        } else if b == b'\n' || b == b'\t' || (0x20..=0x7E).contains(&b) {
                            s.push(b as char);
                        } else {
                            s.push_str(&format!("\\x{:02X}", b));
                        }
                    }
                    s
                };

                ui.add(
                    egui::TextEdit::multiline(&mut tx_display)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(ui.available_width())
                        .desired_rows(3)
                        .interactive(false),
                );

                ui.add_space(3.0);
                let mut do_send_rx = false;
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("uart_rx_mode_combo")
                        .width(88.0)
                        .selected_text(match app.uart_rx_mode {
                            0 => "Text+CRLF",
                            1 => "Raw Text",
                            _ => "Hex Bytes",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.uart_rx_mode, 0, "Text+CRLF (\\r\\n)");
                            ui.selectable_value(&mut app.uart_rx_mode, 1, "Raw Text");
                            ui.selectable_value(&mut app.uart_rx_mode, 2, "Hex Bytes (e.g. 41 0A)");
                        });

                    let input_w = (ui.available_width() - 68.0).max(60.0);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut app.uart_rx_input)
                            .font(egui::TextStyle::Monospace)
                            .hint_text(if app.uart_rx_mode == 2 {
                                "e.g. 41 42 0D 0A"
                            } else {
                                "Type RX data..."
                            })
                            .desired_width(input_w),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        do_send_rx = true;
                    }
                    if ui
                        .button(RichText::new("Send RX").small().strong().color(palette.accent_ok))
                        .on_hover_text("Push bytes into simulator UART RX queue for UartRecv blocks")
                        .clicked()
                    {
                        do_send_rx = true;
                    }
                });

                if do_send_rx && !app.uart_rx_input.is_empty() {
                    let bytes: Vec<u8> = match app.uart_rx_mode {
                        0 => {
                            let mut v = app.uart_rx_input.as_bytes().to_vec();
                            v.extend_from_slice(b"\r\n");
                            v
                        }
                        1 => app.uart_rx_input.as_bytes().to_vec(),
                        _ => app
                            .uart_rx_input
                            .split(|c: char| c.is_whitespace() || c == ',')
                            .filter(|s| !s.is_empty())
                            .filter_map(|tok| {
                                let clean = tok
                                    .trim_start_matches("0x")
                                    .trim_start_matches("0X")
                                    .trim_start_matches("16#");
                                u8::from_str_radix(clean, 16)
                                    .ok()
                                    .or_else(|| tok.parse::<u8>().ok())
                            })
                            .collect(),
                    };
                    if !bytes.is_empty() {
                        app.doc.simulator.push_uart_rx(&bytes);
                        app.uart_rx_input.clear();
                    }
                }
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
    if clear_all_forces {
        app.clear_all_forces();
    }
    if let Some((addr, maybe_val)) = force_action {
        if let Some(val) = maybe_val {
            app.force_bit(addr, val);
        } else {
            app.unforce_bit(addr);
        }
    }
    if let Some((addr, val)) = write_word_val {
        app.write_word_register(addr, val);
    }
    if let Some(var) = toggle_port_var {
        app.doc.toggle_input(&var);
    }
}

// =============================================================================
// TAB 3: SPECIAL REGISTERS & SYSTEM MARKERS REFERENCES (%SM / %SD)
// =============================================================================
#[derive(Clone, Copy)]
struct SpecialRegisterDef {
    iec: &'static str,
    legacy: &'static str,
    name: &'static str,
    category_id: u8, // 1: Flags, 2: Clocks, 3: Diagnostics, 4: Registers, 5: Latch
    desc: &'static str,
    is_bit: bool,
    default_hint: &'static str,
}

const SPECIAL_REGISTERS: &[SpecialRegisterDef] = &[
    // Flags (%SM0..%SM2)
    SpecialRegisterDef {
        iec: "%SM0",
        legacy: "M8000",
        name: "Always ON",
        category_id: 1,
        desc: "Continuously energized contact (PLC RUN status monitor / power rail)",
        is_bit: true,
        default_hint: "TRUE (1)",
    },
    SpecialRegisterDef {
        iec: "%SM1",
        legacy: "M8001",
        name: "Always OFF",
        category_id: 1,
        desc: "Continuously de-energized contact (Inverted RUN / disconnect rung)",
        is_bit: true,
        default_hint: "FALSE (0)",
    },
    SpecialRegisterDef {
        iec: "%SM2",
        legacy: "M8002",
        name: "First Scan Pulse",
        category_id: 1,
        desc: "Initial pulse: ON for exactly 1 scan cycle on STOP->RUN transition",
        is_bit: true,
        default_hint: "Pulse (Scan #1)",
    },
    // Clock Pulses (%SM10..%SM13)
    SpecialRegisterDef {
        iec: "%SM10",
        legacy: "M8011",
        name: "10ms Clock Pulse",
        category_id: 2,
        desc: "10ms period oscillator (5ms ON / 5ms OFF, 100 Hz square wave)",
        is_bit: true,
        default_hint: "100 Hz Clock",
    },
    SpecialRegisterDef {
        iec: "%SM11",
        legacy: "M8012",
        name: "100ms Clock Pulse",
        category_id: 2,
        desc: "100ms period oscillator (50ms ON / 50ms OFF, 10 Hz square wave)",
        is_bit: true,
        default_hint: "10 Hz Clock",
    },
    SpecialRegisterDef {
        iec: "%SM12",
        legacy: "M8013",
        name: "1s Clock Pulse",
        category_id: 2,
        desc: "1s period blinker (0.5s ON / 0.5s OFF, 1 Hz) for alarms & indicators",
        is_bit: true,
        default_hint: "1 Hz Blinker",
    },
    SpecialRegisterDef {
        iec: "%SM13",
        legacy: "M8014",
        name: "1min Clock Pulse",
        category_id: 2,
        desc: "1 minute period clock (30s ON / 30s OFF) for logging & telemetry",
        is_bit: true,
        default_hint: "1 min Clock",
    },
    // Diagnostics (%SM20..%SM21)
    SpecialRegisterDef {
        iec: "%SM20",
        legacy: "M8004",
        name: "System Error",
        category_id: 3,
        desc: "Hardware or execution runtime error flag",
        is_bit: true,
        default_hint: "0 (OK)",
    },
    SpecialRegisterDef {
        iec: "%SM21",
        legacy: "M8020",
        name: "Scan Overrun",
        category_id: 3,
        desc: "Scan cycle execution time exceeded Watchdog timeout preset",
        is_bit: true,
        default_hint: "0 (OK)",
    },
    // Special Registers (%SD0..%SD5)
    SpecialRegisterDef {
        iec: "%SD0",
        legacy: "D8000",
        name: "Watchdog Preset",
        category_id: 4,
        desc: "Max scan execution duration in ms before Watchdog trip (Default: 100 ms)",
        is_bit: false,
        default_hint: "100 ms",
    },
    SpecialRegisterDef {
        iec: "%SD1",
        legacy: "D8010",
        name: "Current Scan Time",
        category_id: 4,
        desc: "Execution duration of the most recent PLC scan cycle in microseconds",
        is_bit: false,
        default_hint: "Dynamic (µs)",
    },
    SpecialRegisterDef {
        iec: "%SD2",
        legacy: "D8011",
        name: "Min Scan Time",
        category_id: 4,
        desc: "Minimum recorded scan cycle execution time in microseconds",
        is_bit: false,
        default_hint: "Dynamic (µs)",
    },
    SpecialRegisterDef {
        iec: "%SD3",
        legacy: "D8012",
        name: "Max Scan Time",
        category_id: 4,
        desc: "Peak recorded scan cycle execution time in microseconds",
        is_bit: false,
        default_hint: "Dynamic (µs)",
    },
    SpecialRegisterDef {
        iec: "%SD4",
        legacy: "D8004",
        name: "Scan Counter (Low)",
        category_id: 4,
        desc: "Lower 16 bits of total PLC scan cycles executed since boot (D132)",
        is_bit: false,
        default_hint: "Low Word",
    },
    SpecialRegisterDef {
        iec: "%SD5",
        legacy: "D8005",
        name: "Scan Counter (High)",
        category_id: 4,
        desc: "Upper 16 bits of total PLC scan cycles executed since boot (D133)",
        is_bit: false,
        default_hint: "High Word",
    },
    SpecialRegisterDef {
        iec: "%SD6",
        legacy: "D8006",
        name: "CPU Temperature",
        category_id: 3,
        desc: "RP2040 on-chip junction temperature in 0.1 °C units (e.g. 265 = 26.5 °C)",
        is_bit: false,
        default_hint: "0.1 °C (D134)",
    },
    SpecialRegisterDef {
        iec: "%SD7",
        legacy: "D8007",
        name: "RETAIN Flash Save Count",
        category_id: 5,
        desc: "Number of times the RETAIN area (M64..M95, %MW96..%MW127) has been written to Flash (D135)",
        is_bit: false,
        default_hint: "Saves (D135)",
    },
    // Retentive Memory Ranges
    SpecialRegisterDef {
        iec: "%MX8.0..%MX11.7",
        legacy: "M64..M95",
        name: "Latch Bit Relays",
        category_id: 5,
        desc: "32 RETAIN Bit Relays preserved in RP2040 Flash across power outages",
        is_bit: true,
        default_hint: "Flash Retentive",
    },
    SpecialRegisterDef {
        iec: "%MW96..%MW127",
        legacy: "D96..D127",
        name: "Latch Word Registers",
        category_id: 5,
        desc: "32 RETAIN Word Registers preserved in RP2040 Flash across power outages",
        is_bit: false,
        default_hint: "Flash Retentive",
    },
];

/// Live value of `%SD7` (RETAIN Flash save count) if the simulator / online snapshot provides it.
fn retain_save_count(app: &CanvasApp) -> Option<i32> {
    ["%SD7", "SD7", "D8007", "D135"]
        .iter()
        .find_map(|k| app.doc.simulator.registers.get(*k).copied())
}

/// Info card describing the RETAIN area and showing `SD7` (Flash save count).
fn render_retain_info(ui: &mut Ui, app: &CanvasApp, palette: ThemePalette) {
    egui::Frame::new()
        .fill(palette.accent_ok.gamma_multiply(0.08))
        .stroke(Stroke::new(1.0, palette.accent_ok.gamma_multiply(0.45)))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(egui::Margin::symmetric(6, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔒 RETAIN Memory (Flash)").small().strong().color(palette.accent_ok));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let live = app.online_mode || app.doc.simulator.is_active();
                    let txt = match retain_save_count(app) {
                        Some(v) if live => format!("SD7 = {} saves ● LIVE", v),
                        Some(v) => format!("SD7 = {} saves", v),
                        None => "SD7 = — (connect Online)".to_string(),
                    };
                    ui.label(RichText::new(txt).monospace().small().strong().color(palette.accent_heading))
                        .on_hover_text("%SD7 / D8007: number of RETAIN saves written to Flash");
                });
            });
            ui.label(
                RichText::new("Bits M64..M95 (%MX8.0..%MX11.7) and words D96..D127 (%MW96..%MW127)")
                    .small()
                    .monospace()
                    .color(palette.text_primary),
            );
            ui.label(
                RichText::new(
                    "Values survive power loss. The firmware saves the area to Flash automatically when it \
                     changes (at most once every 10 minutes), on STOP, and on \"💾 Save RETAIN to Flash\" \
                     (Update to PLC window). \"🗑 Clear RETAIN\" erases it.",
                )
                .small()
                .color(palette.rung_text_dim),
            );
        });
}

fn render_special_registers(
    ui: &mut Ui,
    app: &mut CanvasApp,
    palette: ThemePalette,
    sim_active: bool,
) {
    let selected_meta = app.doc.selected_element_id.and_then(|id| {
        app.doc.element_by_id(id).map(|el| (id, el.element_type, el.name.clone()))
    });
    let mut assign_address: Option<String> = None;

    egui::ScrollArea::vertical()
        .id_salt("special_registers_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.group(|ui| {
                ui.set_width(ui.available_width());

                // Header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Special Register References")
                            .color(palette.accent_heading)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if sim_active {
                            ui.label(
                                RichText::new("● LIVE")
                                    .color(palette.accent_ok)
                                    .size(10.0)
                                    .strong(),
                            );
                        } else {
                            ui.label(
                                RichText::new("○ IDLE")
                                    .color(palette.rung_text_dim)
                                    .size(10.0),
                            );
                        }
                    });
                });

                ui.label(
                    RichText::new("RP2040 System Markers (%SM) & Special Registers (%SD)")
                        .small()
                        .color(palette.rung_text_dim),
                );

                ui.separator();

                // Selected Element Target Banner
                if let Some((sel_id, sel_type, sel_name)) = &selected_meta {
                    let banner_frame = egui::Frame::new()
                        .fill(palette.accent_heading.gamma_multiply(0.12))
                        .stroke(Stroke::new(1.0, palette.accent_heading.gamma_multiply(0.5)))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(6, 4));

                    banner_frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("Target: #{} [{}]", sel_id, sel_type.display_name()))
                                    .color(palette.accent_heading)
                                    .small()
                                    .strong(),
                            );
                            let cur_str = if sel_name.is_empty() { "-" } else { sel_name.as_str() };
                            ui.label(
                                RichText::new(format!("(\"{}\")", cur_str))
                                    .color(palette.text_primary)
                                    .small()
                                    .monospace(),
                            );
                        });
                        ui.label(
                            RichText::new("Click \"Use\" on any register below to assign directly.")
                                .small()
                                .color(palette.rung_text_dim),
                        );
                    });
                    ui.add_space(2.0);
                }

                // Category Filter Bar
                ui.horizontal_wrapped(|ui| {
                    for (cat_id, lbl) in [
                        (0u8, "All"),
                        (1u8, "Flags"),
                        (2u8, "Clocks"),
                        (3u8, "Diag"),
                        (4u8, "Registers"),
                        (5u8, "Latch"),
                    ] {
                        if ui
                            .selectable_label(app.special_reg_filter_cat == cat_id, RichText::new(lbl).small())
                            .clicked()
                        {
                            app.special_reg_filter_cat = cat_id;
                        }
                    }
                });

                // Search Bar
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut app.special_reg_filter_text)
                            .hint_text("🔍 Filter %SM, M8000, %SD, Scan...")
                            .desired_width(ui.available_width() - 28.0),
                    );
                    if !app.special_reg_filter_text.is_empty() {
                        if ui.small_button("✖").on_hover_text("Clear filter").clicked() {
                            app.special_reg_filter_text.clear();
                        }
                    }
                });
            });

            ui.add_space(4.0);

            // RETAIN (Flash-retained memory) explanation + live SD7 save counter
            render_retain_info(ui, app, palette);

            ui.add_space(4.0);

            // Filter items
            let query = app.special_reg_filter_text.trim().to_uppercase();
            let cat_filter = app.special_reg_filter_cat;

            let filtered_items: Vec<&SpecialRegisterDef> = SPECIAL_REGISTERS
                .iter()
                .filter(|item| {
                    if cat_filter != 0 && item.category_id != cat_filter {
                        return false;
                    }
                    if !query.is_empty() {
                        let match_iec = item.iec.to_uppercase().contains(&query);
                        let match_legacy = item.legacy.to_uppercase().contains(&query);
                        let match_name = item.name.to_uppercase().contains(&query);
                        let match_desc = item.desc.to_uppercase().contains(&query);
                        if !match_iec && !match_legacy && !match_name && !match_desc {
                            return false;
                        }
                    }
                    true
                })
                .collect();

            if filtered_items.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(16.0);
                    ui.label(RichText::new("No special registers match filter").color(palette.rung_text_dim));
                    ui.add_space(16.0);
                });
            } else {
                for item in filtered_items {
                    egui::Frame::new()
                        .fill(if palette.is_light() {
                            Color32::from_rgb(245, 247, 250)
                        } else {
                            palette.card_bg
                        })
                        .stroke(Stroke::new(1.0, palette.card_border.gamma_multiply(0.7)))
                        .corner_radius(CornerRadius::same(5))
                        .inner_margin(egui::Margin::symmetric(6, 5))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());

                            // Row 1: Badges + Name + Live Value
                            ui.horizontal(|ui| {
                                // IEC address badge
                                let iec_color = if item.is_bit {
                                    palette.accent_ok
                                } else {
                                    palette.accent_heading
                                };
                                egui::Frame::new()
                                    .fill(iec_color.gamma_multiply(0.15))
                                    .stroke(Stroke::new(1.0, iec_color.gamma_multiply(0.6)))
                                    .corner_radius(CornerRadius::same(3))
                                    .inner_margin(egui::Margin::symmetric(4, 1))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(item.iec).monospace().small().strong().color(iec_color));
                                    });

                                // Legacy alias badge
                                egui::Frame::new()
                                    .fill(palette.pill_bg)
                                    .stroke(Stroke::new(1.0, palette.card_border.gamma_multiply(0.5)))
                                    .corner_radius(CornerRadius::same(3))
                                    .inner_margin(egui::Margin::symmetric(4, 1))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(item.legacy).monospace().small().color(palette.rung_text_dim));
                                    });

                                ui.label(RichText::new(item.name).strong().small().color(palette.text_primary));

                                // Live Value
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if sim_active {
                                        if item.is_bit && !item.iec.contains("..") {
                                            let is_on = app.doc.simulator.get_var_value(item.iec);
                                            let (badge_txt, badge_col) = if is_on {
                                                ("● ON", palette.accent_ok)
                                            } else {
                                                ("○ OFF", palette.rung_text_dim)
                                            };
                                            ui.label(RichText::new(badge_txt).small().strong().color(badge_col));
                                        } else if !item.is_bit && !item.iec.contains("..") {
                                            let reg_val = app.doc.simulator.registers.get(item.iec).copied()
                                                .or_else(|| app.doc.simulator.registers.get(item.legacy).copied());
                                            if let Some(v) = reg_val {
                                                let formatted = match item.iec {
                                                    "%SD0" => format!("{} ms", v),
                                                    "%SD1" | "%SD2" | "%SD3" => format!("{} µs", v),
                                                    "%SD4" | "%SD5" => format!("{}", v),
                                                    "%SD6" => format!("{:.1} °C", (v as f32) / 10.0),
                                                    "%SD7" => format!("{} saves", v),
                                                    _ => format!("{}", v),
                                                };
                                                ui.label(RichText::new(formatted).monospace().small().strong().color(palette.accent_heading));
                                            } else {
                                                ui.label(RichText::new(item.default_hint).small().color(palette.rung_text_dim));
                                            }
                                        } else {
                                            ui.label(RichText::new(item.default_hint).small().color(palette.rung_text_dim));
                                        }
                                    } else {
                                        ui.label(RichText::new(item.default_hint).small().color(palette.rung_text_dim));
                                    }
                                });
                            });

                            // Row 2: Description
                            ui.label(RichText::new(item.desc).small().color(palette.rung_text_dim));

                            // Row 3: Action Buttons
                            ui.horizontal(|ui| {
                                let primary_addr = if item.iec.contains("..") {
                                    item.iec.split("..").next().unwrap_or(item.iec)
                                } else {
                                    item.iec
                                };
                                let primary_legacy = if item.legacy.contains("..") {
                                    item.legacy.split("..").next().unwrap_or(item.legacy)
                                } else {
                                    item.legacy
                                };

                                if selected_meta.is_some() {
                                    let btn_text = format!("↵ Use {}", primary_addr);
                                    if ui
                                        .button(RichText::new(btn_text).small().color(palette.accent_heading))
                                        .on_hover_text(format!("Assign address '{}' to selected element", primary_addr))
                                        .clicked()
                                    {
                                        assign_address = Some(primary_addr.to_string());
                                    }
                                }

                                if ui
                                    .small_button(format!("📋 {}", primary_addr))
                                    .on_hover_text(format!("Copy IEC address '{}' to clipboard", primary_addr))
                                    .clicked()
                                {
                                    ui.ctx().copy_text(primary_addr.to_string());
                                }

                                if ui
                                    .small_button(format!("📋 {}", primary_legacy))
                                    .on_hover_text(format!("Copy legacy address '{}' to clipboard", primary_legacy))
                                    .clicked()
                                {
                                    ui.ctx().copy_text(primary_legacy.to_string());
                                }
                            });
                        });

                    ui.add_space(2.0);
                }
            }

            ui.add_space(4.0);

            // Memory Architecture Legend Card
            egui::CollapsingHeader::new(
                RichText::new("📖 Memory Architecture Legend")
                    .small()
                    .strong()
                    .color(palette.accent_heading),
            )
            .id_salt("special_reg_legend_collapsing")
            .default_open(false)
            .show(ui, |ui| {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("RP2040 Memory Map Sections:")
                            .small()
                            .strong()
                            .color(palette.text_primary),
                    );
                    egui::Grid::new("mem_legend_grid")
                        .num_columns(2)
                        .spacing([6.0, 3.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("0x00..0x1F:").small().monospace().color(palette.rung_text_dim));
                            ui.label(RichText::new("Inputs X0..X31 (%IX0.0..%IX3.7)").small().color(palette.text_primary));
                            ui.end_row();

                            ui.label(RichText::new("0x20..0x3F:").small().monospace().color(palette.rung_text_dim));
                            ui.label(RichText::new("Outputs Y0..Y31 (%QX0.0..%QX3.7)").small().color(palette.text_primary));
                            ui.end_row();

                            ui.label(RichText::new("0x40..0x7F:").small().monospace().color(palette.rung_text_dim));
                            ui.label(RichText::new("User Relays M0..M63 (%MX0.0..%MX7.7)").small().color(palette.text_primary));
                            ui.end_row();

                            ui.label(RichText::new("0x80..0x9F:").small().monospace().color(palette.rung_text_dim));
                            ui.label(RichText::new("Latch Relays M64..M95 (Flash Saved)").small().color(palette.accent_ok));
                            ui.end_row();

                            ui.label(RichText::new("0xA0..0xBF:").small().monospace().color(palette.rung_text_dim));
                            ui.label(RichText::new("Special Markers %SM0..%SM31 (M8000..)").small().color(palette.accent_heading));
                            ui.end_row();

                            ui.label(RichText::new("0xC0..0xDF:").small().monospace().color(palette.rung_text_dim));
                            ui.label(RichText::new("Timers T0..T15 (%TX0..%TX15)").small().color(palette.text_primary));
                            ui.end_row();

                            ui.label(RichText::new("0xE0..0xFF:").small().monospace().color(palette.rung_text_dim));
                            ui.label(RichText::new("Counters C0..C15 (%CX0..%CX15)").small().color(palette.text_primary));
                            ui.end_row();
                        });
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Word Memory: D0..D95 (User), D96..D127 / %MW96..%MW127 (RETAIN), %SD0..%SD31 (D128..D159 / D8000..D8031)")
                            .small()
                            .color(palette.rung_text_dim),
                    );
                });
            });
        });

    // Apply any pending address assignment to selected element
    if let Some(addr) = assign_address {
        if let Some(elem) = app.doc.selected_element_mut() {
            elem.name = addr;
            app.doc.is_dirty = true;
        }
    }
}

// =============================================================================
// TAB 4: REAL-TIME DATA WATCH TABLE & REGISTER MONITOR
// =============================================================================
fn render_watch_table(
    ui: &mut Ui,
    app: &mut CanvasApp,
    palette: ThemePalette,
    sim_active: bool,
) {
    let mut remove_idx: Option<usize> = None;
    let mut write_action: Option<(usize, String)> = None;
    let mut add_preset_kind: Option<u8> = None;
    let mut clear_all_watch = false;

    egui::ScrollArea::vertical()
        .id_salt("watch_table_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Header
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Data Watch Table & Register Monitor")
                            .color(palette.accent_heading)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("✖ Clear").color(Color32::from_rgb(220, 80, 80)))
                            .on_hover_text("Clear all watched rows")
                            .clicked()
                        {
                            clear_all_watch = true;
                        }

                        ui.menu_button("➕ Presets", |ui| {
                            if ui.button("➕ All Digital Inputs (%IX)").clicked() {
                                add_preset_kind = Some(1);
                                ui.close();
                            }
                            if ui.button("➕ All Digital Outputs (%QX)").clicked() {
                                add_preset_kind = Some(2);
                                ui.close();
                            }
                            if ui.button("➕ All Analog Inputs (%IW)").clicked() {
                                add_preset_kind = Some(3);
                                ui.close();
                            }
                            if ui.button("➕ Flash Retentive Variables (M64..M71, D128..D135)").clicked() {
                                add_preset_kind = Some(4);
                                ui.close();
                            }
                            if ui.button("➕ Diagnostic Registers (%SD0..%SD6)").clicked() {
                                add_preset_kind = Some(5);
                                ui.close();
                            }
                        });
                    });
                });

                ui.separator();

                // Search & Filter
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔍").small());
                    ui.add(
                        egui::TextEdit::singleline(&mut app.watch_filter_text)
                            .hint_text("Filter address or symbol...")
                            .desired_width(ui.available_width() - 30.0),
                    );
                    if !app.watch_filter_text.is_empty() && ui.small_button("✖").clicked() {
                        app.watch_filter_text.clear();
                    }
                });
            });

            ui.add_space(4.0);

            // Table of watched rows
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                let filter_lc = app.watch_filter_text.trim().to_lowercase();

                egui::ScrollArea::horizontal()
                    .id_salt("watch_grid_horizontal_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("watch_table_grid")
                            .striped(true)
                            .num_columns(6)
                            .spacing([5.0, 5.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("ADDR").strong().small().color(palette.rung_text_dim));
                                ui.label(RichText::new("SYMBOL").strong().small().color(palette.rung_text_dim));
                                ui.label(RichText::new("FMT").strong().small().color(palette.rung_text_dim));
                                ui.label(RichText::new("LIVE VALUE").strong().small().color(palette.rung_text_dim));
                                ui.label(RichText::new("WRITE").strong().small().color(palette.rung_text_dim));
                                ui.label(RichText::new("DEL").strong().small().color(palette.rung_text_dim));
                                ui.end_row();

                                for idx in 0..app.watch_rows.len() {
                                    let (matches_filter, addr, symbol, format, edit_buf) = {
                                        let row = &app.watch_rows[idx];
                                        let matches = filter_lc.is_empty()
                                            || row.address.to_lowercase().contains(&filter_lc)
                                            || row.symbol.to_lowercase().contains(&filter_lc);
                                        (matches, row.address.clone(), row.symbol.clone(), row.format, row.edit_buffer.clone())
                                    };

                                    if !matches_filter {
                                        continue;
                                    }

                                    // Col 1: Address
                                    ui.label(
                                        RichText::new(&addr)
                                            .monospace()
                                            .strong()
                                            .size(11.0)
                                            .color(palette.accent_heading),
                                    );

                                    // Col 2: Symbol (Increased width to 125px with full tooltip)
                                    let sym_w = 125.0;
                                    let mut sym_edit = symbol.clone();
                                    let sym_res = ui.add_sized(
                                        egui::vec2(sym_w, 18.0),
                                        egui::TextEdit::singleline(&mut sym_edit).desired_width(sym_w),
                                    );
                                    if sym_res.changed() {
                                        app.watch_rows[idx].symbol = sym_edit.clone();
                                        app.save_settings();
                                    }
                                    if !symbol.is_empty() {
                                        sym_res.on_hover_text(&symbol);
                                    }

                                    // Col 3: Format selector
                                    let fmt_text = match format {
                                        WatchDisplayFormat::Auto => "Auto",
                                        WatchDisplayFormat::Dec => "Dec",
                                        WatchDisplayFormat::Hex => "Hex",
                                        WatchDisplayFormat::Bin => "Bin",
                                        WatchDisplayFormat::Bool => "Bool",
                                    };
                                    ui.menu_button(RichText::new(fmt_text).small(), |ui| {
                                        for &(f, label) in &[
                                            (WatchDisplayFormat::Auto, "Auto"),
                                            (WatchDisplayFormat::Dec, "Dec"),
                                            (WatchDisplayFormat::Hex, "Hex (0x)"),
                                            (WatchDisplayFormat::Bin, "Bin (0b)"),
                                            (WatchDisplayFormat::Bool, "Bool (ON/OFF)"),
                                        ] {
                                            if ui.selectable_label(format == f, label).clicked() {
                                                app.watch_rows[idx].format = f;
                                                app.save_settings();
                                                ui.close();
                                            }
                                        }
                                    });

                                    // Col 4: Live Value
                                    let (val_txt, is_bit, val_col) = app.evaluate_watch_value(&addr, format);
                                    if is_bit && sim_active {
                                        if ui
                                            .small_button(RichText::new(&val_txt).strong().size(10.5).color(val_col))
                                            .on_hover_text("Click to toggle in simulation")
                                            .clicked()
                                        {
                                            app.doc.toggle_input(&addr);
                                        }
                                    } else {
                                        ui.label(
                                            RichText::new(&val_txt)
                                                .monospace()
                                                .strong()
                                                .size(10.5)
                                                .color(val_col),
                                        );
                                    }

                                    // Col 5: Write / Modify field & button
                                    ui.horizontal(|ui| {
                                        let mut buf = edit_buf;
                                        let edit_res = ui.add_sized(
                                            egui::vec2(50.0, 18.0),
                                            egui::TextEdit::singleline(&mut buf).desired_width(50.0),
                                        );
                                        if edit_res.changed() {
                                            app.watch_rows[idx].edit_buffer = buf.clone();
                                        }
                                        let enter_pressed = edit_res.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                        if ui.small_button(RichText::new("✍").color(palette.accent_ok)).clicked() || enter_pressed {
                                            write_action = Some((idx, buf));
                                        }
                                    });

                                    // Col 6: Delete button
                                    if ui.small_button(RichText::new("✖").color(palette.rung_text_dim)).clicked() {
                                        remove_idx = Some(idx);
                                    }

                                    ui.end_row();
                                }
                            });
                    });
            });

            ui.add_space(4.0);

            // Add new variable footer card
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Add Variable to Watch").strong().small().color(palette.accent_heading));
                ui.horizontal(|ui| {
                    ui.add_sized(
                        egui::vec2(90.0, 20.0),
                        egui::TextEdit::singleline(&mut app.watch_new_addr)
                            .hint_text("Address (%MW0, %IW0, D10)")
                            .desired_width(90.0),
                    );
                    ui.add_sized(
                        egui::vec2(125.0, 20.0),
                        egui::TextEdit::singleline(&mut app.watch_new_symbol)
                            .hint_text("Symbol tag (optional)")
                            .desired_width(125.0),
                    );
                    if ui.button(RichText::new("➕ Add").strong().color(palette.accent_ok)).clicked() {
                        let a = app.watch_new_addr.trim().to_string();
                        let s = app.watch_new_symbol.trim().to_string();
                        if !a.is_empty() {
                            app.add_watch_row(&a, &s, WatchDisplayFormat::Auto);
                            app.watch_new_addr.clear();
                            app.watch_new_symbol.clear();
                        }
                    }
                });

                // Quick add chips
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Quick Add:").small().color(palette.rung_text_dim));
                    for &chip in &["%IX0.0", "%QX0.0", "%IW0", "%IW1", "%IW2", "%IW3", "%MW0", "%MW1", "%SD1", "%SD6"] {
                        if ui.small_button(RichText::new(chip).monospace().size(10.0)).clicked() {
                            app.add_watch_row(chip, "", WatchDisplayFormat::Auto);
                        }
                    }
                });
            });
        });

    if let Some(idx) = remove_idx {
        if idx < app.watch_rows.len() {
            app.watch_rows.remove(idx);
            app.save_settings();
        }
    }
    if let Some((idx, val_str)) = write_action {
        let _ = app.write_watch_row(idx, &val_str);
    }
    if clear_all_watch {
        app.watch_rows.clear();
        app.save_settings();
    }
    if let Some(kind) = add_preset_kind {
        match kind {
            1 => {
                for i in 0..4 {
                    app.add_watch_row(&format!("%IX0.{}", i), &format!("INPUT_X{}", i), WatchDisplayFormat::Bool);
                }
            }
            2 => {
                for i in 0..6 {
                    app.add_watch_row(&format!("%QX0.{}", i), &format!("OUTPUT_Y{}", i), WatchDisplayFormat::Bool);
                }
            }
            3 => {
                app.add_watch_row("%IW0", "ADC0_GP26", WatchDisplayFormat::Dec);
                app.add_watch_row("%IW1", "ADC1_GP27", WatchDisplayFormat::Dec);
                app.add_watch_row("%IW2", "ADC2_GP28", WatchDisplayFormat::Dec);
                app.add_watch_row("%IW3", "ADC_TEMP_SENSOR", WatchDisplayFormat::Dec);
            }
            4 => {
                for i in 64..72 {
                    let byte = i / 8;
                    let bit = i % 8;
                    app.add_watch_row(&format!("%MX{}.{}", byte, bit), &format!("LATCH_M{}", i), WatchDisplayFormat::Bool);
                }
                for i in 96..104 {
                    app.add_watch_row(&format!("%MW{}", i), &format!("LATCH_D{}", i), WatchDisplayFormat::Dec);
                }
            }
            5 => {
                app.add_watch_row("%SD0", "WATCHDOG_PRESET", WatchDisplayFormat::Dec);
                app.add_watch_row("%SD1", "CURRENT_SCAN_TIME_US", WatchDisplayFormat::Dec);
                app.add_watch_row("%SD2", "MIN_SCAN_TIME_US", WatchDisplayFormat::Dec);
                app.add_watch_row("%SD3", "MAX_SCAN_TIME_US", WatchDisplayFormat::Dec);
                app.add_watch_row("%SD4", "SCAN_COUNT_LOW", WatchDisplayFormat::Dec);
                app.add_watch_row("%SD5", "SCAN_COUNT_HIGH", WatchDisplayFormat::Dec);
                app.add_watch_row("%SD6", "CPU_TEMP_C01", WatchDisplayFormat::Auto);
                app.add_watch_row("%SD7", "RETAIN_SAVE_COUNT", WatchDisplayFormat::Dec);
            }
            _ => {}
        }
    }
}
