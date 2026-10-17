use egui::{RichText, Ui};
use my_canvas_core::SchematicStyle;
use crate::app::CanvasApp;
use crate::theme::AppTheme;

pub fn render(ui: &mut Ui, app: &mut CanvasApp) {
    egui::MenuBar::new().ui(ui, |ui| {
        // --- FILE MENU ---
        ui.menu_button("File", |ui| {
            if ui.button("New Program (Ctrl+N)").clicked() {
                app.new_program();
                ui.close();
            }
            if ui.button("Open Program (.il, .json)...").clicked() {
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
                ];
                for (label, path_str) in examples {
                    if ui.button(label).clicked() {
                        let content = std::fs::read_to_string(path_str)
                            .or_else(|_| {
                                if let Ok(exe) = std::env::current_exe() {
                                    if let Some(parent) = exe.parent() {
                                        if let Ok(c) = std::fs::read_to_string(parent.join(path_str)) {
                                            return Ok(c);
                                        }
                                        if let Ok(c) = std::fs::read_to_string(parent.join("../..").join(path_str)) {
                                            return Ok(c);
                                        }
                                    }
                                }
                                Err(std::io::Error::new(std::io::ErrorKind::NotFound, "not found"))
                            });
                        if let Ok(c) = content {
                            let _ = app.doc.load_from_il_str(&c);
                            app.doc.file_path = Some(path_str.to_string());
                        }
                        ui.close();
                    }
                }
            });
            ui.separator();
            if ui.button("Save Program (.il)...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("IEC 61131-3 Instruction List (*.il)", &["il"])
                    .save_file()
                {
                    let il_text = app.doc.export_to_il_str();
                    let _ = std::fs::write(path, il_text);
                }
                ui.close();
            }
            if ui.button("Export IEC 61131-3 IL (.il)...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("IEC 61131-3 Instruction List (*.il)", &["il"])
                    .save_file()
                {
                    let il_text = app.doc.export_to_il_str();
                    let _ = std::fs::write(path, il_text);
                }
                ui.close();
            }
            if ui.button("Import IEC 61131-3 IL (.il)...").clicked() {
                let mut dialog = rfd::FileDialog::new()
                    .add_filter("IEC 61131-3 Instruction List (*.il, *.ld)", &["il", "ld"]);
                let example_dir = std::path::Path::new("example");
                if example_dir.exists() {
                    dialog = dialog.set_directory(example_dir);
                }
                if let Some(path) = dialog.pick_file() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let _ = app.doc.load_from_il_str(&content);
                        app.doc.file_path = Some(path.display().to_string());
                    }
                }
                ui.close();
            }
            if ui.button("Save As (.json)...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("uPLC JSON Project (*.json)", &["json"])
                    .save_file()
                {
                    if let Ok(json) = serde_json::to_string_pretty(&app.doc) {
                        let _ = std::fs::write(path, json);
                    }
                }
                ui.close();
            }
            ui.separator();
            if ui.button("Exit").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
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
            let sim_label = if app.doc.simulator.is_running() {
                "■ Stop Simulation (F5)"
            } else {
                "▶ Start Simulation (F5)"
            };
            if ui.button(sim_label).clicked() {
                app.doc.toggle_simulation();
                ui.close();
            }
            if ui.button("⏭ Single Scan Step (F6)").clicked() {
                app.doc.single_step_simulation(app.doc.simulator.cycle_time_ms);
                ui.close();
            }
            if ui.button("↺ Reset Simulation (Shift+F5)").clicked() {
                app.doc.reset_simulation();
                ui.close();
            }
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
