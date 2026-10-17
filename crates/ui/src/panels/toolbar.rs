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
                        app.new_program();
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
                        let mut dialog = rfd::FileDialog::new()
                            .add_filter("IEC 61131-3 IL & Projects (*.il, *.json, *.ld)", &["il", "json", "ld"]);
                        let example_dir = std::path::Path::new("example");
                        if example_dir.exists() {
                            dialog = dialog.set_directory(example_dir);
                        }
                        if let Some(path) = dialog.pick_file() {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                                    if let Ok(doc) = serde_json::from_str(&content) {
                                        app.doc = doc;
                                    }
                                } else {
                                    let _ = app.doc.load_from_il_str(&content);
                                }
                                app.doc.file_path = Some(path.display().to_string());
                            }
                        }
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
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("IEC 61131-3 Instruction List (*.il)", &["il"])
                            .save_file()
                        {
                            let il_text = app.doc.export_to_il_str();
                            let _ = std::fs::write(path, il_text);
                        }
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
                // Group 2: Simulation Station (Run, Step, Stop, Reset, Speed, Timer)
                // =========================================================================
                let is_running = app.doc.simulator.is_running();
                let is_stopped = app.doc.simulator.is_stopped();
                let is_step = app.doc.simulator.is_step();

                toolbar_group(ui, palette, |ui| {
                    // Start / RUN Button
                    let (start_label, start_w) = if is_running {
                        ("RUN", if is_compact { 48.0 } else { 62.0 })
                    } else if is_compact {
                        ("", 28.0)
                    } else {
                        ("Start", 60.0)
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
                    .on_hover_text("Start / Continuous Simulation (F5)")
                    .clicked()
                    {
                        if is_running {
                            app.doc.stop_simulation();
                        } else {
                            app.doc.start_simulation();
                        }
                    }

                    // Step Button
                    let (step_lbl, step_w) = if is_compact { ("", 28.0) } else { ("Step", 56.0) };
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
                    .on_hover_text("Execute Single Scan Step (F6)")
                    .clicked()
                    {
                        app.doc.single_step_simulation(app.doc.simulator.cycle_time_ms);
                    }

                    // Stop Button
                    let (stop_lbl, stop_w) = if is_compact { ("", 28.0) } else { ("Stop", 56.0) };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::Stop),
                        stop_lbl,
                        palette.accent_danger,
                        false,
                        !is_stopped,
                        false,
                        stop_w,
                        palette,
                    )
                    .on_hover_text("Stop Simulation & Safe Output Shutdown")
                    .clicked()
                    {
                        app.doc.stop_simulation();
                    }

                    // Reset Button
                    let (reset_lbl, reset_w) = if is_compact { ("", 28.0) } else { ("Reset", 58.0) };
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
                    .on_hover_text("Reset Simulation & Clear Memory (Shift+F5)")
                    .clicked()
                    {
                        app.doc.reset_simulation();
                    }

                    toolbar_divider(ui, palette);

                    // Simulation Speed Multiplier
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
                        });

                    // Digital Hardware Scan Timer Display
                    if !is_ultra_compact {
                        let lcd_frame = egui::Frame::new()
                            .fill(palette.canvas_bg)
                            .stroke(Stroke::new(1.0, palette.rung_badge_border))
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(6, 4));

                        lcd_frame.show(ui, |ui| {
                            ui.label(
                                RichText::new(app.doc.simulator.formatted_sim_time())
                                    .monospace()
                                    .size(11.5)
                                    .color(if is_running {
                                        palette.accent_ok
                                    } else {
                                        palette.accent_heading
                                    }),
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
                    let cur_target_label = if app.online_target_hw { "🔌 HW" } else { "💻 Sim" };
                    egui::ComboBox::from_id_salt("toolbar_online_target_combo")
                        .width(if is_compact { 64.0 } else { 74.0 })
                        .selected_text(cur_target_label)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(app.online_target_hw, "🔌 Hardware (USB CDC)")
                                .on_hover_text("Monitor physical RP2040 Micro-VM over USB CDC Serial")
                                .clicked()
                            {
                                app.set_online_target_hw(true);
                            }
                            if ui
                                .selectable_label(!app.online_target_hw, "💻 Virtual Simulator")
                                .on_hover_text("Monitor internal local PLC software simulation engine")
                                .clicked()
                            {
                                app.set_online_target_hw(false);
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
