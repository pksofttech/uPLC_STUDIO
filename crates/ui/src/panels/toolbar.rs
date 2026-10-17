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

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);

        // =========================================================================
        // Group 1: Project & History
        // =========================================================================
        toolbar_group(ui, palette, |ui| {
            // New File
            if modern_tool_button(
                ui,
                Some(VectorIcon::NewFile),
                "New",
                palette.accent_heading,
                false,
                true,
                false,
                58.0,
                palette,
            )
            .on_hover_text("New Ladder Project (Ctrl+N)")
            .clicked()
            {
                app.new_program();
            }

            // Open File
            if modern_tool_button(
                ui,
                Some(VectorIcon::OpenFile),
                "Open",
                palette.accent_heading,
                false,
                true,
                false,
                60.0,
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
            if modern_tool_button(
                ui,
                Some(VectorIcon::SaveFile),
                "Save",
                palette.accent_ok,
                false,
                true,
                false,
                60.0,
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
            if modern_tool_button(
                ui,
                Some(VectorIcon::Undo),
                "Undo",
                palette.accent_warn,
                false,
                app.doc.can_undo(),
                false,
                62.0,
                palette,
            )
            .on_hover_text("Undo last circuit edit (Ctrl+Z)")
            .clicked()
            {
                app.doc.undo();
            }

            // Redo
            if modern_tool_button(
                ui,
                Some(VectorIcon::Redo),
                "Redo",
                palette.accent_heading,
                false,
                app.doc.can_redo(),
                false,
                62.0,
                palette,
            )
            .on_hover_text("Redo circuit edit (Ctrl+Y)")
            .clicked()
            {
                app.doc.redo();
            }
        });

        // =========================================================================
        // Group 2: Ladder Component Palette & Drag-and-Drop
        // =========================================================================
        toolbar_group(ui, palette, |ui| {
            // Select Tool
            let is_sel = app.active_tool == ActiveTool::Select;
            let sel_resp = modern_tool_button(
                ui,
                Some(VectorIcon::CursorSelect),
                "Select",
                palette.accent_heading,
                is_sel,
                true,
                false,
                64.0,
                palette,
            )
            .on_hover_text("Select & Move Tool (Click or drag box to select / Press Esc)");
            if sel_resp.clicked() {
                app.active_tool = ActiveTool::Select;
            }
        });

        // =========================================================================
        // Group 4: Simulation Station
        // =========================================================================
        let is_running = app.doc.simulator.is_running();
        let is_stopped = app.doc.simulator.is_stopped();
        let is_step = app.doc.simulator.is_step();

        toolbar_group(ui, palette, |ui| {
            // Start / Running Button
            let (start_label, start_accent, start_w) = if is_running {
                ("RUN", palette.accent_ok, 68.0)
            } else {
                ("Start", palette.accent_ok, 66.0)
            };

            if modern_tool_button(
                ui,
                Some(VectorIcon::Play),
                start_label,
                start_accent,
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
            if modern_tool_button(
                ui,
                Some(VectorIcon::Step),
                "Step",
                palette.accent_warn,
                is_step,
                true,
                false,
                62.0,
                palette,
            )
            .on_hover_text("Execute Single Scan Step (F6)")
            .clicked()
            {
                app.doc.single_step_simulation(app.doc.simulator.cycle_time_ms);
            }

            // Stop Button (Safe output shutdown)
            if modern_tool_button(
                ui,
                Some(VectorIcon::Stop),
                "Stop",
                palette.accent_danger,
                false,
                !is_stopped,
                false,
                62.0,
                palette,
            )
            .on_hover_text("Stop Simulation & Safe Output Shutdown")
            .clicked()
            {
                app.doc.stop_simulation();
            }

            // Reset Button
            if modern_tool_button(
                ui,
                Some(VectorIcon::Reset),
                "Reset",
                palette.accent_heading,
                false,
                true,
                false,
                62.0,
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
        });

        // =========================================================================
        // Group 5: Environment, Style & Settings
        // =========================================================================
        toolbar_group(ui, palette, |ui| {
            // Unified Schematic Style (Switch & Coil synced)
            ui.label(
                RichText::new("Style:")
                    .size(11.5)
                    .color(palette.rung_text_dim),
            );
            egui::ComboBox::from_id_salt("toolbar_schematic_style_combo")
                .selected_text(app.doc.schematic_style.short_name())
                .width(105.0)
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

            // Theme Switcher Pills
            for t in AppTheme::ALL {
                let is_active = app.theme == t;
                let btn_w = if t == AppTheme::Modern { 54.0 } else { 44.0 };
                let resp = modern_tool_button(
                    ui,
                    None,
                    t.label(),
                    palette.accent_heading,
                    is_active,
                    true,
                    false,
                    btn_w,
                    palette,
                );
                if resp
                    .on_hover_text(format!("Switch to {} theme (auto-saved)", t.label()))
                    .clicked()
                {
                    app.set_theme(t);
                }
            }

            toolbar_divider(ui, palette);

            // IEC 61131-3 IL Code Preview & Export
            if modern_tool_button(
                ui,
                Some(VectorIcon::ExportCode),
                "Code",
                palette.accent_ok,
                false,
                true,
                false,
                62.0,
                palette,
            )
            .on_hover_text("Preview & Export IEC 61131-3 IL Code")
            .clicked()
            {
                app.show_code_export = true;
            }

            // Settings, Theme & Hardware I/O Config
            if modern_tool_button(
                ui,
                Some(VectorIcon::ConfigGear),
                "Config",
                palette.accent_heading,
                app.show_io_config,
                true,
                false,
                66.0,
                palette,
            )
            .on_hover_text("Settings, Theme & Hardware Configuration")
            .clicked()
            {
                app.show_io_config = !app.show_io_config;
            }
        });
    });
}
