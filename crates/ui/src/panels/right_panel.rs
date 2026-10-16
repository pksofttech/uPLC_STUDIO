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

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;

        let prop_label = if has_elem_sel {
            "⚙ Property ●"
        } else if has_rung_sel {
            "⚙ Network ●"
        } else {
            "⚙ Property"
        };

        let t0 = ui.selectable_label(
            app.right_panel_tab == 0,
            RichText::new(prop_label).size(11.5).strong(),
        );
        if t0.clicked() {
            app.right_panel_tab = 0;
        }

        let tags_count = app.doc.program.collect_all_elements().len();
        let t1 = ui.selectable_label(
            app.right_panel_tab == 1,
            RichText::new(format!("📋 Tags ({})", tags_count)).size(11.5).strong(),
        );
        if t1.clicked() {
            app.right_panel_tab = 1;
        }

        let in_count = app.doc.program.io_mappings.iter().filter(|m| m.is_input).count();
        let out_count = app.doc.program.io_mappings.iter().filter(|m| !m.is_input).count();
        let t2 = ui.selectable_label(
            app.right_panel_tab == 2,
            RichText::new(format!("⚡ I/O ({}/{})", in_count, out_count)).size(11.5).strong(),
        );
        if t2.clicked() {
            app.right_panel_tab = 2;
        }
    });

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
                        let (type_color, type_title) = match elem.element_type {
                            TypeTag::Contact => (palette.accent_heading, "CONTACT"),
                            TypeTag::Coil => (palette.accent_ok, "COIL"),
                            TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => (palette.accent_warn, "TIMER"),
                            TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => (palette.accent_warn, "COUNTER"),
                            TypeTag::Sr | TypeTag::Rs => (palette.accent_warn, "BISTABLE FB"),
                            TypeTag::Jump | TypeTag::Label => (palette.accent_heading, "FLOW"),
                            TypeTag::Compare | TypeTag::Math | TypeTag::Move => (Color32::from_rgb(180, 110, 240), "FUNCTION"),
                            _ => (palette.rung_text_dim, "ELEMENT"),
                        };

                        let badge_frame = egui::Frame::new()
                            .fill(type_color.linear_multiply(0.2))
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

                // 2. Identification Card (Address, Symbol Tag, Remark)
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Identification & Addressing").strong().size(11.5).color(palette.text_primary));
                    ui.separator();

                    egui::Grid::new("elem_id_grid")
                        .num_columns(2)
                        .spacing([8.0, 6.0])
                        .min_col_width(75.0)
                        .show(ui, |ui| {
                            ui.label("Address / Tag:");
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut elem.name)
                                    .hint_text("e.g. X0, Y0, M0, %IX0.0")
                                    .desired_width(140.0)
                            );
                            if resp.changed() {
                                elem.name = elem.name.to_ascii_uppercase();
                            }
                            ui.end_row();

                            ui.label("Symbol Name:");
                            ui.add(
                                egui::TextEdit::singleline(&mut elem.symbol)
                                    .hint_text("e.g. START_PB")
                                    .desired_width(140.0)
                            );
                            ui.end_row();

                            ui.label("Remark:");
                            ui.add(
                                egui::TextEdit::singleline(&mut elem.comment)
                                    .hint_text("Description...")
                                    .desired_width(140.0)
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
                            ui.label(RichText::new("Contact Mode (รูปแบบหน้าสัมผัส)").strong().size(11.5));
                            ui.separator();

                            egui::Grid::new("contact_mode_grid")
                                .num_columns(2)
                                .spacing([6.0, 6.0])
                                .show(ui, |ui| {
                                    let is_no = elem.contact_mode == ContactMode::NormallyOpen;
                                    if ui.selectable_label(is_no, "-[ ]- NO (Normally Open)").clicked() && !is_no {
                                        new_contact_mode = Some(ContactMode::NormallyOpen);
                                    }

                                    let is_nc = elem.contact_mode == ContactMode::NormallyClosed;
                                    if ui.selectable_label(is_nc, "-[/]- NC (Normally Closed)").clicked() && !is_nc {
                                        new_contact_mode = Some(ContactMode::NormallyClosed);
                                    }
                                    ui.end_row();

                                    let is_p = elem.contact_mode == ContactMode::PositiveTransition;
                                    if ui.selectable_label(is_p, "-[P]- Rising Edge (P)").clicked() && !is_p {
                                        new_contact_mode = Some(ContactMode::PositiveTransition);
                                    }

                                    let is_n = elem.contact_mode == ContactMode::NegativeTransition;
                                    if ui.selectable_label(is_n, "-[N]- Falling Edge (N)").clicked() && !is_n {
                                        new_contact_mode = Some(ContactMode::NegativeTransition);
                                    }
                                    ui.end_row();
                                });
                        }
                        TypeTag::Coil => {
                            ui.label(RichText::new("Coil Mode (รูปแบบคอยล์)").strong().size(11.5));
                            ui.separator();

                            egui::Grid::new("coil_mode_grid")
                                .num_columns(2)
                                .spacing([6.0, 6.0])
                                .show(ui, |ui| {
                                    let is_norm = elem.coil_mode == CoilMode::Normal;
                                    if ui.selectable_label(is_norm, "-( )- Normal Output").clicked() && !is_norm {
                                        new_coil_mode = Some(CoilMode::Normal);
                                    }

                                    let is_neg = elem.coil_mode == CoilMode::Negated;
                                    if ui.selectable_label(is_neg, "-(/)- Negated Output").clicked() && !is_neg {
                                        new_coil_mode = Some(CoilMode::Negated);
                                    }
                                    ui.end_row();

                                    let is_set = elem.coil_mode == CoilMode::SetLatch;
                                    if ui.selectable_label(is_set, "-(S)- Set Latch").clicked() && !is_set {
                                        new_coil_mode = Some(CoilMode::SetLatch);
                                    }

                                    let is_rst = elem.coil_mode == CoilMode::ResetUnlatch;
                                    if ui.selectable_label(is_rst, "-(R)- Reset Unlatch").clicked() && !is_rst {
                                        new_coil_mode = Some(CoilMode::ResetUnlatch);
                                    }
                                    ui.end_row();

                                    let is_pos = elem.coil_mode == CoilMode::PositiveTransition;
                                    if ui.selectable_label(is_pos, "-(P)- Rising Pulse").clicked() && !is_pos {
                                        new_coil_mode = Some(CoilMode::PositiveTransition);
                                    }

                                    let is_neg_p = elem.coil_mode == CoilMode::NegativeTransition;
                                    if ui.selectable_label(is_neg_p, "-(N)- Falling Pulse").clicked() && !is_neg_p {
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
                            ui.label(RichText::new(fb_name).strong().size(11.5));
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label("Preset (PT):");
                                ui.add(
                                    egui::DragValue::new(&mut elem.preset_value)
                                        .range(10..=600000)
                                        .speed(50)
                                        .suffix(" ms"),
                                );
                                let sec = elem.preset_value as f32 / 1000.0;
                                ui.label(RichText::new(format!("({:.2}s)", sec)).small().weak());
                            });

                            ui.add_space(4.0);
                            ui.label(RichText::new("Quick Presets:").small().weak());
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
                            ui.label(RichText::new(fb_name).strong().size(11.5));
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label("Preset (PV):");
                                ui.add(
                                    egui::DragValue::new(&mut elem.preset_value)
                                        .range(1..=10000)
                                        .speed(1),
                                );
                            });

                            ui.add_space(4.0);
                            ui.label(RichText::new("Quick Presets:").small().weak());
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
                            ui.label(RichText::new("Compare Logic (เปรียบเทียบค่า)").strong().size(11.5));
                            ui.separator();

                            egui::Grid::new("cmp_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Register Tag:");
                                    ui.text_edit_singleline(&mut elem.name);
                                    ui.end_row();

                                    ui.label("Operator:");
                                    egui::ComboBox::from_id_salt("cmp_op_combo")
                                        .width(60.0)
                                        .selected_text(&elem.compare_op)
                                        .show_ui(ui, |ui| {
                                            for op in ["==", "!=", ">", "<", ">=", "<="] {
                                                ui.selectable_value(&mut elem.compare_op, op.to_string(), op);
                                            }
                                        });
                                    ui.end_row();

                                    ui.label("Target Value:");
                                    ui.add(egui::DragValue::new(&mut elem.preset_value).speed(1));
                                    ui.end_row();
                                });
                        }
                        TypeTag::Math => {
                            ui.label(RichText::new("Math Operation (คำนวณเลข)").strong().size(11.5));
                            ui.separator();

                            egui::Grid::new("math_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Target Reg:");
                                    ui.text_edit_singleline(&mut elem.target_reg);
                                    ui.end_row();

                                    ui.label("Operation:");
                                    egui::ComboBox::from_id_salt("math_op_combo")
                                        .width(60.0)
                                        .selected_text(&elem.math_op)
                                        .show_ui(ui, |ui| {
                                            for op in ["+", "-", "*", "/", "%"] {
                                                ui.selectable_value(&mut elem.math_op, op.to_string(), op);
                                            }
                                        });
                                    ui.end_row();

                                    ui.label("Value:");
                                    ui.add(egui::DragValue::new(&mut elem.preset_value).speed(1));
                                    ui.end_row();
                                });
                        }
                        TypeTag::Move => {
                            ui.label(RichText::new("Move Data (ส่งถ่ายข้อมูล)").strong().size(11.5));
                            ui.separator();

                            egui::Grid::new("move_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Target Reg:");
                                    ui.text_edit_singleline(&mut elem.target_reg);
                                    ui.end_row();

                                    ui.label("Value:");
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
                            ui.label(RichText::new(fb_name).strong().size(11.5));
                            ui.separator();

                            egui::Grid::new("bistable_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    let r_label = if elem.element_type == TypeTag::Sr { "Reset (R):" } else { "Reset (R1):" };
                                    ui.label(r_label);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut elem.target_reg)
                                            .hint_text("e.g. X1, M1, %IX0.1")
                                            .desired_width(120.0),
                                    );
                                    ui.end_row();

                                    ui.label("Output (Q1):");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut elem.name)
                                            .hint_text("e.g. M0, Y0, %QX0.0")
                                            .desired_width(120.0),
                                    );
                                    ui.end_row();
                                });

                            ui.add_space(4.0);
                            let formula = if elem.element_type == TypeTag::Sr {
                                "Set-dominant: Q1 = S1 OR (NOT R AND Q1_prev)"
                            } else {
                                "Reset-dominant: Q1 = NOT R1 AND (S OR Q1_prev)"
                            };
                            ui.label(RichText::new(formula).small().weak());
                        }
                        TypeTag::Jump => {
                            ui.label(RichText::new("Conditional Jump (JMP)").strong().size(11.5));
                            ui.separator();

                            egui::Grid::new("jump_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Target Label:");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut elem.name)
                                            .hint_text("e.g. LBL1, BYPASS")
                                            .desired_width(120.0),
                                    );
                                    ui.end_row();
                                });
                            ui.add_space(4.0);
                            ui.label(RichText::new("When power is TRUE, jumps to the matching label network.").small().weak());
                        }
                        TypeTag::Label => {
                            ui.label(RichText::new("Target Label (LBL)").strong().size(11.5));
                            ui.separator();

                            egui::Grid::new("label_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Label Name:");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut elem.name)
                                            .hint_text("e.g. LBL1, BYPASS")
                                            .desired_width(120.0),
                                    );
                                    ui.end_row();
                                });
                            ui.add_space(4.0);
                            ui.label(RichText::new("Designates landing destination for JMP instructions.").small().weak());
                        }
                        _ => {
                            ui.label(RichText::new("No special parameters").small().weak());
                        }
                    }
                });

                ui.add_space(4.0);

                // 4. Quick Address Assign Pills (1-Click)
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Quick Address Assign (คลิกเพื่อกำหนดแอดเดรสทันที)").small().weak());
                    ui.separator();

                    ui.horizontal_wrapped(|ui| {
                        let presets: &[(&str, &str)] = match elem.element_type {
                            TypeTag::Coil => &[
                                ("Y0", "MOTOR_RUN"),
                                ("Y1", "LAMP_RUN"),
                                ("Y2", "ALARM_BUZZ"),
                                ("Y3", "LAMP_FAULT"),
                                ("M0", "FLAG_READY"),
                                ("M1", "FLAG_AUTO"),
                                ("M2", "FLAG_ERROR"),
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
                                ("X0", "START_PB"),
                                ("X1", "STOP_PB"),
                                ("X2", "SENS_LIMIT"),
                                ("X3", "EMERGENCY"),
                                ("Y0", "MOTOR_RUN"),
                                ("M0", "FLAG_READY"),
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

                        ui.label("Network Title / Comment:");
                        ui.add(
                            egui::TextEdit::singleline(&mut rung.comment)
                                .hint_text("Enter description for this network...")
                                .desired_width(f32::INFINITY),
                        );

                        ui.add_space(4.0);
                        ui.label(RichText::new(format!("Elements: {} in series / parallel", rung.elements.len())).small().weak());

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
                    ui.weak("💡 Tip: Click any contact or coil inside this network to view its element properties, or press Enter to edit.");
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
                    ui.label(RichText::new(format!("Target Hardware: {}", app.doc.program.hardware)).small().weak());
                    ui.label(RichText::new(format!("Total Networks: {}", app.doc.program.rungs.len())).small().weak());

                    let total_elems: usize = app.doc.program.rungs.iter().map(|r| r.elements.len()).sum();
                    ui.label(RichText::new(format!("Total Elements: {}", total_elems)).small().weak());

                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("💡 Click any contact, coil, or network on the canvas to inspect and edit its properties here.")
                            .small()
                            .color(palette.rung_text_dim),
                    );

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("📋 Open Tags Explorer").clicked() {
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
// TAB 1: TAGS & CROSS-REFERENCE EXPLORER
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
                RichText::new("Tags & Cross-Reference Explorer")
                    .color(palette.accent_heading)
                    .strong(),
            );
        });
        ui.separator();

        // Filter Bar: Category Tabs
        ui.horizontal(|ui| {
            for (cat_id, lbl) in [
                (0u8, "All"),
                (1u8, "In (X)"),
                (2u8, "Out (Y)"),
                (3u8, "Mem (M)"),
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
                    .hint_text("🔍 Search tag name or type...")
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

    let elements_summary = app.doc.program.collect_all_elements();
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
                .min_col_width(52.0)
                .spacing([8.0, 5.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("TAG").strong().small().color(palette.rung_text_dim));
                    ui.label(RichText::new("TYPE").strong().small().color(palette.rung_text_dim));
                    ui.label(RichText::new("NETWORKS").strong().small().color(palette.rung_text_dim));
                    ui.label(RichText::new("STATE").strong().small().color(palette.rung_text_dim));
                    ui.end_row();

                    for (name, el_type, val_str) in &elements_summary {
                        let prefix = name.chars().next().unwrap_or(' ').to_ascii_uppercase();
                        let cat_match = match app.elem_filter_cat {
                            1 => prefix == 'X' || prefix == 'I' || name.starts_with("%IX"),
                            2 => prefix == 'Y' || prefix == 'Q' || name.starts_with("%QX"),
                            3 => prefix == 'M' || prefix == 'R' || prefix == 'V' || name.starts_with("%MX"),
                            4 => prefix == 'T' || prefix == 'C',
                            _ => true,
                        };
                        if !cat_match {
                            continue;
                        }
                        if !filter_upper.is_empty()
                            && !name.to_ascii_uppercase().contains(&filter_upper)
                            && !el_type.display_name().to_ascii_uppercase().contains(&filter_upper)
                        {
                            continue;
                        }

                        let is_active = app.doc.simulator.get_var_value(name);
                        let is_selected_tag = !selected_name.is_empty() && selected_name == *name;
                        let name_color = if is_selected_tag {
                            palette.rung_text_sel
                        } else if is_active {
                            palette.accent_ok
                        } else {
                            palette.text_primary
                        };

                        let resp = ui
                            .selectable_label(
                                is_selected_tag,
                                RichText::new(name).monospace().color(name_color),
                            )
                            .on_hover_text("Click: Select on canvas | Double-click: Edit Properties");
                        if resp.clicked() {
                            clicked_name = Some(name.clone());
                        }
                        if resp.double_clicked() {
                            double_clicked_name = Some(name.clone());
                        }

                        ui.label(RichText::new(el_type.display_name()).small());

                        // Cross-reference network usages list (e.g. N01, N02)
                        let usages = app.doc.find_address_usages(name);
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
                            TypeTag::Contact | TypeTag::Coil => {
                                if is_active {
                                    ("● ON (1)".to_string(), palette.accent_ok)
                                } else {
                                    ("○ OFF (0)".to_string(), palette.rung_text_dim)
                                }
                            }
                            _ => (val_str.clone(), palette.text_primary),
                        };
                        if ui
                            .small_button(RichText::new(&live_val).color(val_color))
                            .on_hover_text("Click to toggle in simulation")
                            .clicked()
                        {
                            toggle_name = Some(name.clone());
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
                ui.label(RichText::new(format!("Platform: {}", app.doc.program.hardware)).small().weak());
                let in_count = app.doc.program.io_mappings.iter().filter(|m| m.is_input).count();
                let out_count = app.doc.program.io_mappings.iter().filter(|m| !m.is_input).count();
                ui.label(RichText::new(format!("Configured: {} Inputs, {} Outputs", in_count, out_count)).small().color(palette.text_primary));
            });

            ui.add_space(4.0);

            // Digital Inputs Group
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new("Digital Inputs (X / %IX)")
                        .strong()
                        .color(palette.accent_warn),
                );
                ui.separator();

                egui::Grid::new("input_ports_grid")
                    .striped(true)
                    .num_columns(4)
                    .spacing([6.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("STATUS").strong().small());
                        ui.label(RichText::new("ADDRESS").strong().small());
                        ui.label(RichText::new("SYMBOL").strong().small());
                        ui.label(RichText::new("PIN").strong().small());
                        ui.end_row();

                        for io in app.doc.program.io_mappings.iter_mut().filter(|m| m.is_input) {
                            let state = app.doc.simulator.get_var_value(&io.plc_address);
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
                                .on_hover_text(format!("{} — Click to toggle ON/OFF", io.symbol))
                                .clicked()
                            {
                                toggle_port_var = Some(io.plc_address.clone());
                            }

                            ui.label(RichText::new(&io.plc_address).monospace().strong());
                            ui.label(RichText::new(&io.symbol).small().color(palette.text_primary));
                            ui.add(egui::TextEdit::singleline(&mut io.hw_pin).desired_width(38.0));
                            ui.end_row();
                        }
                    });
            });

            ui.add_space(4.0);

            // Digital Outputs Group
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new("Digital Outputs (Y / %QX)")
                        .strong()
                        .color(palette.accent_ok),
                );
                ui.separator();

                egui::Grid::new("output_ports_grid")
                    .striped(true)
                    .num_columns(4)
                    .spacing([6.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("LED").strong().small());
                        ui.label(RichText::new("ADDRESS").strong().small());
                        ui.label(RichText::new("SYMBOL").strong().small());
                        ui.label(RichText::new("PIN").strong().small());
                        ui.end_row();

                        for io in app.doc.program.io_mappings.iter_mut().filter(|m| !m.is_input) {
                            let state = app.doc.simulator.get_var_value(&io.plc_address);
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
                            .on_hover_text(&io.symbol);

                            ui.label(RichText::new(&io.plc_address).monospace().strong());
                            ui.label(RichText::new(&io.symbol).small().color(palette.text_primary));
                            ui.add(egui::TextEdit::singleline(&mut io.hw_pin).desired_width(38.0));
                            ui.end_row();
                        }
                    });
            });
        });

    if let Some(var) = toggle_port_var {
        app.doc.toggle_input(&var);
    }
}
