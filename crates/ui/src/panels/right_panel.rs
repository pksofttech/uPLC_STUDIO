use egui::{Color32, RichText, Ui};
use crate::app::CanvasApp;
use crate::theme::ThemePalette;
use my_canvas_core::{CoilMode, ContactMode, SimState, TypeTag};

pub fn render(ui: &mut Ui, app: &mut CanvasApp) {
    let palette = app.theme.palette();
    let sim_active =
        app.doc.simulator.state != SimState::Stopped || app.doc.simulator.simulation_enabled;

    // 1. Top Box: Cross-Reference & Program Elements Explorer
    render_elements_explorer(ui, app, palette, sim_active);

    ui.add_space(6.0);

    // 2. Middle Box: Selected Element / Rung Inspector
    render_element_inspector(ui, app, palette);

    ui.add_space(6.0);

    // 3. Bottom Side-by-Side Split: Input Ports & Output Ports
    render_io_ports(ui, app, palette);
}

fn render_elements_explorer(
    ui: &mut Ui,
    app: &mut CanvasApp,
    palette: ThemePalette,
    sim_active: bool,
) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Program Elements & Cross-Ref")
                    .color(palette.accent_heading)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("Arrows: Navigate | Enter: Config")
                        .small()
                        .color(palette.rung_text_dim),
                );
            });
        });
        ui.separator();

        // Filter Bar: Category Tabs + Search Box
        ui.horizontal(|ui| {
            for (cat_id, lbl) in [
                (0u8, "All"),
                (1u8, "In(X)"),
                (2u8, "Out(Y)"),
                (3u8, "Mem(M)"),
                (4u8, "T/C"),
            ] {
                if ui
                    .selectable_label(app.elem_filter_cat == cat_id, RichText::new(lbl).small())
                    .clicked()
                {
                    app.elem_filter_cat = cat_id;
                }
            }
            ui.add(
                egui::TextEdit::singleline(&mut app.elem_filter_text)
                    .hint_text("Search tag...")
                    .desired_width(ui.available_width()),
            );
        });
        ui.add_space(2.0);

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

        egui::ScrollArea::vertical()
            .id_salt("listview_elements_scroll")
            .max_height(135.0)
            .show(ui, |ui| {
                egui::Grid::new("listview_elements_grid")
                    .striped(true)
                    .num_columns(4)
                    .min_col_width(54.0)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("TAG").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("TYPE").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("RUNGS").strong().small().color(palette.rung_text_dim));
                        ui.label(RichText::new("STATE").strong().small().color(palette.rung_text_dim));
                        ui.end_row();

                        for (name, el_type, val_str) in &elements_summary {
                            let prefix = name.chars().next().unwrap_or(' ').to_ascii_uppercase();
                            let cat_match = match app.elem_filter_cat {
                                1 => prefix == 'X' || prefix == 'I',
                                2 => prefix == 'Y' || prefix == 'Q',
                                3 => prefix == 'M' || prefix == 'R' || prefix == 'V',
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
                                .on_hover_text("Click to select on canvas | Double-click to configure");
                            if resp.clicked() {
                                clicked_name = Some(name.clone());
                            }
                            if resp.double_clicked() {
                                double_clicked_name = Some(name.clone());
                            }

                            ui.label(RichText::new(el_type.display_name()).small());

                            // Show cross-reference rung list (e.g. R01, R02)
                            let usages = app.doc.find_address_usages(name);
                            let rung_str = if usages.is_empty() {
                                "-".to_string()
                            } else {
                                usages
                                    .iter()
                                    .take(3)
                                    .map(|(r, _, _)| format!("R{:02}", r + 1))
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
                                .on_hover_text("Click to toggle simulation state")
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
                app.doc.simulator.toggle_input(&n);
            } else {
                app.open_config_for_selected();
            }
        }
        if let Some(n) = toggle_name {
            app.doc.simulator.toggle_input(&n);
        }
    });
}

