use egui::{Color32, CornerRadius, Margin, RichText, Ui};
use my_canvas_core::SimState;
use crate::app::{CanvasApp, WorkMode};

pub fn render(ui: &mut Ui, app: &mut CanvasApp) {
    let palette = app.theme.palette();
    let pill_alpha = if app.theme.is_light() { 28 } else { 36 };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        // --- LEFT ZONE ---
        // 1. Compact Colored Mode Pill
        let (pill_text, pill_fg, pill_tooltip): (&str, Color32, Option<String>) =
            match app.work_mode {
                WorkMode::Edit => ("EDIT", palette.accent_heading, None),
                WorkMode::Simulator => match app.doc.simulator.state {
                    SimState::Running => ("● SIM RUN", palette.accent_ok, None),
                    SimState::Step => ("SIM STEP", palette.accent_warn, None),
                    SimState::Stopped => ("SIM PAUSED", palette.accent_heading, None),
                },
                WorkMode::OnlineDebug => {
                    if !app.is_hw_online_connected() {
                        (
                            "ONLINE · OFFLINE",
                            palette.accent_warn,
                            Some("Disconnected — auto-reconnecting...".to_string()),
                        )
                    } else {
                        match app.online_program_match_info() {
                            Some((true, _, ide_crc, _, _)) => (
                                "● ONLINE · MATCH",
                                palette.accent_ok,
                                Some(format!(
                                    "{} · CRC 0x{:04X}",
                                    app.plc_target().mcu_name(),
                                    ide_crc
                                )),
                            ),
                            Some((false, _, ide_crc, _, plc_crc)) => (
                                "ONLINE · MISMATCH",
                                palette.accent_warn,
                                Some(format!("IDE 0x{:04X} ≠ PLC 0x{:04X}", ide_crc, plc_crc)),
                            ),
                            None => ("● ONLINE", palette.accent_ok, None),
                        }
                    }
                }
            };

        let pill_bg =
            Color32::from_rgba_unmultiplied(pill_fg.r(), pill_fg.g(), pill_fg.b(), pill_alpha);
        let pill_resp = egui::Frame::NONE
            .fill(pill_bg)
            .corner_radius(CornerRadius::same(3))
            .inner_margin(Margin::symmetric(6, 1))
            .show(ui, |ui| {
                ui.label(RichText::new(pill_text).size(11.0).strong().color(pill_fg));
            })
            .response;
        if let Some(tip) = pill_tooltip {
            pill_resp.on_hover_text(tip);
        }

        // 2. Clickable Hardware & Port Segment
        let target = app.plc_target();
        let hw_text = if app.com_port.trim().is_empty() {
            format!("{} · Auto", target.mcu_name())
        } else {
            format!(
                "{} · {} @ {}",
                target.mcu_name(),
                app.com_port_display(),
                app.baud_rate
            )
        };
        let hw_tooltip = format!(
            "{}\n{}\nClick to open Hardware Settings",
            app.doc.program.hardware,
            target.link_description()
        );
        let hw_resp = ui
            .add(
                egui::Label::new(RichText::new(hw_text).size(11.0).color(palette.text_primary))
                    .sense(egui::Sense::click()),
            )
            .on_hover_text(hw_tooltip)
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if hw_resp.clicked() {
            app.settings_tab = 1;
            app.show_io_config = true;
        }

        // 3. Scan & Cycle Readout (non-Edit modes only)
        if app.work_mode != WorkMode::Edit {
            let (scan_cnt, cycle_ms) = if app.online_mode && app.online_target_hw {
                let sc = if app.is_hw_online_connected() {
                    app.latest_snapshot
                        .as_ref()
                        .map_or(0, |s| s.scan_count as u64)
                } else {
                    0
                };
                (sc, 10u32) // Deterministic 10ms scan cycle on hardware (SCAN_CYCLE_MS)
            } else {
                (app.doc.simulator.scan_count, app.doc.simulator.cycle_time_ms)
            };
            ui.label(RichText::new("·").size(11.0).color(palette.rung_text_dim));
            ui.label(
                RichText::new(format!("Scans: {} · {}ms", scan_cnt, cycle_ms))
                    .size(11.0)
                    .color(palette.rung_text_dim),
            );
        }

        // 4. Accumulator Readout (active Simulator or Online)
        if app.online_mode || app.doc.simulator.is_active() {
            let (acc_b, acc_w) = if app.online_mode && app.online_target_hw {
                if app.is_hw_online_connected() {
                    if let Some(snap) = app.latest_snapshot.as_ref() {
                        (snap.acc_bool, snap.acc_word)
                    } else {
                        (false, 0)
                    }
                } else {
                    (false, 0)
                }
            } else {
                (app.doc.simulator.acc_bool, app.doc.simulator.acc_word)
            };
            ui.label(RichText::new("·").size(11.0).color(palette.rung_text_dim));
            ui.label(
                RichText::new(format!("ACC: {} | {}", u8::from(acc_b), acc_w))
                    .monospace()
                    .size(11.0)
                    .color(if acc_b {
                        palette.accent_ok
                    } else {
                        palette.accent_heading
                    }),
            );
        }

        // --- RIGHT ZONE ---
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // 1. Clickable Zoom Readout
            let zoom_resp = ui
                .add(
                    egui::Label::new(
                        RichText::new(format!("{:.0}%", app.canvas.zoom * 100.0))
                            .monospace()
                            .size(11.0)
                            .color(palette.text_primary),
                    )
                    .sense(egui::Sense::click()),
                )
                .on_hover_text("Click to reset zoom & pan (100%)")
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if zoom_resp.clicked() {
                app.canvas.reset_view();
            }

            // 2. Active Tool
            ui.label(RichText::new("·").size(11.0).color(palette.rung_text_dim));
            ui.label(
                RichText::new(app.active_tool.name())
                    .size(11.0)
                    .color(palette.rung_text_dim),
            );

            // 3. Selected Network
            if let Some(r_idx) = app.doc.selected_rung_idx {
                ui.label(RichText::new("·").size(11.0).color(palette.rung_text_dim));
                ui.label(
                    RichText::new(format!("Network #{:02}", r_idx + 1))
                        .size(11.0)
                        .color(palette.rung_text_dim),
                );
            }
        });
    });
}
