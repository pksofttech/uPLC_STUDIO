use egui::{RichText, Ui};
use my_canvas_core::SchematicStyle;
use crate::app::{CanvasApp, WorkMode};
use crate::theme::AppTheme;

pub fn render(ui: &mut Ui, app: &mut CanvasApp) {
    egui::MenuBar::new().ui(ui, |ui| {
        // --- FILE MENU ---
        ui.menu_button("File", |ui| {
            if ui.button("New (Ctrl+N)").clicked() {
                app.request_new_program();
                ui.close();
            }
            if ui.button("Open... (Ctrl+O)").clicked() {
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
                    ("11 - Drum Rhythm Sequencer Demo", "example/11_drum_rhythm_demo.il"),
                    ("12 - PID & Linear Scaling Process", "example/12_pid_scale_process.il"),
                    ("13 - Bitwise, Shift Register & UART", "example/13_bitwise_shift_uart.il"),
                    ("14 - Counter CTU/CTD, Compare & Reset Demo", "example/14_counter_up_down_demo.il"),
                    ("15 - CTUD Up/Down Parking Lot Demo", "example/15_ctud_parking_lot_demo.il"),
                    ("16 - Toggle Relay (Push-On / Push-Off) Demo", "example/16_toggle_relay_demo.il"),
                ];
                for (label, path_str) in examples {
                    if ui.button(label).clicked() {
                        app.request_load_example(path_str.to_string());
                        ui.close();
                    }
                }
            });
            ui.separator();
            if ui.button("Save (Ctrl+S)").clicked() {
                app.save_or_save_as();
                ui.close();
            }
            if ui.button("Save As (.il)...").clicked() {
                app.save_program_dialog();
                ui.close();
            }
            if ui.button("Save As (.json)...").clicked() {
                app.save_as_json_dialog();
                ui.close();
            }
            ui.separator();
            if ui.button("Import IL (.il)...").clicked() {
                app.request_import_il();
                ui.close();
            }
            if ui.button("Export IL (.il)...").clicked() {
                app.export_il_dialog();
                ui.close();
            }
            ui.separator();
            if ui.button("Settings...").clicked() {
                app.settings_tab = 0;
                app.show_io_config = true;
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
            let is_edit_mode = app.work_mode == WorkMode::Edit;
            if !is_edit_mode {
                if ui
                    .button(RichText::new("Switch to Edit Mode").strong())
                    .clicked()
                {
                    app.set_work_mode(WorkMode::Edit);
                    ui.close();
                }
                ui.separator();
            }
            if ui
                .add_enabled(is_edit_mode && app.doc.can_undo(), egui::Button::new("Undo (Ctrl+Z)"))
                .clicked()
            {
                app.doc.undo();
                ui.close();
            }
            if ui
                .add_enabled(is_edit_mode && app.doc.can_redo(), egui::Button::new("Redo (Ctrl+Y)"))
                .clicked()
            {
                app.doc.redo();
                ui.close();
            }
            ui.separator();
            let has_sel = !app.doc.selected_ids().is_empty();
            if ui
                .add_enabled(is_edit_mode && has_sel, egui::Button::new("Cut (Ctrl+X)"))
                .clicked()
            {
                app.cut_selected_elements(ui.ctx());
                ui.close();
            }
            if ui
                .add_enabled(has_sel, egui::Button::new("Copy (Ctrl+C)"))
                .clicked()
            {
                app.copy_selected_elements(ui.ctx());
                ui.close();
            }
            if ui
                .add_enabled(is_edit_mode && !app.clipboard.is_empty(), egui::Button::new("Paste (Ctrl+V)"))
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
            if ui.add_enabled(is_edit_mode, egui::Button::new("+ New Network")).clicked() {
                app.doc.add_rung();
                ui.close();
            }
            if ui.button("Copy Network").clicked() {
                app.copy_selected_network();
                ui.close();
            }
            if ui
                .add_enabled(is_edit_mode, egui::Button::new("Paste Network Below"))
                .clicked()
            {
                app.paste_network_below();
                ui.close();
            }
            ui.separator();
            if ui
                .add_enabled(is_edit_mode, egui::Button::new("Delete Element (Del)"))
                .clicked()
            {
                app.doc.delete_selected_element();
                ui.close();
            }
            if ui
                .add_enabled(is_edit_mode, egui::Button::new("Delete Network"))
                .clicked()
            {
                if let Some(idx) = app.doc.selected_rung_idx {
                    app.doc.delete_rung(idx);
                }
                ui.close();
            }
        });

        // --- VIEW MENU ---
        ui.menu_button("View", |ui| {
            ui.menu_button("Theme", |ui| {
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
                    if ui
                        .selectable_label(is_cur, st.label())
                        .on_hover_text(st.description())
                        .clicked()
                    {
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
            if ui.checkbox(&mut app.show_il_code, "Show IL Code Panel").changed() {
                app.save_settings();
            }
        });

        // --- PLC MENU ---
        ui.menu_button("PLC", |ui| {
            ui.menu_button("Target Hardware", |ui| {
                let targets = crate::target::HARDWARE_TARGETS;
                for target in targets {
                    let is_cur = app.doc.program.hardware == target;
                    if ui.selectable_label(is_cur, target).clicked() {
                        app.doc.program.hardware = target.to_string();
                        app.sync_doc_io_mappings_from_hw_config();
                        app.save_settings();
                        ui.close();
                    }
                }
                if !targets.contains(&app.doc.program.hardware.as_str()) {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("Current: {}", app.doc.program.hardware))
                            .italics()
                            .size(11.0),
                    );
                }
            });
            ui.separator();
            if ui
                .selectable_label(app.work_mode == WorkMode::Edit, "Edit Mode")
                .clicked()
            {
                app.set_work_mode(WorkMode::Edit);
                ui.close();
            }
            if ui
                .selectable_label(app.work_mode == WorkMode::Simulator, "Simulator Mode (F5)")
                .clicked()
            {
                app.set_work_mode(WorkMode::Simulator);
                ui.close();
            }
            if ui
                .selectable_label(app.work_mode == WorkMode::OnlineDebug, "Online PLC Debug (F7)")
                .clicked()
            {
                app.request_work_mode(WorkMode::OnlineDebug);
                ui.close();
            }
            ui.separator();
            let is_hw = app.work_mode == WorkMode::OnlineDebug;
            let is_running = if is_hw {
                app.online_mode && app.plc_remote_running
            } else {
                app.doc.simulator.is_running()
            };
            let run_label = if is_hw {
                if is_running {
                    "Stop Hardware PLC (F5)"
                } else {
                    "Run Hardware PLC (F5)"
                }
            } else if is_running {
                "Pause Simulation (F5)"
            } else {
                "Run Simulation (F5)"
            };
            if ui.button(run_label).clicked() {
                app.toggle_run_mode();
                ui.close();
            }
            if ui.button("Single Scan Step (F6)").clicked() {
                app.single_step_mode();
                ui.close();
            }
            if ui.button("Reset / Halt (Shift+F5)").clicked() {
                app.reset_run_mode();
                ui.close();
            }
            ui.separator();
            if ui.button("Update to PLC...").clicked() {
                app.show_update_plc = true;
                ui.close();
            }
            if ui.button("Read from PLC...").clicked() {
                app.request_upload_from_plc();
                ui.close();
            }
            if ui.button(io_pins_menu_label(app)).clicked() {
                app.settings_tab = 2;
                app.show_io_config = true;
                ui.close();
            }
            if ui.button(flash_menu_label(app)).clicked() {
                app.firmware_flasher.show_window = true;
                ui.close();
            }
            if ui.button("Preview / Export IL Code...").clicked() {
                app.show_code_export = true;
                ui.close();
            }
        });

        // --- HELP MENU ---
        ui.menu_button("Help", |ui| {
            if ui.button("Check for Updates...").clicked() {
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

        // --- RIGHT-ALIGNED UPDATE BADGE ---
        if let Some(new_ver) = app.updater.has_pending_update().map(|s| s.to_string()) {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let badge_btn = egui::Button::new(
                    RichText::new(format!("Update v{} Available", new_ver))
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
            });
        }
    });
}

/// PLC-menu label for the I/O pin configuration entry (target dependent)
fn io_pins_menu_label(app: &crate::app::CanvasApp) -> String {
    let target = app.plc_target();
    if target.supports_runtime_io_config() {
        "Configure RP2040 Hardware I/O Pins...".to_string()
    } else {
        format!("{} I/O Pin Map...", target.mcu_name())
    }
}

/// PLC-menu label for the firmware flasher entry (USB UF2 for RP2040, ST-Link for STM32)
fn flash_menu_label(app: &crate::app::CanvasApp) -> &'static str {
    if app.plc_target().supports_usb_bootsel() {
        "Flash MCU Firmware (USB)..."
    } else {
        "Flash MCU Firmware (ST-Link V2)..."
    }
}
