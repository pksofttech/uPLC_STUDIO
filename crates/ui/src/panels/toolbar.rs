use crate::app::CanvasApp;
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
                        app.doc.can_undo(),
                        false,
                        undo_w,
                        palette,
                    )
                    .on_hover_text("Undo last circuit edit [Ctrl+Z]")
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
                        app.doc.can_redo(),
                        false,
                        redo_w,
                        palette,
                    )
                    .on_hover_text("Redo circuit edit [Ctrl+Y]")
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
                    .on_hover_text("Select & Move Tool (Click or drag box to select) [Esc]")
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
                // Group 2: Simulation / PLC Execution (Run, Step, Stop, Reset | Speed, LCD)
                // =========================================================================
                let is_hw = app.online_target_hw;
                let is_hw_conn = !is_hw || app.is_hw_online_connected();
                let (is_running, is_stopped, is_step) = if is_hw {
                    let r = is_hw_conn && app.plc_remote_running;
                    (r, !r, !r)
                } else {
                    (
                        app.doc.simulator.is_running(),
                        app.doc.simulator.is_stopped(),
                        app.doc.simulator.is_step(),
                    )
                };

                cad_toolbar_group(ui, palette, |ui| {
                    // Start / RUN Hero Button
                    let (start_label, start_w) = if is_running {
                        ("RUN", 54.0)
                    } else if is_compact {
                        ("Run", 50.0)
                    } else {
                        ("Start", 56.0)
                    };

                    let run_hover = if is_hw {
                        if app.online_mode && !is_hw_conn {
                            "Waiting for RP2040 USB Connection (Auto-reconnecting...)"
                        } else if is_running {
                            "Halt / STOP Hardware PLC Execution [F5]"
                        } else if app.online_mode {
                            "Start / RUN Hardware PLC Execution [F5]"
                        } else {
                            "Connect & Start Hardware PLC Execution [F5]"
                        }
                    } else {
                        "Start / Continuous Simulation [F5]"
                    };

                    if cad_hero_button(
                        ui,
                        Some(VectorIcon::Play),
                        start_label,
                        palette.accent_ok,
                        is_running,
                        true,
                        start_w,
                        palette,
                    )
                    .on_hover_text(run_hover)
                    .clicked()
                    {
                        app.toggle_run_mode();
                    }

                    // Step Button
                    let (step_lbl, step_w) = if is_compact {
                        ("", CAD_ICON_BTN_WIDTH)
                    } else {
                        ("Step", 52.0)
                    };
                    let step_hover = if is_hw {
                        "Execute Single Scan Step on Hardware PLC [F6]"
                    } else {
                        "Execute Single Scan Step [F6]"
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
                    let (stop_lbl, stop_w) = if is_compact {
                        ("", CAD_ICON_BTN_WIDTH)
                    } else {
                        ("Stop", 52.0)
                    };
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
                    let (reset_lbl, reset_w) = if is_compact {
                        ("", CAD_ICON_BTN_WIDTH)
                    } else {
                        ("Reset", 58.0)
                    };
                    let reset_hover = if is_hw {
                        "Reset / Halt Hardware PLC [Shift+F5]"
                    } else {
                        "Reset Simulation & Clear Memory [Shift+F5]"
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

                    // Simulation Speed Multiplier (48px guarantees "x10" + arrow never clips)
                    ui.add_enabled_ui(!is_hw, |ui| {
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
                            .on_hover_text(if is_hw {
                                "Simulation speed (Fixed 10ms hardware scan cycle on RP2040)"
                            } else {
                                "Virtual simulation speed multiplier"
                            });
                    });

                    // Digital Hardware Scan Timer / Counter Display (Fixed-size 84x26 LCD Readout)
                    if !is_ultra_compact {
                        let (time_text, text_color) = if is_hw {
                            if app.online_mode && !is_hw_conn {
                                ("RECONN...".to_string(), palette.accent_warn)
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
                            }
                        } else {
                            (
                                app.doc.simulator.formatted_sim_time(),
                                if is_running {
                                    palette.accent_ok
                                } else {
                                    palette.accent_heading
                                },
                            )
                        };

                        cad_lcd_readout(ui, &time_text, text_color, palette);
                    }
                });

                // =========================================================================
                // Group 3: Online & PLC Link (Go Online, Target Selector, Update PLC)
                // =========================================================================
                cad_toolbar_group(ui, palette, |ui| {
                    let is_online = app.online_mode;
                    let (online_btn_text, online_btn_w) = if is_online {
                        ("ONLINE", 68.0)
                    } else if is_compact {
                        ("Online", 64.0)
                    } else {
                        ("Go Online", 82.0)
                    };
                    let online_btn_color = if is_online {
                        palette.accent_ok
                    } else {
                        palette.accent_heading
                    };

                    if cad_hero_button(
                        ui,
                        Some(VectorIcon::SerialComm),
                        online_btn_text,
                        online_btn_color,
                        is_online,
                        true,
                        online_btn_w,
                        palette,
                    )
                    .on_hover_text(if is_online {
                        "Online Monitoring ACTIVE: Real-time status from PLC / Simulator [Ctrl+M / F7]"
                    } else {
                        "Start Online Monitoring: Live debug Process Image in real time [Ctrl+M / F7]"
                    })
                    .clicked()
                    {
                        app.toggle_online_mode();
                    }

                    toolbar_divider(ui, palette);

                    // Target selector: Hardware (RP2040) vs Local Simulator
                    let cur_target_label = if is_compact {
                        if app.online_target_hw {
                            "🔌 HW"
                        } else {
                            "💻 Sim"
                        }
                    } else if app.online_target_hw {
                        "🔌 RP2040"
                    } else {
                        "💻 Simulator"
                    };
                    let target_combo_w = if is_compact { 68.0 } else { 96.0 };
                    egui::ComboBox::from_id_salt("toolbar_online_target_combo")
                        .width(target_combo_w)
                        .selected_text(cur_target_label)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(app.online_target_hw, "🔌 Hardware (RP2040 USB)")
                                .on_hover_text(
                                    "Monitor & control physical RP2040 Micro-VM over USB CDC Serial",
                                )
                                .clicked()
                            {
                                app.set_online_target_hw(true);
                                ui.close();
                            }
                            if ui
                                .selectable_label(!app.online_target_hw, "💻 Virtual Simulator")
                                .on_hover_text(
                                    "Monitor & control internal local PLC software simulation engine",
                                )
                                .clicked()
                            {
                                app.set_online_target_hw(false);
                                ui.close();
                            }
                        });

                    toolbar_divider(ui, palette);

                    // Update to PLC (Compile IL -> Bytecode & Upload to MCU VM)
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
                        "Compile IL to 32-bit Micro-VM Bytecode & Update to PLC via Serial/USB",
                    )
                    .clicked()
                    {
                        app.show_update_plc = !app.show_update_plc;
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
