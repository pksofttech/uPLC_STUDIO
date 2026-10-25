use crate::app::{CanvasApp, WorkMode};
use crate::icons::{modern_tool_button, paint_icon, VectorIcon};
use crate::theme::{AppTheme, ThemePalette};
use crate::tools::ActiveTool;
use egui::{Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, Stroke, StrokeKind, Ui};
use my_canvas_core::SchematicStyle;

/// Uniform CAD toolbar control height (in logical pixels)
const CAD_BTN_HEIGHT: f32 = 26.0;
/// Square icon-only CAD button width (26x26)
const CAD_ICON_BTN_WIDTH: f32 = 26.0;

/// AutoCAD-style precision toolbar group container.
/// Uses direct horizontal Frame allocation with `Align::Center` and uniform `interact_size.y`
/// so buttons, dividers, ComboBoxes, and the LCD readout align on the exact same centerline.
fn cad_toolbar_group<R>(
    ui: &mut Ui,
    palette: ThemePalette,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let frame = egui::Frame::new()
        .fill(palette.pill_bg)
        .stroke(Stroke::new(1.0, palette.rung_badge_border))
        .corner_radius(CornerRadius::same(3)) // Crisp 3px technical CAD chamfer
        .inner_margin(egui::Margin::symmetric(4, 3));

    frame
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(3.0, 0.0);
            ui.spacing_mut().interact_size.y = CAD_BTN_HEIGHT;
            ui.spacing_mut().button_padding = egui::vec2(6.0, 4.0);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| add_contents(ui))
                .inner
        })
        .inner
}

/// CAD-style vertical hairline divider inside a toolbar group
fn toolbar_divider(ui: &mut Ui, palette: ThemePalette) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(3.0, CAD_BTN_HEIGHT), egui::Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.center().x, rect.min.y + 3.0),
            egui::pos2(rect.center().x, rect.max.y - 3.0),
        ],
        Stroke::new(1.0, palette.rung_badge_border),
    );
}

/// Prominent CAD hero action button (e.g. RUN, ONLINE) with active LED bar and border glow
fn cad_hero_button(
    ui: &mut Ui,
    icon: Option<VectorIcon>,
    label: &str,
    accent: Color32,
    is_active: bool,
    is_enabled: bool,
    width: f32,
    palette: ThemePalette,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(width, CAD_BTN_HEIGHT),
        if is_enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );

    if ui.is_rect_visible(rect) {
        let hovered = is_enabled && resp.hovered();
        let pressed = is_enabled && resp.is_pointer_button_down_on();

        let (bg, stroke) = if !is_enabled {
            (palette.pill_bg.gamma_multiply(0.4), Stroke::NONE)
        } else if is_active {
            (accent.gamma_multiply(0.24), Stroke::new(1.5, accent))
        } else if pressed {
            (accent.gamma_multiply(0.36), Stroke::new(1.2, accent))
        } else if hovered {
            (
                palette.rung_badge_hov_bg,
                Stroke::new(1.0, palette.accent_heading),
            )
        } else {
            (palette.card_bg, Stroke::new(1.0, palette.rung_badge_border))
        };

        ui.painter()
            .rect(rect, CornerRadius::same(3), bg, stroke, StrokeKind::Inside);

        // Industrial CAD status LED strip on bottom edge when active
        if is_active && is_enabled {
            let led_rect = Rect::from_min_max(
                egui::pos2(rect.min.x + 3.0, rect.max.y - 2.5),
                egui::pos2(rect.max.x - 3.0, rect.max.y - 0.8),
            );
            ui.painter()
                .rect_filled(led_rect, CornerRadius::same(1), accent);
        }

        let text_color = if !is_enabled {
            palette.rung_text_dim
        } else if is_active {
            accent
        } else {
            palette.text_primary
        };

        let has_label = !label.is_empty();
        if let Some(ic) = icon {
            let ic_w = 15.0;
            let ic_x = if has_label {
                rect.min.x + 12.0
            } else {
                rect.center().x
            };
            let ic_rect = Rect::from_center_size(
                egui::pos2(ic_x, rect.center().y),
                egui::vec2(ic_w, ic_w),
            );
            paint_icon(ui.painter(), ic_rect, ic, text_color);
        }

        if has_label {
            let text_x = if icon.is_some() {
                rect.min.x + 22.5
            } else {
                rect.center().x
            };
            let align = if icon.is_some() {
                Align2::LEFT_CENTER
            } else {
                Align2::CENTER_CENTER
            };
            ui.painter().text(
                egui::pos2(text_x, rect.center().y),
                align,
                label,
                FontId::proportional(11.0),
                text_color,
            );
        }
    }

    resp
}

