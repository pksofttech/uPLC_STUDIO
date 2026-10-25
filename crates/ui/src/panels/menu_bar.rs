use egui::{RichText, Ui};
use my_canvas_core::SchematicStyle;
use crate::app::CanvasApp;
use crate::theme::AppTheme;

pub fn render(ui: &mut Ui, app: &mut CanvasApp) {
    egui::MenuBar::new().ui(ui, |ui| {
        // --- FILE MENU ---
        ui.menu_button("File", |ui| {
            if ui.button("New Program (Ctrl+N)").clicked() {
                app.request_new_program();
                ui.close();
            }
            if ui.button("Open Program (Ctrl+O)...").clicked() {
                app.request_open_program();
                ui.close();
            }
            ui.menu_button("Open Recent", |ui| {
                if app.recent_files.is_empty() {
                    ui.label(RichText::new("(no recent files)").weak().italics());
                } else {
                    let mut chosen: Option<String> = None;
                    for (i, path) in app.recent_files.iter().enumerate() {
                        let name = std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(path);
                        if ui
                            .button(format!("{}. {}", i + 1, name))
                            .on_hover_text(path.as_str())
                            .clicked()
                        {
                            chosen = Some(path.clone());
                        }
                    }
                    ui.separator();
                    if ui.button("Clear Recent Files").clicked() {
                        app.clear_recent_files();
                        ui.close();
                    }
                    if let Some(path) = chosen {
                        app.request_open_recent(path);
                        ui.close();
                    }
                }
            });
            ui.menu_button("Load Example (.il)", |ui| {
                let examples = [
                    ("01 - Motor Start/Stop (Latching)", "example/01_motor_start_stop.il"),
                    ("02 - Star-Delta Starter (Timer)", "example/02_star_delta_timer.il"),
                    ("03 - Traffic Light Controller", "example/03_traffic_light.il"),
                    ("04 - Conveyor Counter & Batch Pack", "example/04_counter_batch_pack.il"),
                    ("05 - Automatic Sliding Door", "example/05_auto_door_control.il"),
                    ("06 - Register Math & Compare", "example/06_math_compare_analog.il"),
                    ("07 - All IEC Components (Comprehensive Test)", "example/07_all_iec_components.il"),
                    ("08 - RP2040 Demo LED Blink (GP25)", "example/08_rp2040_demo_led_blink.il"),
                    ("09 - Modbus RTU Master (RS-485)", "example/09_modbus_rtu_master.il"),
                    ("10 - RP2040 Automatic I/O Test", "example/10_auto_io_test.il"),
                ];
                for (label, path_str) in examples {
                    if ui.button(label).clicked() {
                        app.request_load_example(path_str.to_string());
                        ui.close();
                    }
                }
            });
            ui.separator();
            if ui.button("Save Program (Ctrl+S)").clicked() {
                app.save_or_save_as();
                ui.close();
            }
            if ui.button("Save Program As (.il)...").clicked() {
                app.save_program_dialog();
                ui.close();
            }
            if ui.button("Save As (.json)...").clicked() {
                app.save_as_json_dialog();
                ui.close();
            }
            if ui.button("Export IEC 61131-3 IL (.il)...").clicked() {
                app.export_il_dialog();
                ui.close();
            }
            if ui.button("Import IEC 61131-3 IL (.il)...").clicked() {
                app.request_import_il();
                ui.close();
            }
            if ui.button("📥 Upload Program from Connected PLC...").clicked() {
                app.request_upload_from_plc();
                ui.close();
            }
            ui.separator();
            if ui.button("Exit").clicked() {
                app.request_exit(ui.ctx());
                ui.close();
            }
        });

        // --- EDIT MENU ---
        ui.menu_button("Edit", |ui| {
            if ui
                .add_enabled(app.doc.can_undo(), egui::Button::new("Undo (Ctrl+Z)"))
                .clicked()
            {
                app.doc.undo();
                ui.close();
            }
            if ui
                .add_enabled(app.doc.can_redo(), egui::Button::new("Redo (Ctrl+Y)"))
                .clicked()
            {
                app.doc.redo();
                ui.close();
            }
            ui.separator();
            let has_sel = !app.doc.selected_ids().is_empty();
            if ui.add_enabled(has_sel, egui::Button::new("Cut Element (Ctrl+X)")).clicked() {
                app.cut_selected_elements(ui.ctx());
                ui.close();
            }
            if ui.add_enabled(has_sel, egui::Button::new("Copy Element (Ctrl+C)")).clicked() {
                app.copy_selected_elements(ui.ctx());
                ui.close();
            }
            if ui
                .add_enabled(!app.clipboard.is_empty(), egui::Button::new("Paste Element (Ctrl+V)"))
                .clicked()
            {
                app.paste_clipboard_elements(None);
                ui.close();
            }
            ui.separator();
            if ui.button("Find / Replace... (Ctrl+F)").clicked() {
                app.open_find();
                ui.close();
            }
            ui.separator();
            if ui.button("+ New Network").clicked() {
                app.doc.add_rung();
                ui.close();
            }
            if ui.button("Copy Network").clicked() {
                app.copy_selected_network();
                ui.close();
            }
            if ui.button("Paste Network Below").clicked() {
                app.paste_network_below();
                ui.close();
            }
            ui.separator();
            if ui.button("Delete Selected Element (Del)").clicked() {
                app.doc.delete_selected_element();
                ui.close();
            }
            if ui.button("Delete Selected Network").clicked() {
                if let Some(idx) = app.doc.selected_rung_idx {
                    app.doc.delete_rung(idx);
                }
                ui.close();
            }
        });

        // --- VIEW MENU ---
        ui.menu_button("View", |ui| {
            ui.menu_button("Theme (Modern / Dark / Light)", |ui| {
                for t in AppTheme::ALL {
                    if ui.selectable_label(app.theme == t, t.label()).clicked() {
                        app.set_theme(t);
                        ui.close();
                    }
                }
            });
            ui.menu_button("Schematic Style", |ui| {
                for st in SchematicStyle::ALL {
                    let is_cur = app.doc.schematic_style == st;
                    if ui.selectable_label(is_cur, st.label()).on_hover_text(st.description()).clicked() {
                        app.doc.set_schematic_style(st);
                        app.save_settings();
                        ui.close();
                    }
                }
            });
            ui.menu_button("Window Size", |ui| {
                let current_size_label = match (app.window_width, app.window_height) {
                    (Some(w), Some(h)) => format!("Current: {:.0} × {:.0}", w, h),
                    _ => "Current: 1280 × 720".to_string(),
                };
                ui.label(RichText::new(current_size_label).weak());
                ui.separator();

                if ui.button("1280 × 720 (Standard HD)").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1280.0, 720.0)));
                    app.window_width = Some(1280.0);
                    app.window_height = Some(720.0);
                    app.window_maximized = false;
                    app.save_settings();
                    ui.close();
                }
                if ui.button("1440 × 900 (WXGA+)").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1440.0, 900.0)));
                    app.window_width = Some(1440.0);
                    app.window_height = Some(900.0);
                    app.window_maximized = false;
                    app.save_settings();
                    ui.close();
                }
                if ui.button("1600 × 900 (HD+)").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1600.0, 900.0)));
                    app.window_width = Some(1600.0);
                    app.window_height = Some(900.0);
                    app.window_maximized = false;
                    app.save_settings();
                    ui.close();
                }
                if ui.button("1920 × 1080 (Full HD)").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1920.0, 1080.0)));
                    app.window_width = Some(1920.0);
                    app.window_height = Some(1080.0);
                    app.window_maximized = false;
                    app.save_settings();
                    ui.close();
                }
                ui.separator();
                let max_text = if app.window_maximized {
                    "Restore Window Size"
                } else {
                    "Maximize Window"
                };
                if ui.button(max_text).clicked() {
                    let new_max = !app.window_maximized;
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(new_max));
                    app.window_maximized = new_max;
                    app.save_settings();
                    ui.close();
                }
            });
            ui.separator();
            if ui.button("Reset Zoom & Pan").clicked() {
                app.canvas.reset_view();
                ui.close();
            }
            if ui
                .checkbox(&mut app.show_il_code, "Show IEC 61131-3 IL Panel")
                .changed()
            {
                app.save_settings();
            }
        });

        // --- HARDWARE MENU ---
        ui.menu_button("Hardware", |ui| {
            ui.label(RichText::new("Target IEC 61131-3 Hardware Platform:").weak().size(11.0));
            ui.separator();
            let targets = [
                "RP2040 Micro-VM (Custom Firmware)",
                "IEC 61131-3 Standard PLC",
                "OpenPLC Runtime (Linux / Windows)",
                "OpenPLC MicroPLC (IEC 61131-3)",
                "OpenPLC Raspberry Pi",
                "OpenPLC ESP32 (IEC 61131-3)",
                "OpenPLC STM32 (IEC 61131-3)",
                "Generic Modbus RTU / TCP Device",
                "Simulation / Virtual PLC",
            ];
            for target in targets {
                let is_cur = app.doc.program.hardware == target;
                if ui.selectable_label(is_cur, target).clicked() {
                    app.doc.program.hardware = target.to_string();
                    app.save_settings();
                    ui.close();
                }
            }
            if !targets.contains(&app.doc.program.hardware.as_str()) {
                ui.separator();
                ui.label(RichText::new(format!("Current: {}", app.doc.program.hardware)).italics().size(11.0));
            }
            ui.separator();
            if ui.button("🔧 Configure RP2040 Hardware I/O Pins...").clicked() {
                app.show_update_plc = true;
                app.update_plc_tab = 2;
                ui.close();
            }
            if ui.button("⚡ Update to PLC (Compile IL → Bytecode)...").clicked() {
                app.show_update_plc = true;
                ui.close();
            }
            if ui.button("🔥 Flash MCU Firmware (USB)...").clicked() {
                app.firmware_flasher.show_window = true;
                ui.close();
            }
        });

        // --- TOOLS MENU ---
        ui.menu_button("Tools", |ui| {
            if ui.button("🔧 Configure RP2040 Hardware I/O Pins...").clicked() {
                app.show_update_plc = true;
                app.update_plc_tab = 2;
                ui.close();
            }
            if ui.button("⚡ Update to PLC (Compile IL → Bytecode)...").clicked() {
                app.show_update_plc = true;
                ui.close();
            }
            if ui.button("🔥 Flash MCU Firmware (USB)...").clicked() {
                app.firmware_flasher.show_window = true;
                ui.close();
            }
            if ui.button("Preview / Export IEC 61131-3 IL Code").clicked() {
                app.show_code_export = true;
                ui.close();
            }
            ui.separator();
            let is_hw = app.online_target_hw;
            let is_running = if is_hw {
                app.online_mode && app.plc_remote_running
            } else {
                app.doc.simulator.is_running()
            };
            let sim_label = if is_hw {
                if is_running {
                    "■ Stop PLC Execution (F5)"
                } else if app.online_mode {
                    "▶ Start / RUN PLC (F5)"
                } else {
                    "▶ Connect & RUN PLC (F5)"
                }
            } else if is_running {
                "■ Stop Simulation (F5)"
            } else {
                "▶ Start Simulation (F5)"
            };
            if ui.button(sim_label).clicked() {
                app.toggle_run_mode();
                ui.close();
            }
            let step_label = if is_hw {
                "⏭ Single Scan Step PLC (F6)"
            } else {
                "⏭ Single Scan Step (F6)"
            };
            if ui.button(step_label).clicked() {
                app.single_step_mode();
                ui.close();
            }
            let reset_label = if is_hw {
                "↺ Reset / Halt PLC (Shift+F5)"
            } else {
                "↺ Reset Simulation (Shift+F5)"
            };
            if ui.button(reset_label).clicked() {
                app.reset_run_mode();
                ui.close();
            }
            ui.separator();
            let online_label = if app.online_mode {
                "🛑 Stop Online Monitoring (Ctrl+M / F7)"
            } else {
                "🌐 Start Online Monitoring (Ctrl+M / F7)"
            };
            if ui.button(online_label).clicked() {
                app.toggle_online_mode();
                ui.close();
            }
            ui.menu_button("Online Target Device", |ui| {
                if ui.selectable_label(app.online_target_hw, "🔌 Hardware (RP2040 Micro-VM)").clicked() {
                    app.set_online_target_hw(true);
                    ui.close();
                }
                if ui.selectable_label(!app.online_target_hw, "💻 Virtual Simulator").clicked() {
                    app.set_online_target_hw(false);
                    ui.close();
                }
            });
        });

        // --- OPTIONS MENU ---
        ui.menu_button("Options", |ui| {
            ui.menu_button("Theme (Modern / Dark / Light)", |ui| {
                for t in AppTheme::ALL {
                    if ui.selectable_label(app.theme == t, t.label()).clicked() {
                        app.set_theme(t);
                        ui.close();
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Cycle Time (ms):");
                if ui
                    .add(egui::DragValue::new(&mut app.doc.simulator.cycle_time_ms).range(1..=1000))
                    .changed()
                {
                    app.save_settings();
                }
            });
            if ui.button("Settings & IO Ports...").clicked() {
                app.show_io_config = true;
                ui.close();
            }
        });

        // --- HELP MENU ---
        ui.menu_button("Help", |ui| {
            if ui.button("🔄 Check for Updates...").clicked() {
                app.updater.show_window = true;
                if matches!(app.updater.status, crate::updater::UpdateStatus::Idle) {
                    app.updater.check_for_updates(ui.ctx().clone());
                }
                ui.close();
            }
            ui.separator();
            if ui.button("About LaduinoPLC").clicked() {
                app.show_about = true;
                ui.close();
            }
        });

        if let Some(new_ver) = app.updater.has_pending_update().map(|s| s.to_string()) {
            ui.separator();
            let badge_btn = egui::Button::new(
                RichText::new(format!("🟢 Update v{} Available", new_ver))
                    .size(11.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            )
            .fill(egui::Color32::from_rgb(24, 130, 90))
            .corner_radius(egui::CornerRadius::same(4));

            if ui
                .add(badge_btn)
                .on_hover_text("Click to open Software Update & Release Manager")
                .clicked()
            {
                app.updater.show_window = true;
            }
        }
    });
}
