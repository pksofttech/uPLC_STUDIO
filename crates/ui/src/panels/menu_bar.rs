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
            if ui.button("Save Program (Ctrl+S)...").clicked() {
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
                app.import_il_dialog();
                ui.close();
            }
            if ui.button("📥 Upload Program from Connected PLC...").clicked() {
                app.transmit_bytecode_to_plc(my_canvas_core::PlcSerialCommand::UploadProgram);
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
            if ui.button("+ New Network").clicked() {
                app.doc.add_rung();
                ui.close();
            }
            if ui.button("Copy Network").clicked() {
                app.copied_rung_idx = app.doc.selected_rung_idx;
                ui.close();
            }
            if ui.button("Paste Network Below").clicked() {
                if let Some(idx) = app.doc.selected_rung_idx {
                    let src = app.copied_rung_idx.unwrap_or(idx);
                    app.doc.copy_rung_at(src, idx + 1);
                }
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
            if ui.button("⚡ Update to PLC (Compile IL → Bytecode)...").clicked() {
                app.show_update_plc = true;
                ui.close();
            }
        });

        // --- TOOLS MENU ---
        ui.menu_button("Tools", |ui| {
            if ui.button("⚡ Update to PLC (Compile IL → Bytecode)...").clicked() {
                app.show_update_plc = true;
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
            if ui.button("About uPLC Studio").clicked() {
                app.show_about = true;
                ui.close();
            }
        });
    });
}