/// Fixed-dimension recessed CAD LCD readout for scan count / simulation timer
fn cad_lcd_readout(ui: &mut Ui, text: &str, text_color: Color32, palette: ThemePalette) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(84.0, CAD_BTN_HEIGHT), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect(
            rect,
            CornerRadius::same(3),
            palette.canvas_bg,
            Stroke::new(1.0, palette.rung_badge_border),
            StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::monospace(11.0),
            text_color,
        );
    }
}

pub fn render(ui: &mut Ui, app: &mut CanvasApp) {
    let palette = app.theme.palette();
    let avail_width = ui.available_width();

    // 3-tier responsive breakpoints calibrated so groups never overflow or clip labels
    let show_file_labels = avail_width >= 1460.0;
    let is_compact = avail_width < 1220.0;
    let is_ultra_compact = avail_width < 980.0;

    egui::ScrollArea::horizontal()
        .id_salt("top_toolbar_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 2.0);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                // =========================================================================
                // Group 1: Project & Cursor Tools (New, Open, Save | Undo, Redo | Select, Pan)
                // =========================================================================
                cad_toolbar_group(ui, palette, |ui| {
                    // New File
                    let (new_lbl, new_w) = if show_file_labels {
                        ("New", 50.0)
                    } else {
                        ("", CAD_ICON_BTN_WIDTH)
                    };
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
                    .on_hover_text("New Ladder Project [Ctrl+N]")
                    .clicked()
                    {
                        app.request_new_program();
                    }

                    // Open File
                    let (open_lbl, open_w) = if show_file_labels {
                        ("Open", 54.0)
                    } else {
                        ("", CAD_ICON_BTN_WIDTH)
                    };
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
                    .on_hover_text("Open IEC 61131-3 IL (.il) or Project (.json) [Ctrl+O]")
                    .clicked()
                    {
                        app.request_open_program();
                    }

                    // Save File
                    let (save_lbl, save_w) = if show_file_labels {
                        ("Save", 54.0)
                    } else {
                        ("", CAD_ICON_BTN_WIDTH)
                    };
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
                    .on_hover_text("Save Program [Ctrl+S]")
                    .clicked()
                    {
                        app.save_or_save_as();
                    }

                    toolbar_divider(ui, palette);

                    let is_edit_mode = app.work_mode == WorkMode::Edit;

                    // Undo
                    let (undo_lbl, undo_w) = if show_file_labels {
                        ("Undo", 54.0)
                    } else {
                        ("", CAD_ICON_BTN_WIDTH)
                    };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::Undo),
                        undo_lbl,
                        palette.accent_warn,
                        false,
                        is_edit_mode && app.doc.can_undo(),
                        false,
                        undo_w,
                        palette,
                    )
                    .on_hover_text(if is_edit_mode {
                        "Undo last circuit edit [Ctrl+Z]"
                    } else {
                        "Switch to Edit Mode to undo circuit changes"
                    })
                    .clicked()
                    {
                        app.doc.undo();
                    }

                    // Redo
                    let (redo_lbl, redo_w) = if show_file_labels {
                        ("Redo", 54.0)
                    } else {
                        ("", CAD_ICON_BTN_WIDTH)
                    };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::Redo),
                        redo_lbl,
                        palette.accent_heading,
                        false,
                        is_edit_mode && app.doc.can_redo(),
                        false,
                        redo_w,
                        palette,
                    )
                    .on_hover_text(if is_edit_mode {
                        "Redo circuit edit [Ctrl+Y]"
                    } else {
                        "Switch to Edit Mode to redo circuit changes"
                    })
                    .clicked()
                    {
                        app.doc.redo();
                    }

                    toolbar_divider(ui, palette);

                    // Select / Pointer Tool
                    let is_sel = app.active_tool == ActiveTool::Select;
                    let (sel_lbl, sel_w) = if is_compact {
                        ("", CAD_ICON_BTN_WIDTH)
                    } else {
                        ("Select", 60.0)
                    };
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
                    .on_hover_text("Select & Inspect Tool (Click or drag box to select) [Esc]")
                    .clicked()
                    {
                        app.active_tool = ActiveTool::Select;
                    }

                    // Hand Pan Tool
                    let is_pan = app.active_tool == ActiveTool::Pan;
                    let (pan_lbl, pan_w) = if is_compact {
                        ("", CAD_ICON_BTN_WIDTH)
                    } else {
                        ("Pan", 48.0)
                    };
                    if modern_tool_button(
                        ui,
                        Some(VectorIcon::HandPan),
                        pan_lbl,
                        palette.accent_heading,
                        is_pan,
                        true,
                        false,
                        pan_w,
                        palette,
                    )
                    .on_hover_text("Pan Canvas Viewport (Drag with Left Mouse)")
                    .clicked()
                    {
                        app.active_tool = if is_pan {
                            ActiveTool::Select
                        } else {
                            ActiveTool::Pan
                        };
                    }
                });

                // =========================================================================
                // Group 2: 3-Way Operating Mode Switcher (Edit | Simulator | Online PLC)
                // =========================================================================
                cad_toolbar_group(ui, palette, |ui| {
                    let cur_mode = app.work_mode;

                    // 1. Edit Mode Button
                    let (edit_lbl, edit_w) = if is_compact {
                        ("✏ Edit", 54.0)
                    } else {
                        ("✏ Edit Mode", 80.0)
                    };
                    if cad_hero_button(
                        ui,
                        None,
                        edit_lbl,
                        palette.accent_heading,
                        cur_mode == WorkMode::Edit,
                        true,
                        edit_w,
                        palette,
                    )
                    .on_hover_text(
                        "1. Offline Circuit Edit Mode: Add, wire, and configure Ladder elements",
                    )
                    .clicked()
                    {
                        app.set_work_mode(WorkMode::Edit);
                    }

                    // 2. Simulator Mode Button
                    let (sim_lbl, sim_w) = if is_compact {
                        ("💻 Sim", 54.0)
                    } else {
                        ("💻 Simulator", 84.0)
                    };
                    if cad_hero_button(
                        ui,
                        None,
                        sim_lbl,
                        palette.accent_ok,
                        cur_mode == WorkMode::Simulator,
                        true,
                        sim_w,
                        palette,
                    )
                    .on_hover_text(
                        "2. PC Virtual Simulator Mode: Test logic locally on PC & click contacts on canvas to toggle ON/OFF [F5]",
                    )
                    .clicked()
                    {
                        app.set_work_mode(WorkMode::Simulator);
                    }

                    // 3. Online PLC Debug Mode Button
                    let is_hw_conn = app.is_hw_online_connected();
                    let is_matched = app
                        .online_program_match_info()
                        .map_or(true, |(m, _, _, _, _)| m);
                    let online_accent = if cur_mode == WorkMode::OnlineDebug {
                        if !is_hw_conn || !is_matched {
                            palette.accent_warn
                        } else {
                            palette.accent_ok
                        }
                    } else {
                        palette.accent_warn
                    };
                    let (on_lbl, on_w) = if is_compact {
                        ("🔌 PLC", 54.0)
                    } else {
                        ("🔌 Online PLC", 90.0)
                    };
                    if cad_hero_button(
                        ui,
                        None,
                        on_lbl,
                        online_accent,
                        cur_mode == WorkMode::OnlineDebug,
                        true,
                        on_w,
                        palette,
                    )
                    .on_hover_text(
                        "3. Hardware Online Debug Mode: Connect to physical RP2040 PLC via USB-CDC for live monitoring & Force I/O [F7 / Ctrl+M]",
                    )
                    .clicked()
                    {
                        app.set_work_mode(WorkMode::OnlineDebug);
                    }
                });

                // =========================================================================
                // Group 3: Contextual Execution & PLC Controls (Adapts to active WorkMode)
                // =========================================================================
                cad_toolbar_group(ui, palette, |ui| match app.work_mode {
                    WorkMode::Edit => {
                        // Quick Start Simulation button
                        let (sim_run_lbl, sim_run_w) = if is_compact {
                            ("Sim", 52.0)
                        } else {
                            ("Simulate", 74.0)
                        };
                        if cad_hero_button(
                            ui,
                            Some(VectorIcon::Play),
                            sim_run_lbl,
                            palette.accent_ok,
                            false,
                            true,
                            sim_run_w,
                            palette,
                        )
                        .on_hover_text("Start Local PC Simulation immediately [F5]")
                        .clicked()
                        {
                            app.set_work_mode(WorkMode::Simulator);
                        }

                        toolbar_divider(ui, palette);

                        // Update / Download to PLC
                        let (plc_lbl, plc_w) = if is_compact {
                            ("Update", 64.0)
                        } else {
                            ("Update PLC", 88.0)
                        };
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
                        .on_hover_text(
                            "Compile IL to 32-bit Micro-VM Bytecode & Download to RP2040 PLC Flash via USB",
                        )
                        .clicked()
                        {
                            app.show_update_plc = !app.show_update_plc;
                        }

                        // Read / Upload from PLC
                        if !is_ultra_compact {
                            let (read_lbl, read_w) = if is_compact {
                                ("Read", 46.0)
                            } else {
                                ("📥 Read PLC", 76.0)
                            };
                            if cad_hero_button(
                                ui,
                                None,
                                read_lbl,
                                palette.accent_heading,
                                false,
                                true,
                                read_w,
                                palette,
                            )
                            .on_hover_text(
                                "Read compiled bytecode from connected RP2040 PLC and decompile into Ladder Diagram",
                            )
                            .clicked()
                            {
                                app.request_upload_from_plc();
                            }
                        }
                    }

                    WorkMode::Simulator => {
                        let is_running = app.doc.simulator.is_running();
                        let is_step = app.doc.simulator.is_step();

                        // Run / Pause Hero Button
                        let (run_lbl, run_w) = if is_running {
                            ("RUN", 54.0)
                        } else if is_compact {
                            ("Run", 50.0)
                        } else {
                            ("Resume", 64.0)
                        };
                        if cad_hero_button(
                            ui,
                            Some(if is_running {
                                VectorIcon::Stop
                            } else {
                                VectorIcon::Play
                            }),
                            run_lbl,
                            if is_running {
                                palette.accent_ok
                            } else {
                                palette.accent_heading
                            },
                            is_running,
                            true,
                            run_w,
                            palette,
                        )
                        .on_hover_text(if is_running {
                            "Pause Local Simulation [F5]"
                        } else {
                            "Run / Resume Continuous Simulation [F5]"
                        })
                        .clicked()
                        {
                            app.doc.toggle_simulation();
                        }

                        // Single Scan Step Button
                        let (step_lbl, step_w) = if is_compact {
                            ("", CAD_ICON_BTN_WIDTH)
                        } else {
                            ("Step", 52.0)
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
                        .on_hover_text("Execute Single Scan Step in Simulator [F6]")
                        .clicked()
                        {
                            app.single_step_mode();
                        }

                        // Reset Simulation Button
                        let (reset_lbl, reset_w) = if is_compact {
                            ("", CAD_ICON_BTN_WIDTH)
                        } else {
                            ("Reset", 56.0)
                        };
                        if modern_tool_button(
                            ui,
                            Some(VectorIcon::Reset),
                            reset_lbl,
                            palette.accent_danger,
                            false,
                            true,
                            false,
                            reset_w,
                            palette,
                        )
                        .on_hover_text("Reset Simulation & Clear All Coils/Timers/Counters [Shift+F5]")
                        .clicked()
                        {
                            app.doc.reset_simulation();
                        }

                        toolbar_divider(ui, palette);

                        // Simulation Speed Multiplier
                        egui::ComboBox::from_id_salt("toolbar_sim_speed_combo")
                            .width(48.0)
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
                            .on_hover_text("Virtual simulation speed multiplier");

                        // Simulated Time LCD Readout
                        if !is_ultra_compact {
                            let time_text = app.doc.simulator.formatted_sim_time();
                            let text_color = if is_running {
                                palette.accent_ok
                            } else {
                                palette.accent_warn
                            };
                            cad_lcd_readout(ui, &time_text, text_color, palette);
                        }
                    }

                    WorkMode::OnlineDebug => {
                        let is_hw_conn = app.is_hw_online_connected();
                        let is_running = is_hw_conn && app.plc_remote_running;
                        let is_matched = app
                            .online_program_match_info()
                            .map_or(true, |(m, _, _, _, _)| m);

                        // Hardware PLC RUN / STOP Hero Button
                        let (run_lbl, run_w) = if !is_hw_conn {
                            ("WAIT", 52.0)
                        } else if is_running {
                            ("RUN", 54.0)
                        } else {
                            ("STOP", 54.0)
                        };
                        if cad_hero_button(
                            ui,
                            Some(if is_running {
                                VectorIcon::Play
                            } else {
                                VectorIcon::Stop
                            }),
                            run_lbl,
                            if !is_hw_conn {
                                palette.accent_warn
                            } else if is_running {
                                palette.accent_ok
                            } else {
                                palette.accent_danger
                            },
                            is_running,
                            is_hw_conn,
                            run_w,
                            palette,
                        )
                        .on_hover_text(if !is_hw_conn {
                            "Waiting for RP2040 USB-CDC Connection..."
                        } else if is_running {
                            "PLC is RUNNING — Click to STOP Hardware PLC Execution [F5]"
                        } else {
                            "PLC is STOPPED — Click to RUN Hardware PLC Execution [F5]"
                        })
                        .clicked()
                        {
                            app.toggle_run_mode();
                        }

                        // Hardware Step Button
                        let (step_lbl, step_w) = if is_compact {
                            ("", CAD_ICON_BTN_WIDTH)
                        } else {
                            ("Step", 52.0)
                        };
                        if modern_tool_button(
                            ui,
                            Some(VectorIcon::Step),
                            step_lbl,
                            palette.accent_warn,
                            false,
                            is_hw_conn,
                            false,
                            step_w,
                            palette,
                        )
                        .on_hover_text("Execute Single Scan Step on Hardware PLC [F6]")
                        .clicked()
                        {
                            app.single_step_mode();
                        }

                        // Hardware Stop / Reset Button
                        let (stop_lbl, stop_w) = if is_compact {
                            ("", CAD_ICON_BTN_WIDTH)
                        } else {
                            ("Halt", 50.0)
                        };
                        if modern_tool_button(
                            ui,
                            Some(VectorIcon::Stop),
                            stop_lbl,
                            palette.accent_danger,
                            false,
                            is_hw_conn && is_running,
                            false,
                            stop_w,
                            palette,
                        )
                        .on_hover_text("Halt Hardware PLC Scan & Safe Output Shutdown")
                        .clicked()
                        {
                            app.stop_run_mode();
                        }

                        toolbar_divider(ui, palette);

                        // Hardware Scan Counter LCD Readout
                        if !is_ultra_compact {
                            let (time_text, text_color) = if !is_hw_conn {
                                ("WAIT USB".to_string(), palette.accent_warn)
                            } else {
                                let scans =
                                    app.latest_snapshot.as_ref().map_or(0, |s| s.scan_count);
                                (
                                    format!("{:>6} scans", scans),
                                    if is_running {
                                        palette.accent_ok
                                    } else {
                                        palette.accent_heading
                                    },
                                )
                            };
                            cad_lcd_readout(ui, &time_text, text_color, palette);
                        }

                        // Sync / Update PLC Button (Highlights when MISMATCH!)
                        let (plc_lbl, plc_w) = if !is_matched && is_hw_conn {
                            if is_compact {
                                ("⚠ Sync", 58.0)
                            } else {
                                ("⚡ Sync PLC", 84.0)
                            }
                        } else if is_compact {
                            ("Update", 64.0)
                        } else {
                            ("Update PLC", 88.0)
                        };
                        if modern_tool_button(
                            ui,
                            Some(VectorIcon::SerialComm),
                            plc_lbl,
                            palette.accent_warn,
                            app.show_update_plc || (!is_matched && is_hw_conn),
                            true,
                            false,
                            plc_w,
                            palette,
                        )
                        .on_hover_text(
                            "Open Update to PLC Dialog (Compile & Download Bytecode, I/O Config, or Flash Firmware)",
                        )
                        .clicked()
                        {
                            app.show_update_plc = !app.show_update_plc;
                        }
                    }
                });

                // =========================================================================
                // Group 4: View & Setup (Code Export, Schematic Style, Theme, Config)
                // =========================================================================
                cad_toolbar_group(ui, palette, |ui| {
                    // IEC 61131-3 IL Code Preview & Export
                    let (code_lbl, code_w) = if is_compact {
                        ("", CAD_ICON_BTN_WIDTH)
                    } else {
                        ("Code", 54.0)
                    };
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
                    let style_selected_text = if is_compact {
                        match app.doc.schematic_style {
                            SchematicStyle::IecStandard => "IEC",
                            SchematicStyle::JicNema => "JIC",
                            SchematicStyle::DinEuropean => "DIN",
                            SchematicStyle::PushBtnPilot => "Panel",
                        }
                    } else {
                        app.doc.schematic_style.short_name()
                    };
                    egui::ComboBox::from_id_salt("toolbar_schematic_style_combo")
                        .selected_text(style_selected_text)
                        .width(if is_compact { 62.0 } else { 96.0 })
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
                    let theme_selected_text = if is_compact {
                        app.theme.label().to_string()
                    } else {
                        format!("🎨 {}", app.theme.label())
                    };
                    egui::ComboBox::from_id_salt("toolbar_theme_combo")
                        .selected_text(theme_selected_text)
                        .width(if is_compact { 68.0 } else { 86.0 })
                        .show_ui(ui, |ui| {
                            for t in AppTheme::ALL {
                                if ui.selectable_label(app.theme == t, t.label()).clicked() {
                                    app.set_theme(t);
                                }
                            }
                        });

                    toolbar_divider(ui, palette);

                    // Settings & Hardware I/O Configuration
                    let (cfg_lbl, cfg_w) = if is_compact {
                        ("", CAD_ICON_BTN_WIDTH)
                    } else {
                        ("Config", 62.0)
                    };
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
