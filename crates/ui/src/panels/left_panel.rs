use egui::{pos2, Color32, RichText, Sense, Stroke, Ui};
use crate::app::CanvasApp;
use crate::icons::{modern_component_button, modern_tool_tile, VectorIcon};
use crate::tools::ActiveTool;

pub fn render(ui: &mut Ui, app: &mut CanvasApp) {
    let has_rung = app.doc.selected_rung_idx.is_some();
    let palette = app.theme.palette();

    // --- 1. Top Box: Modern Icon Grid for Network Tools ---
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new("Network Tools")
                .color(palette.accent_heading)
                .strong(),
        );
        ui.separator();

        let avail_w = ui.available_width();
        let half_w = ((avail_w - 6.0) * 0.5).max(68.0);

        // Row 1: Undo / Redo
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if modern_tool_tile(
                ui,
                VectorIcon::Undo,
                "Undo",
                palette.accent_warn,
                app.doc.can_undo(),
                half_w,
                palette,
            )
            .on_hover_text("Undo last circuit edit (Ctrl+Z)")
            .clicked()
            {
                app.doc.undo();
            }

            if modern_tool_tile(
                ui,
                VectorIcon::Redo,
                "Redo",
                palette.accent_heading,
                app.doc.can_redo(),
                half_w,
                palette,
            )
            .on_hover_text("Redo circuit edit (Ctrl+Y)")
            .clicked()
            {
                app.doc.redo();
            }
        });

        ui.add_space(4.0);

        // Row 2: New Network / Delete Network
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if modern_tool_tile(
                ui,
                VectorIcon::NewRung,
                "New Network",
                palette.accent_ok,
                true,
                half_w,
                palette,
            )
            .on_hover_text("Append a new network to the ladder program")
            .clicked()
            {
                app.doc.add_rung();
            }

            if modern_tool_tile(
                ui,
                VectorIcon::DeleteRung,
                "Delete",
                palette.accent_danger,
                has_rung,
                half_w,
                palette,
            )
            .on_hover_text("Delete selected network")
            .clicked()
            {
                if let Some(idx) = app.doc.selected_rung_idx {
                    app.doc.delete_rung(idx);
                }
            }
        });

        ui.add_space(4.0);

        // Row 3: Move Up / Move Down
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if modern_tool_tile(
                ui,
                VectorIcon::MoveUp,
                "Move Up",
                palette.accent_heading,
                has_rung,
                half_w,
                palette,
            )
            .on_hover_text("Move selected network up")
            .clicked()
            {
                if let Some(idx) = app.doc.selected_rung_idx {
                    app.doc.move_rung_up(idx);
                }
            }

            if modern_tool_tile(
                ui,
                VectorIcon::MoveDown,
                "Move Dn",
                palette.accent_heading,
                has_rung,
                half_w,
                palette,
            )
            .on_hover_text("Move selected network down")
            .clicked()
            {
                if let Some(idx) = app.doc.selected_rung_idx {
                    app.doc.move_rung_down(idx);
                }
            }
        });

        ui.add_space(4.0);

        // Row 4: Copy Up / Copy Down
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if modern_tool_tile(
                ui,
                VectorIcon::CopyUp,
                "Copy Up",
                Color32::from_rgb(162, 145, 208),
                has_rung,
                half_w,
                palette,
            )
            .on_hover_text("Duplicate network above current network")
            .clicked()
            {
                if let Some(idx) = app.doc.selected_rung_idx {
                    app.doc.copy_rung_at(idx, idx);
                }
            }

            if modern_tool_tile(
                ui,
                VectorIcon::CopyDown,
                "Copy Dn",
                Color32::from_rgb(162, 145, 208),
                has_rung,
                half_w,
                palette,
            )
            .on_hover_text("Paste/Duplicate network below current network")
            .clicked()
            {
                if let Some(idx) = app.doc.selected_rung_idx {
                    let src = app.copied_rung_idx.unwrap_or(idx);
                    app.doc.copy_rung_at(src, idx + 1);
                }
            }
        });

        ui.add_space(4.0);

        // Full-width Copy Network button
        if modern_tool_tile(
            ui,
            VectorIcon::CopyRung,
            "Copy Selected Network",
            palette.accent_warn,
            has_rung,
            avail_w,
            palette,
        )
        .on_hover_text("Copy selected network for pasting elsewhere")
        .clicked()
        {
            app.copied_rung_idx = app.doc.selected_rung_idx;
        }
    });

    ui.add_space(6.0);

    // --- 2. Bottom Box: Modern Components Palette with Schematic Vector Icons ---
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new("Components")
                .color(palette.accent_heading)
                .strong(),
        );
        ui.separator();

        let sel_count = if app.doc.selected_element_ids.is_empty() {
            if app.doc.selected_element_id.is_some() { 1 } else { 0 }
        } else {
            app.doc.selected_element_ids.len()
        };
        if sel_count > 0 {
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} Selected:", sel_count))
                            .small()
                            .color(palette.accent_heading)
                            .strong(),
                    );
                    if ui
                        .button(RichText::new("+∥ Par (ขนาน)").small().color(palette.accent_ok))
                        .on_hover_text("Add parallel branch across selection (P)")
                        .clicked()
                    {
                        let next_id = app.doc.program.alloc_id();
                        let new_elem = app.active_tool.create_default_element(next_id)
                            .unwrap_or_else(|| my_canvas_core::LadderElement::new_contact(next_id, "X0", "PAR", my_canvas_core::ContactMode::NormallyOpen));
                        app.doc.insert_parallel_to_selection(new_elem, false);
                    }
                    if ui
                        .button(RichText::new("+─ Ser (อนุกรม)").small().color(palette.accent_warn))
                        .on_hover_text("Add in series after selection (S)")
                        .clicked()
                    {
                        let next_id = app.doc.program.alloc_id();
                        let new_elem = app.active_tool.create_default_element(next_id)
                            .unwrap_or_else(|| my_canvas_core::LadderElement::new_contact(next_id, "X0", "SER", my_canvas_core::ContactMode::NormallyOpen));
                        app.doc.insert_series_to_selection(new_elem, true);
                    }
                });
            });
            ui.add_space(3.0);
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;

            // 1. IEC Bit Logic
            category_menu(
                ui,
                VectorIcon::ContactNo,
                "IEC Bit Logic",
                palette.accent_warn,
                palette,
                |ui| {
                    popup_tool_item(ui, app, "NO Contact — [ ]", ActiveTool::ContactNo);
                    popup_tool_item(ui, app, "NC Contact — [/]", ActiveTool::ContactNc);
                    popup_tool_item(ui, app, "P Contact — [P] (Rising Edge)", ActiveTool::ContactP);
                    popup_tool_item(ui, app, "N Contact — [N] (Falling Edge)", ActiveTool::ContactN);
                    ui.separator();
                    popup_tool_item(ui, app, "Coil — ( ) (Normal Out)", ActiveTool::CoilNormal);
                    popup_tool_item(ui, app, "Negated Coil — (/) (Inverted)", ActiveTool::CoilNegated);
                    popup_tool_item(ui, app, "Set Latch Coil — (S)", ActiveTool::CoilSet);
                    popup_tool_item(ui, app, "Reset Coil — (R)", ActiveTool::CoilReset);
                    popup_tool_item(ui, app, "Positive Edge Coil — (P)", ActiveTool::CoilP);
                    popup_tool_item(ui, app, "Negative Edge Coil — (N)", ActiveTool::CoilN);
                    ui.separator();
                    popup_tool_item(ui, app, "SR — Set-Dominant Bistable FB", ActiveTool::Sr);
                    popup_tool_item(ui, app, "RS — Reset-Dominant Bistable FB", ActiveTool::Rs);
                    ui.separator();
                    popup_tool_item(ui, app, "Master Control Relay (MCR)", ActiveTool::MasterRelay);
                    popup_tool_item(ui, app, "Parallel Branch", ActiveTool::Branch);
                    popup_tool_item(ui, app, "Short Circuit Wire", ActiveTool::Wire);
                    popup_tool_item(ui, app, "Open Circuit", ActiveTool::OpenCircuit);
                },
            );

            // 2. IEC Function Blocks (Timers & Counters)
            category_menu(
                ui,
                VectorIcon::Timer,
                "IEC Timers & Counters",
                palette.accent_ok,
                palette,
                |ui| {
                    popup_tool_item(ui, app, "TON — Delay On Timer", ActiveTool::Ton);
                    popup_tool_item(ui, app, "TOF — Delay Off Timer", ActiveTool::Tof);
                    popup_tool_item(ui, app, "TP — Pulse Timer", ActiveTool::Tp);
                    ui.separator();
                    popup_tool_item(ui, app, "CTU — Count Up", ActiveTool::Ctu);
                    popup_tool_item(ui, app, "CTD — Count Down", ActiveTool::Ctd);
                    popup_tool_item(ui, app, "CTUD — Up/Down Counter", ActiveTool::Ctud);
                },
            );

            // 3. IEC Edge Triggers (R_TRIG / F_TRIG)
            category_menu(
                ui,
                VectorIcon::PulseEdge,
                "IEC Edge Triggers",
                palette.accent_ok,
                palette,
                |ui| {
                    popup_tool_item(ui, app, "R_TRIG — Rising Edge FB", ActiveTool::RTrig);
                    popup_tool_item(ui, app, "F_TRIG — Falling Edge FB", ActiveTool::FTrig);
                },
            );

            // 4. IEC Math & Compare
            category_menu(
                ui,
                VectorIcon::FunctionBlock,
                "IEC Math & Compare",
                palette.accent_warn,
                palette,
                |ui| {
                    popup_tool_item(ui, app, "Move [:=]", ActiveTool::Move);
                    popup_tool_item(ui, app, "Compare [==, !=, >, <, >=, <=]", ActiveTool::Compare);
                    popup_tool_item(ui, app, "Add [+=]", ActiveTool::Add);
                    popup_tool_item(ui, app, "Sub [-=]", ActiveTool::Sub);
                    popup_tool_item(ui, app, "Mul [*=]", ActiveTool::Mul);
                    popup_tool_item(ui, app, "Div [/=]", ActiveTool::Div);
                    popup_tool_item(ui, app, "Mod [%=]", ActiveTool::Mod);
                },
            );

            // 5. IEC Program Flow
            category_menu(
                ui,
                VectorIcon::MoveUp,
                "IEC Program Flow",
                palette.accent_heading,
                palette,
                |ui| {
                    popup_tool_item(ui, app, "JMP — Jump to Label", ActiveTool::Jump);
                    popup_tool_item(ui, app, "LBL — Target Label", ActiveTool::Label);
                },
            );

            // 6. Hardware & Extensions
            category_menu(
                ui,
                VectorIcon::SerialComm,
                "Hardware & Extensions",
                palette.accent_heading,
                palette,
                |ui| {
                    popup_tool_item(ui, app, "A/D Converter Read (ADC)", ActiveTool::Adc);
                    popup_tool_item(ui, app, "Shift Register (S00)", ActiveTool::ShiftReg);
                    popup_tool_item(ui, app, "Shift Left (SHL)", ActiveTool::ShiftLeft);
                    popup_tool_item(ui, app, "Shift Right (SHR)", ActiveTool::ShiftRight);
                    ui.separator();
                    popup_tool_item(ui, app, "UART Send (TX)", ActiveTool::UartSend);
                    popup_tool_item(ui, app, "UART Recv (RX)", ActiveTool::UartRecv);
                },
            );
        });
    });
}

