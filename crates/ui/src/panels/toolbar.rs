use crate::app::CanvasApp;
use crate::icons::{modern_tool_button, VectorIcon};
use crate::theme::{AppTheme, ThemePalette};
use crate::tools::ActiveTool;
use egui::{CornerRadius, RichText, Stroke, Ui};
use my_canvas_core::SchematicStyle;

fn toolbar_group<R>(
    ui: &mut Ui,
    palette: ThemePalette,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let frame = egui::Frame::new()
        .fill(palette.pill_bg)
        .stroke(Stroke::new(1.0, palette.rung_badge_border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(5, 3));

    frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                add_contents(ui)
            })
            .inner
        })
        .inner
}

fn toolbar_divider(ui: &mut Ui, palette: ThemePalette) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 21.0), egui::Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.center().x, rect.min.y + 2.0),
            egui::pos2(rect.center().x, rect.max.y - 2.0),
        ],
        Stroke::new(1.0, palette.rung_badge_border),
    );
}

pub fn render(ui: &mut Ui, app: &mut CanvasApp) {
    let palette = app.theme.palette();
    let avail_width = ui.available_width();
    let is_compact = avail_width < 1260.0;
    let is_ultra_compact = avail_width < 980.0;

    egui::ScrollArea::horizontal()
        .id_salt("top_toolbar_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 2.0);

                // =========================================================================
                // Group 1: Project & Edit Tools (File, Undo/Redo, Selection)
                // =========================================================================
                toolbar_group(ui, palette, |ui| {
                    // New File
                    let (new_lbl, new_w) = if is_compact { ("", 28.0) } else { ("New", 56.0) };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::NewFile),
                        new_lbl,
                        palette.accent_heading,
                        false,
                        true,
                        false,
                        new_w,
                        palette,
                    )
                    .on_hover_text("New Ladder Project (Ctrl+N)")
                    .clicked()
                    {
                        app.request_new_program();
                    }

                    // Open File
                    let (open_lbl, open_w) = if is_compact { ("", 28.0) } else { ("Open", 58.0) };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::OpenFile),
                        open_lbl,
                        palette.accent_heading,
                        false,
                        true,
                        false,
                        open_w,
                        palette,
                    )
                    .on_hover_text("Open IEC 61131-3 IL (.il) or Project (.json)")
                    .clicked()
                    {
                        app.request_open_program();
                    }

                    // Save File
                    let (save_lbl, save_w) = if is_compact { ("", 28.0) } else { ("Save", 58.0) };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::SaveFile),
                        save_lbl,
                        palette.accent_ok,
                        false,
                        true,
                        false,
                        save_w,
                        palette,
                    )
                    .on_hover_text("Save IEC 61131-3 IL (.il)")
                    .clicked()
                    {
                        app.save_program_dialog();
                    }

                    toolbar_divider(ui, palette);

                    // Undo
                    let (undo_lbl, undo_w) = if is_compact { ("", 28.0) } else { ("Undo", 58.0) };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::Undo),
                        undo_lbl,
                        palette.accent_warn,
                        false,
                        app.doc.can_undo(),
                        false,
                        undo_w,
                        palette,
                    )
                    .on_hover_text("Undo last circuit edit (Ctrl+Z)")
                    .clicked()
                    {
                        app.doc.undo();
                    }

                    // Redo
                    let (redo_lbl, redo_w) = if is_compact { ("", 28.0) } else { ("Redo", 58.0) };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::Redo),
                        redo_lbl,
                        palette.accent_heading,
                        false,
                        app.doc.can_redo(),
                        false,
                        redo_w,
                        palette,
                    )
                    .on_hover_text("Redo circuit edit (Ctrl+Y)")
                    .clicked()
                    {
                        app.doc.redo();
                    }

                    toolbar_divider(ui, palette);

                    // Select / Pointer Tool
                    let is_sel = app.active_tool == ActiveTool::Select;
                    let (sel_lbl, sel_w) = if is_compact { ("", 28.0) } else { ("Select", 62.0) };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::CursorSelect),
                        sel_lbl,
                        palette.accent_heading,
                        is_sel,
                        true,
                        false,
                        sel_w,
                        palette,
                    )
                    .on_hover_text("Select & Move Tool (Click or drag box to select / Press Esc)")
                    .clicked()
                    {
                        app.active_tool = ActiveTool::Select;
                    }
                });

                // =========================================================================
                // Group 2: Simulation / PLC Control Station (Run, Step, Stop, Reset)
                // =========================================================================
                let is_hw = app.online_target_hw;
                let is_hw_conn = !is_hw || app.is_hw_online_connected();
                let (is_running, is_stopped, is_step) = if is_hw {
                    let r = is_hw_conn && app.plc_remote_running;
                    (r, !r, !r)
                } else {
                    (app.doc.simulator.is_running(), app.doc.simulator.is_stopped(), app.doc.simulator.is_step())
                };

                toolbar_group(ui, palette, |ui| {
                    // Start / RUN Button
                    let (start_label, start_w) = if is_running {
                        ("RUN", if is_compact { 48.0 } else { 62.0 })
                    } else if is_compact {
                        ("", 28.0)
                    } else {
                        ("Start", 60.0)
                    };

                    let run_hover = if is_hw {
                        if app.online_mode && !is_hw_conn {
                            "Waiting for RP2040 USB Connection (Auto-reconnecting...)"
                        } else if is_running {
                            "Halt / STOP Hardware PLC Execution (F5)"
                        } else if app.online_mode {
                            "Start / RUN Hardware PLC Execution (F5)"
                        } else {
                            "Connect & Start Hardware PLC Execution (F5)"
                        }
                    } else {
                        "Start / Continuous Simulation (F5)"
                    };

                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::Play),
                        start_label,
                        palette.accent_ok,
                        is_running,
                        true,
                        false,
                        start_w,
                        palette,
                    )
                    .on_hover_text(run_hover)
                    .clicked()
                    {
                        app.toggle_run_mode();
                    }

                    // Step Button
                    let (step_lbl, step_w) = if is_compact { ("", 28.0) } else { ("Step", 56.0) };
                    let step_hover = if is_hw {
                        "Execute Single Scan Step on Hardware PLC (F6)"
                    } else {
                        "Execute Single Scan Step (F6)"
                    };

                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::Step),
                        step_lbl,
                        palette.accent_warn,
                        is_step,
                        true,
                        false,
                        step_w,
                        palette,
                    )
                    .on_hover_text(step_hover)
                    .clicked()
                    {
                        app.single_step_mode();
                    }

                    // Stop Button
                    let (stop_lbl, stop_w) = if is_compact { ("", 28.0) } else { ("Stop", 56.0) };
                    let stop_hover = if is_hw {
                        "Halt Hardware PLC Scan & Safe Output Shutdown"
                    } else {
                        "Stop Simulation & Safe Output Shutdown"
                    };
                    let stop_enabled = if is_hw {
                        app.online_mode && is_running
                    } else {
                        !is_stopped
                    };

                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::Stop),
                        stop_lbl,
                        palette.accent_danger,
                        false,
                        stop_enabled,
                        false,
                        stop_w,
                        palette,
                    )
                    .on_hover_text(stop_hover)
                    .clicked()
                    {
                        app.stop_run_mode();
                    }

                    // Reset Button
                    let (reset_lbl, reset_w) = if is_compact { ("", 28.0) } else { ("Reset", 58.0) };
                    let reset_hover = if is_hw {
                        "Reset / Halt Hardware PLC (Shift+F5)"
                    } else {
                        "Reset Simulation & Clear Memory (Shift+F5)"
                    };

                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::Reset),
                        reset_lbl,
                        palette.accent_heading,
                        false,
                        true,
                        false,
                        reset_w,
                        palette,
                    )
                    .on_hover_text(reset_hover)
                    .clicked()
                    {
                        app.reset_run_mode();
                    }

                    toolbar_divider(ui, palette);

                    // Simulation Speed Multiplier (Virtual Simulator only)
                    ui.add_enabled_ui(!is_hw, |ui| {
                        egui::ComboBox::from_id_salt("toolbar_sim_speed_combo")
                            .width(42.0)
                            .selected_text(format!("x{}", app.doc.simulator.speed_multiplier))
                            .show_ui(ui, |ui| {
                                for mult in [1u32, 2, 5, 10] {
                                    ui.selectable_value(
                                        &mut app.doc.simulator.speed_multiplier,
                                        mult,
                                        format!("x{}", mult),
                                    );
                                }
                            })
                            .response
                            .on_hover_text(if is_hw {
                                "Simulation speed (Fixed 10ms hardware scan cycle on RP2040)"
                            } else {
                                "Virtual simulation speed multiplier"
                            });
                    });

                    // Digital Hardware Scan Timer / Counter Display
                    if !is_ultra_compact {
                        let lcd_frame = egui::Frame::new()
                            .fill(palette.canvas_bg)
                            .stroke(Stroke::new(1.0, palette.rung_badge_border))
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(6, 4));

                        let (time_text, text_color) = if is_hw {
                            if app.online_mode && !is_hw_conn {
                                ("RECONN...".to_string(), palette.accent_warn)
                            } else {
                                let scans = app.latest_snapshot.as_ref().map_or(0, |s| s.scan_count);
                                (
                                    format!("{:>6} scans", scans),
                                    if is_running { palette.accent_ok } else { palette.accent_heading },
                                )
                            }
                        } else {
                            (
                                app.doc.simulator.formatted_sim_time(),
                                if is_running { palette.accent_ok } else { palette.accent_heading },
                            )
                        };

                        lcd_frame.show(ui, |ui| {
                            ui.label(
                                RichText::new(time_text)
                                    .monospace()
                                    .size(11.5)
                                    .color(text_color),
                            );
                        });
                    }
                });

                // =========================================================================
                // Group 3: PLC & Online (Live Telemetry, Target Selector, Update PLC)
                // =========================================================================
                toolbar_group(ui, palette, |ui| {
                    let is_online = app.online_mode;
                    let (online_btn_text, online_btn_w) = if is_online {
                        (if is_compact { "Online" } else { "🟢 Online" }, if is_compact { 64.0 } else { 82.0 })
                    } else {
                        (if is_compact { "Online" } else { "🌐 Go Online" }, if is_compact { 64.0 } else { 92.0 })
                    };
                    let online_btn_color = if is_online { palette.accent_ok } else { palette.accent_heading };

                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::SerialComm),
                        online_btn_text,
                        online_btn_color,
                        is_online,
                        true,
                        false,
                        online_btn_w,
                        palette,
                    )
                    .on_hover_text(if is_online {
                        "Online Monitoring ACTIVE: Real-time status from PLC / Simulator (Ctrl+M / F7 to stop)"
                    } else {
                        "Start Online Monitoring: Live debug Process Image and ACC in real time (Ctrl+M / F7)"
                    })
                    .clicked()
                    {
                        app.toggle_online_mode();
                    }

                    toolbar_divider(ui, palette);

                    // Target selector: Hardware (RP2040) vs Local Simulator
                    let cur_target_label = if is_compact {
                        if app.online_target_hw { "🔌 HW" } else { "💻 Sim" }
                    } else {
                        if app.online_target_hw { "🔌 RP2040" } else { "💻 Simulator" }
                    };
                    let combo_w = if is_compact { 68.0 } else { 96.0 };
                    egui::ComboBox::from_id_salt("toolbar_online_target_combo")
                        .width(combo_w)
                        .selected_text(cur_target_label)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(app.online_target_hw, "🔌 Hardware (RP2040 USB)")
                                .on_hover_text("Monitor & control physical RP2040 Micro-VM over USB CDC Serial")
                                .clicked()
                            {
                                app.set_online_target_hw(true);
                                ui.close();
                            }
                            if ui
                                .selectable_label(!app.online_target_hw, "💻 Virtual Simulator")
                                .on_hover_text("Monitor & control internal local PLC software simulation engine")
                                .clicked()
                            {
                                app.set_online_target_hw(false);
                                ui.close();
                            }
                        });

                    toolbar_divider(ui, palette);

                    // Update to PLC (Compile IL -> Bytecode & Upload to MCU VM)
                    let (plc_lbl, plc_w) = if is_compact { ("Update", 68.0) } else { ("Update PLC", 92.0) };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::SerialComm),
                        plc_lbl,
                        palette.accent_warn,
                        app.show_update_plc,
                        true,
                        false,
                        plc_w,
                        palette,
                    )
                    .on_hover_text("Compile IL to 32-bit Micro-VM Bytecode & Update to PLC via Serial/USB")
                    .clicked()
                    {
                        app.show_update_plc = !app.show_update_plc;
                    }
                });

                // =========================================================================
                // Group 4: View & Tools (Code Export, Schematic Style, Theme, Config)
                // =========================================================================
                toolbar_group(ui, palette, |ui| {
                    // IEC 61131-3 IL Code Preview & Export
                    let (code_lbl, code_w) = if is_compact { ("", 28.0) } else { ("Code", 58.0) };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::ExportCode),
                        code_lbl,
                        palette.accent_ok,
                        app.show_code_export,
                        true,
                        false,
                        code_w,
                        palette,
                    )
                    .on_hover_text("Preview & Export IEC 61131-3 IL Code")
                    .clicked()
                    {
                        app.show_code_export = !app.show_code_export;
                    }

                    toolbar_divider(ui, palette);

                    // Schematic Style Selector
                    egui::ComboBox::from_id_salt("toolbar_schematic_style_combo")
                        .selected_text(app.doc.schematic_style.short_name())
                        .width(if is_compact { 68.0 } else { 82.0 })
                        .show_ui(ui, |ui| {
                            for st in SchematicStyle::ALL {
                                let is_cur = app.doc.schematic_style == st;
                                if ui
                                    .selectable_label(is_cur, st.label())
                                    .on_hover_text(st.description())
                                    .clicked()
                                {
                                    app.doc.set_schematic_style(st);
                                    app.save_settings();
                                }
                            }
                        });

                    toolbar_divider(ui, palette);

                    // Compact Modern Theme Selector
                    egui::ComboBox::from_id_salt("toolbar_theme_combo")
                        .selected_text(format!("🎨 {}", app.theme.label()))
                        .width(if is_compact { 72.0 } else { 85.0 })
                        .show_ui(ui, |ui| {
                            for t in AppTheme::ALL {
                                if ui.selectable_label(app.theme == t, t.label()).clicked() {
                                    app.set_theme(t);
                                }
                            }
                        });

                    toolbar_divider(ui, palette);

                    // Settings & Hardware I/O Configuration
                    let (cfg_lbl, cfg_w) = if is_compact { ("", 28.0) } else { ("Config", 62.0) };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::ConfigGear),
                        cfg_lbl,
                        palette.accent_heading,
                        app.show_io_config,
                        true,
                        false,
                        cfg_w,
                        palette,
                    )
                    .on_hover_text("Settings, Theme & Hardware Configuration")
                    .clicked()
                    {
                        app.show_io_config = !app.show_io_config;
                    }
                });
            });
        });
}