fn render_element_inspector(ui: &mut Ui, app: &mut CanvasApp, palette: ThemePalette) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        let mut open_full_dialog = false;

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Element Inspector")
                    .color(palette.accent_warn)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if app.doc.selected_element_id.is_some()
                    && ui
                        .button(
                            RichText::new("Configure... (Enter)")
                                .color(palette.accent_heading)
                                .strong(),
                        )
                        .on_hover_text("Open professional LD Element Configuration window")
                        .clicked()
                {
                    open_full_dialog = true;
                }
            });
        });
        ui.separator();

        let mut deleted = false;
        let mut toggle_multi_mode = false;
        let mut clear_multi_sel = false;
        let mut new_contact_mode: Option<ContactMode> = None;
        let mut new_coil_mode: Option<CoilMode> = None;
        let mut quick_assign: Option<(String, String)> = None;

        if app.doc.selected_element_ids.len() > 1 {
            let count = app.doc.selected_element_ids.len();
            ui.label(
                RichText::new(format!("Multi-Selection ({} elements)", count))
                    .color(palette.accent_heading)
                    .strong(),
            );
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
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(RichText::new("Invert Mode (NO/NC)").color(palette.accent_warn))
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
        } else if let Some(elem) = app.doc.selected_element_mut() {
            egui::Grid::new("selected_elem_props_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Element:");
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(elem.element_type.display_name())
                                .strong()
                                .color(palette.text_primary),
                        );
                        ui.label(
                            RichText::new(format!("#{}", elem.id))
                                .monospace()
                                .small()
                                .color(palette.rung_text_dim),
                        );
                    });
                    ui.end_row();

                    ui.label("Address:");
                    ui.text_edit_singleline(&mut elem.name);
                    ui.end_row();

                    ui.label("Symbol Tag:");
                    ui.text_edit_singleline(&mut elem.symbol);
                    ui.end_row();

                    match elem.element_type {
                        TypeTag::Contact => {
                            ui.label("Contact:");
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(
                                        elem.contact_mode == ContactMode::NormallyOpen,
                                        "-[ ]- NO",
                                    )
                                    .clicked()
                                    && elem.contact_mode != ContactMode::NormallyOpen
                                {
                                    new_contact_mode = Some(ContactMode::NormallyOpen);
                                }
                                if ui
                                    .selectable_label(
                                        elem.contact_mode == ContactMode::NormallyClosed,
                                        "-[/]- NC",
                                    )
                                    .clicked()
                                    && elem.contact_mode != ContactMode::NormallyClosed
                                {
                                    new_contact_mode = Some(ContactMode::NormallyClosed);
                                }
                                if ui
                                    .selectable_label(
                                        elem.contact_mode == ContactMode::PositiveTransition,
                                        "-[P]- P",
                                    )
                                    .clicked()
                                    && elem.contact_mode != ContactMode::PositiveTransition
                                {
                                    new_contact_mode = Some(ContactMode::PositiveTransition);
                                }
                                if ui
                                    .selectable_label(
                                        elem.contact_mode == ContactMode::NegativeTransition,
                                        "-[N]- N",
                                    )
                                    .clicked()
                                    && elem.contact_mode != ContactMode::NegativeTransition
                                {
                                    new_contact_mode = Some(ContactMode::NegativeTransition);
                                }
                            });
                            ui.end_row();
                        }
                        TypeTag::Coil => {
                            ui.label("Coil Mode:");
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(elem.coil_mode == CoilMode::Normal, "( )")
                                    .clicked()
                                    && elem.coil_mode != CoilMode::Normal
                                {
                                    new_coil_mode = Some(CoilMode::Normal);
                                }
                                if ui
                                    .selectable_label(elem.coil_mode == CoilMode::Negated, "(/)")
                                    .clicked()
                                    && elem.coil_mode != CoilMode::Negated
                                {
                                    new_coil_mode = Some(CoilMode::Negated);
                                }
                                if ui
                                    .selectable_label(elem.coil_mode == CoilMode::SetLatch, "(S)")
                                    .clicked()
                                    && elem.coil_mode != CoilMode::SetLatch
                                {
                                    new_coil_mode = Some(CoilMode::SetLatch);
                                }
                                if ui
                                    .selectable_label(
                                        elem.coil_mode == CoilMode::ResetUnlatch,
                                        "(R)",
                                    )
                                    .clicked()
                                    && elem.coil_mode != CoilMode::ResetUnlatch
                                {
                                    new_coil_mode = Some(CoilMode::ResetUnlatch);
                                }
                                if ui
                                    .selectable_label(
                                        elem.coil_mode == CoilMode::PositiveTransition,
                                        "(P)",
                                    )
                                    .clicked()
                                    && elem.coil_mode != CoilMode::PositiveTransition
                                {
                                    new_coil_mode = Some(CoilMode::PositiveTransition);
                                }
                                if ui
                                    .selectable_label(
                                        elem.coil_mode == CoilMode::NegativeTransition,
                                        "(N)",
                                    )
                                    .clicked()
                                    && elem.coil_mode != CoilMode::NegativeTransition
                                {
                                    new_coil_mode = Some(CoilMode::NegativeTransition);
                                }
                            });
                            ui.end_row();
                        }
                        TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => {
                            ui.label("Preset (ms):");
                            ui.add(
                                egui::DragValue::new(&mut elem.preset_value)
                                    .range(10..=600000)
                                    .speed(50),
                            );
                            ui.end_row();
                        }
                        TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => {
                            ui.label("Preset (PV):");
                            ui.add(
                                egui::DragValue::new(&mut elem.preset_value)
                                    .range(1..=10000)
                                    .speed(1),
                            );
                            ui.end_row();
                        }
                        TypeTag::Compare => {
                            ui.label("Compare:");
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt("cmp_op")
                                    .width(48.0)
                                    .selected_text(&elem.compare_op)
                                    .show_ui(ui, |ui| {
                                        for op in ["==", "!=", ">", "<", ">=", "<="] {
                                            ui.selectable_value(
                                                &mut elem.compare_op,
                                                op.to_string(),
                                                op,
                                            );
                                        }
                                    });
                                ui.add(egui::DragValue::new(&mut elem.preset_value).speed(1));
                            });
                            ui.end_row();
                        }
                        TypeTag::Math | TypeTag::Move => {
                            ui.label("Operand:");
                            ui.add(egui::DragValue::new(&mut elem.preset_value).speed(1));
                            ui.end_row();
                        }
                        TypeTag::Comment => {
                            ui.label("Comment:");
                            ui.text_edit_singleline(&mut elem.comment);
                            ui.end_row();
                        }
                        _ => {}
                    }
                });

            // 1-Click Quick Address Picker Pills
            ui.add_space(4.0);
            ui.label(RichText::new("Quick Address Assign:").small().weak());
            ui.horizontal_wrapped(|ui| {
                let presets: &[(&str, &str)] = match elem.element_type {
                    TypeTag::Coil => &[
                        ("Y0", "MOTOR_RUN"),
                        ("Y1", "LAMP_RUN"),
                        ("Y2", "ALARM_BUZZ"),
                        ("Y3", "LAMP_FAULT"),
                        ("M0", "RELAY_M0"),
                        ("M1", "RELAY_M1"),
                    ],
                    TypeTag::Ton | TypeTag::Tof | TypeTag::Tp => &[
                        ("T0", "TIMER_0"),
                        ("T1", "TIMER_1"),
                        ("T2", "TIMER_2"),
                    ],
                    TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => &[
                        ("C0", "COUNT_0"),
                        ("C1", "COUNT_1"),
                        ("C2", "COUNT_2"),
                    ],
                    _ => &[
                        ("X0", "START_PB"),
                        ("X1", "STOP_PB"),
                        ("X2", "SENS_LIMIT"),
                        ("X3", "EMERGENCY"),
                        ("Y0", "MOTOR_RUN"),
                        ("M0", "RELAY_M0"),
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

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .button(
                        RichText::new("Full Settings (Enter)").color(palette.accent_heading),
                    )
                    .clicked()
                {
                    open_full_dialog = true;
                }
                if ui
                    .button(RichText::new("Delete (Del)").color(palette.accent_danger))
                    .clicked()
                {
                    deleted = true;
                }
            });
        } else if let Some(r_idx) = app.doc.selected_rung_idx {
            if let Some(rung) = app.doc.program.rungs.get_mut(r_idx) {
                ui.label(RichText::new(format!("Active Rung #{:02}", r_idx + 1)).strong());
                ui.horizontal(|ui| {
                    ui.label("Comment:");
                    ui.text_edit_singleline(&mut rung.comment);
                });
                ui.weak("Tip: Click any element to select it, use Arrow Keys to navigate, or Double-Click / press Enter to open the full Configuration Window.");
            }
        }

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

fn render_io_ports(ui: &mut Ui, app: &mut CanvasApp, palette: ThemePalette) {
    let mut toggle_port_var: Option<String> = None;
    ui.columns(2, |cols| {
        // Left Column: Input Ports (Name | Port)
        cols[0].group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("Input Ports")
                    .strong()
                    .color(palette.accent_warn),
            );
            ui.separator();

            egui::ScrollArea::vertical()
                .id_salt("input_ports_scroll")
                .max_height(155.0)
                .show(ui, |ui| {
                    egui::Grid::new("input_ports_grid")
                        .striped(true)
                        .num_columns(2)
                        .spacing([6.0, 5.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("Switch").strong().small());
                            ui.label(RichText::new("Pin").strong().small());
                            ui.end_row();

                            for io in app.doc.program.io_mappings.iter_mut().filter(|m| m.is_input)
                            {
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
                                    RichText::new(format!(
                                        "{} {}",
                                        if state { "●" } else { "○" },
                                        io.plc_address
                                    ))
                                    .monospace()
                                    .color(btn_fg),
                                )
                                .fill(btn_bg);

                                if ui
                                    .add(btn)
                                    .on_hover_text(format!(
                                        "{} — Click to toggle ON/OFF",
                                        io.symbol
                                    ))
                                    .clicked()
                                {
                                    toggle_port_var = Some(io.plc_address.clone());
                                }
                                ui.add(
                                    egui::TextEdit::singleline(&mut io.hw_pin).desired_width(36.0),
                                );
                                ui.end_row();
                            }
                        });
                });
        });

        // Right Column: Output Ports (Name | Port)
        cols[1].group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("Output Ports")
                    .strong()
                    .color(palette.accent_heading),
            );
            ui.separator();

            egui::ScrollArea::vertical()
                .id_salt("output_ports_scroll")
                .max_height(155.0)
                .show(ui, |ui| {
                    egui::Grid::new("output_ports_grid")
                        .striped(true)
                        .num_columns(2)
                        .spacing([6.0, 5.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("LED").strong().small());
                            ui.label(RichText::new("Pin").strong().small());
                            ui.end_row();

                            for io in app
                                .doc
                                .program
                                .io_mappings
                                .iter_mut()
                                .filter(|m| !m.is_input)
                            {
                                let state = app.doc.simulator.get_var_value(&io.plc_address);
                                let badge = if state { "●" } else { "○" };
                                ui.label(
                                    RichText::new(format!("{} {}", badge, io.plc_address))
                                        .monospace()
                                        .color(if state {
                                            palette.wire_energized
                                        } else {
                                            palette.rung_text_dim
                                        }),
                                )
                                .on_hover_text(&io.symbol);
                                ui.add(
                                    egui::TextEdit::singleline(&mut io.hw_pin).desired_width(36.0),
                                );
                                ui.end_row();
                            }
                        });
                });
        });
    });

    if let Some(var) = toggle_port_var {
        app.doc.simulator.toggle_input(&var);
    }
}