fn popup_tool_item(ui: &mut Ui, app: &mut CanvasApp, label: &str, tool: ActiveTool) {
    let is_sel = app.active_tool == tool;
    let resp = ui.add(egui::Button::new(RichText::new(label)).selected(is_sel).sense(Sense::click_and_drag()))
        .on_hover_text("Click to select tool, or drag onto canvas");

    if resp.drag_started() || (resp.dragged() && app.canvas.dragging_tool.is_none()) {
        app.canvas.start_tool_drag(tool, label);
    }
    if resp.clicked() {
        if app.active_tool == tool {
            app.active_tool = ActiveTool::Select;
        } else {
            app.active_tool = tool;
        }
        ui.close();
    }
}

fn category_menu(
    ui: &mut Ui,
    icon: VectorIcon,
    label: &str,
    accent: Color32,
    palette: crate::theme::ThemePalette,
    add_contents: impl FnOnce(&mut Ui),
) {
    let resp = modern_component_button(ui, icon, label, accent, false, palette);
    // Draw small right-side dropdown chevron
    let rect = resp.rect;
    let cx = rect.max.x - 11.0;
    let cy = rect.center().y;
    let s = Stroke::new(1.4, palette.rung_text_dim);
    ui.painter()
        .line_segment([pos2(cx - 3.5, cy - 1.5), pos2(cx, cy + 2.5)], s);
    ui.painter()
        .line_segment([pos2(cx, cy + 2.5), pos2(cx + 3.5, cy - 1.5)], s);

    egui::Popup::menu(&resp).show(add_contents);
}
