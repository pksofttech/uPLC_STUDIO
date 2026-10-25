use crate::app::{CanvasApp, WorkMode};
use crate::icons::{paint_icon, VectorIcon};
use crate::theme::ThemePalette;
use crate::tools::ActiveTool;
use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};

pub fn render(ui: &mut Ui, app: &mut CanvasApp) {
    let palette = app.theme.palette();
    let is_edit_mode = app.work_mode == WorkMode::Edit;

    if !is_edit_mode {
        let (mode_title, mode_col) = match app.work_mode {
            WorkMode::Simulator => ("💻 Simulator Mode", palette.accent_ok),
            WorkMode::OnlineDebug => ("🔌 Online PLC Debug", palette.accent_warn),
            WorkMode::Edit => ("✏ Edit Mode", palette.accent_heading),
        };
        egui::Frame::new()
            .fill(mode_col.gamma_multiply(0.14))
            .stroke(Stroke::new(1.2, mode_col))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(mode_title).color(mode_col).strong().size(11.5));
                ui.label(
                    RichText::new("🔒 Circuit editing is locked while testing.")
                        .size(10.0)
                        .color(palette.text_primary),
                );
                ui.add_space(4.0);
                if ui
                    .add_sized(
                        [ui.available_width(), 24.0],
                        egui::Button::new(
                            RichText::new("✏ Switch to Edit Mode")
                                .strong()
                                .size(11.0)
                                .color(Color32::WHITE),
                        )
                        .fill(palette.accent_heading),
                    )
                    .clicked()
                {
                    app.set_work_mode(WorkMode::Edit);
                }
            });
        ui.add_space(6.0);
    }

    ui.add_enabled_ui(is_edit_mode, |ui| {
    // --- 1. Top Box: Modern Icon Grid for Network Tools ---
    ui.group(|ui| {
        ui.set_width(ui.available_width());

        let total_rungs = app.doc.program.rungs.len();
        let cur_idx = app.doc.selected_rung_idx;
        let can_move_up = cur_idx.map_or(false, |i| i > 0);
        let can_move_dn = cur_idx.map_or(false, |i| i + 1 < total_rungs);
        let has_rung = cur_idx.is_some();

        // Header with Rung Status Badge
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Network Tools")
                    .color(palette.accent_heading)
                    .strong(),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(idx) = cur_idx {
                    let badge_text = format!("#{}/{}", idx + 1, total_rungs);
                    let pill_bg = if palette.is_light() {
                        Color32::from_rgb(220, 235, 252)
                    } else {
                        Color32::from_rgb(32, 50, 75)
                    };
                    let pill_border = if palette.is_light() {
                        Color32::from_rgb(70, 130, 220)
                    } else {
                        Color32::from_rgb(80, 140, 230)
                    };
                    egui::Frame::new()
                        .fill(pill_bg)
                        .stroke(Stroke::new(1.0, pill_border))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(5, 1))
                        .show(ui, |ui| {
                            ui.label(RichText::new(badge_text).size(10.0).strong().color(
                                if palette.is_light() {
                                    Color32::from_rgb(20, 60, 140)
                                } else {
                                    Color32::WHITE
                                },
                            ));
                        });
                } else {
                    ui.label(
                        RichText::new(format!("{} Rungs", total_rungs))
                            .size(10.5)
                            .color(palette.rung_text_dim),
                    );
                }
            });
        });

        ui.separator();

        let copy_accent = if palette.is_light() {
            Color32::from_rgb(115, 65, 185)
        } else {
            Color32::from_rgb(168, 148, 220)
        };

        let avail_w = ui.available_width();
        let col_w = ((avail_w - 6.0) / 3.0).floor().max(42.0);

        // Row 1: New Rung, Move Up, Move Down
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;

            if network_action_tile(
                ui,
                VectorIcon::NewRung,
                "New Rung",
                palette.accent_ok,
                true,
                col_w,
                palette,
            )
            .on_hover_text("Append a new network to the end of ladder program")
            .clicked()
            {
                app.doc.add_rung();
            }

            if network_action_tile(
                ui,
                VectorIcon::MoveUp,
                "Move Up",
                palette.accent_heading,
                can_move_up,
                col_w,
                palette,
            )
            .on_hover_text("Move selected network up")
            .clicked()
            {
                if let Some(idx) = cur_idx {
                    app.doc.move_rung_up(idx);
                }
            }

            if network_action_tile(
                ui,
                VectorIcon::MoveDown,
                "Move Dn",
                palette.accent_heading,
                can_move_dn,
                col_w,
                palette,
            )
            .on_hover_text("Move selected network down")
            .clicked()
            {
                if let Some(idx) = cur_idx {
                    app.doc.move_rung_down(idx);
                }
            }
        });

        ui.add_space(3.0);

        // Row 2: Duplicate Below, Duplicate Above, Delete
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;

            if network_action_tile(
                ui,
                VectorIcon::CopyDown,
                "Duplicate",
                copy_accent,
                has_rung,
                col_w,
                palette,
            )
            .on_hover_text("Duplicate selected network below (copy & paste)")
            .clicked()
            {
                if cur_idx.is_some() {
                    app.paste_network_below();
                }
            }

            if network_action_tile(
                ui,
                VectorIcon::CopyUp,
                "Copy Up",
                copy_accent,
                has_rung,
                col_w,
                palette,
            )
            .on_hover_text("Duplicate selected network above current position")
            .clicked()
            {
                if let Some(idx) = cur_idx {
                    app.doc.copy_rung_at(idx, idx);
                }
            }

            if network_action_tile(
                ui,
                VectorIcon::DeleteRung,
                "Delete",
                palette.accent_danger,
                has_rung,
                col_w,
                palette,
            )
            .on_hover_text("Delete selected network (Delete)")
            .clicked()
            {
                if let Some(idx) = cur_idx {
                    app.doc.delete_rung(idx);
                }
            }
        });
    });

    ui.add_space(6.0);

    // --- 2. Bottom Box: Modern Components Palette with Quick Bar & Collapsing Accordion ---
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new("Components")
                .color(palette.accent_heading)
                .strong(),
        );
        ui.separator();

        // --- Quick Access Insert Bar ---
        ui.label(
            RichText::new("★ Quick Insert")
                .size(11.0)
                .color(palette.accent_heading)
                .strong(),
        );

        let avail_w = ui.available_width();
        let col_w = ((avail_w - 6.0) / 3.0).floor().max(42.0);

        // Row 1: NO, NC, Coil
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            quick_tool_tile(
                ui,
                app,
                "-[ ]-",
                "NO",
                ActiveTool::ContactNo,
                palette.accent_ok,
                col_w,
                palette,
            );
            quick_tool_tile(
                ui,
                app,
                "-[/]-",
                "NC",
                ActiveTool::ContactNc,
                palette.accent_warn,
                col_w,
                palette,
            );
            quick_tool_tile(
                ui,
                app,
                "-( )-",
                "Coil",
                ActiveTool::CoilNormal,
                palette.coil_idle,
                col_w,
                palette,
            );
        });
        ui.add_space(3.0);

        // Row 2: Set, Reset, TON
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            quick_tool_tile(
                ui,
                app,
                "-(S)-",
                "Set",
                ActiveTool::CoilSet,
                palette.accent_heading,
                col_w,
                palette,
            );
            quick_tool_tile(
                ui,
                app,
                "-(R)-",
                "Reset",
                ActiveTool::CoilReset,
                palette.accent_warn,
                col_w,
                palette,
            );
            quick_tool_tile(
                ui,
                app,
                "TON",
                "Timer",
                ActiveTool::Ton,
                palette.accent_ok,
                col_w,
                palette,
            );
        });

        ui.add_space(5.0);
        ui.separator();
        ui.add_space(3.0);

        // --- Search / Filter bar ---
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(RichText::new("🔍").size(10.5).color(palette.rung_text_dim));
            let edit_w = (ui.available_width()
                - if !app.component_filter.is_empty() {
                    22.0
                } else {
                    0.0
                })
            .max(60.0);
            ui.add(
                egui::TextEdit::singleline(&mut app.component_filter)
                    .hint_text("Filter components...")
                    .desired_width(edit_w),
            );
            if !app.component_filter.is_empty() {
                if ui.small_button("✖").clicked() {
                    app.component_filter.clear();
                }
            }
        });

        ui.add_space(4.0);

        // --- Collapsing Accordion or Filtered Results ---
        egui::ScrollArea::vertical()
            .id_salt("left_components_scroll")
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;

                let filter_query = app.component_filter.trim().to_lowercase();
                if !filter_query.is_empty() {
                    let mut match_count = 0;
                    for comp in ALL_COMPONENTS {
                        if comp.name.to_lowercase().contains(&filter_query)
                            || comp.badge.to_lowercase().contains(&filter_query)
                            || comp.category.to_lowercase().contains(&filter_query)
                        {
                            component_accordion_item(
                                ui,
                                app,
                                comp.icon,
                                comp.badge,
                                comp.name,
                                comp.tool,
                                comp.accent(palette),
                                palette,
                            );
                            match_count += 1;
                        }
                    }
                    if match_count == 0 {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("No matching components")
                                .size(11.0)
                                .color(palette.rung_text_dim),
                        );
                    }
                } else {
                    // 1. IEC Bit Logic
                    egui::CollapsingHeader::new(
                        RichText::new("⚡ IEC Bit Logic")
                            .size(11.5)
                            .strong()
                            .color(palette.accent_heading),
                    )
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::ContactNo),
                            "-[ ]-",
                            "NO Contact [ ]",
                            ActiveTool::ContactNo,
                            palette.accent_ok,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::ContactNc),
                            "-[/]-",
                            "NC Contact [/]",
                            ActiveTool::ContactNc,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::PulseEdge),
                            "-[P]-",
                            "P Contact (Rising)",
                            ActiveTool::ContactP,
                            palette.accent_ok,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::PulseEdge),
                            "-[N]-",
                            "N Contact (Falling)",
                            ActiveTool::ContactN,
                            palette.accent_warn,
                            palette,
                        );
                        ui.separator();
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::Coil),
                            "-( )-",
                            "Coil (Normal Out)",
                            ActiveTool::CoilNormal,
                            palette.coil_idle,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::Coil),
                            "-(/)-",
                            "Negated Coil (/)",
                            ActiveTool::CoilNegated,
                            palette.accent_danger,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::CoilSet),
                            "-(S)-",
                            "Set Latch Coil (S)",
                            ActiveTool::CoilSet,
                            palette.accent_heading,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::CoilReset),
                            "-(R)-",
                            "Reset Coil (R)",
                            ActiveTool::CoilReset,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::PulseEdge),
                            "-(P)-",
                            "Positive Edge Coil",
                            ActiveTool::CoilP,
                            palette.accent_ok,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::PulseEdge),
                            "-(N)-",
                            "Negative Edge Coil",
                            ActiveTool::CoilN,
                            palette.accent_warn,
                            palette,
                        );
                        ui.separator();
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "SR",
                            "SR Bistable FB",
                            ActiveTool::Sr,
                            palette.accent_heading,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "RS",
                            "RS Bistable FB",
                            ActiveTool::Rs,
                            palette.accent_heading,
                            palette,
                        );
                        ui.separator();
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::MasterRelay),
                            "MCR",
                            "Master Relay (MC)",
                            ActiveTool::MasterRelay,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::ParallelBranch),
                            "-[∥]-",
                            "Parallel Branch",
                            ActiveTool::Branch,
                            palette.accent_ok,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            None,
                            "───",
                            "Short Circuit Wire",
                            ActiveTool::Wire,
                            palette.wire_idle,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            None,
                            "─ ─",
                            "Open Circuit",
                            ActiveTool::OpenCircuit,
                            palette.accent_danger,
                            palette,
                        );
                    });

                    // 2. IEC Timers & Counters
                    egui::CollapsingHeader::new(
                        RichText::new("⏱ IEC Timers & Counters")
                            .size(11.5)
                            .strong()
                            .color(palette.accent_ok),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::Timer),
                            "TON",
                            "TON (On-Delay)",
                            ActiveTool::Ton,
                            palette.accent_ok,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::Timer),
                            "TOF",
                            "TOF (Off-Delay)",
                            ActiveTool::Tof,
                            palette.accent_ok,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::Timer),
                            "TP",
                            "TP (Pulse Timer)",
                            ActiveTool::Tp,
                            palette.accent_ok,
                            palette,
                        );
                        ui.separator();
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::Counter),
                            "CTU",
                            "CTU (Count Up)",
                            ActiveTool::Ctu,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::Counter),
                            "CTD",
                            "CTD (Count Down)",
                            ActiveTool::Ctd,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::Counter),
                            "CTUD",
                            "CTUD (Up/Down)",
                            ActiveTool::Ctud,
                            palette.accent_warn,
                            palette,
                        );
                    });

                    // 3. IEC Edge Triggers
                    egui::CollapsingHeader::new(
                        RichText::new("⚡ IEC Edge Triggers")
                            .size(11.5)
                            .strong()
                            .color(palette.accent_ok),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::PulseEdge),
                            "R_TRG",
                            "R_TRIG (Rising Edge)",
                            ActiveTool::RTrig,
                            palette.accent_ok,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::PulseEdge),
                            "F_TRG",
                            "F_TRIG (Falling Edge)",
                            ActiveTool::FTrig,
                            palette.accent_warn,
                            palette,
                        );
                    });

                    // 4. IEC Math & Compare
                    egui::CollapsingHeader::new(
                        RichText::new("📐 IEC Math & Compare")
                            .size(11.5)
                            .strong()
                            .color(palette.accent_warn),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            ":=",
                            "Move [:=]",
                            ActiveTool::Move,
                            palette.accent_heading,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "CMP",
                            "Compare [==, !=, ...]",
                            ActiveTool::Compare,
                            palette.accent_heading,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "+=",
                            "Add [+=]",
                            ActiveTool::Add,
                            palette.accent_ok,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "-=",
                            "Sub [-=]",
                            ActiveTool::Sub,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "*=",
                            "Mul [*=]",
                            ActiveTool::Mul,
                            palette.accent_heading,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "/=",
                            "Div [/=]",
                            ActiveTool::Div,
                            palette.accent_danger,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "%=",
                            "Mod [%=]",
                            ActiveTool::Mod,
                            palette.accent_heading,
                            palette,
                        );
                    });

                    // 5. IEC Program Flow
                    egui::CollapsingHeader::new(
                        RichText::new("🔀 IEC Program Flow")
                            .size(11.5)
                            .strong()
                            .color(palette.accent_heading),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::MoveDown),
                            "JMP",
                            "JMP (Jump to Label)",
                            ActiveTool::Jump,
                            palette.accent_heading,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::MoveUp),
                            "LBL",
                            "LBL (Target Label)",
                            ActiveTool::Label,
                            palette.accent_ok,
                            palette,
                        );
                    });

                    // 6. Process Control
                    egui::CollapsingHeader::new(
                        RichText::new("🎛 Process Control")
                            .size(11.5)
                            .strong()
                            .color(palette.accent_ok),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "PID",
                            "PID Controller",
                            ActiveTool::Pid,
                            palette.accent_ok,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "SCL",
                            "Scale (Linear Map)",
                            ActiveTool::Scale,
                            palette.accent_heading,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "LIM",
                            "Limit (Clamp Min/Max)",
                            ActiveTool::Limit,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "ADC",
                            "A/D Converter Read",
                            ActiveTool::Adc,
                            palette.accent_heading,
                            palette,
                        );
                    });

                    // 7. Bitwise & Shift
                    egui::CollapsingHeader::new(
                        RichText::new("🔣 Bitwise & Shift")
                            .size(11.5)
                            .strong()
                            .color(palette.accent_warn),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "AND",
                            "Bitwise AND [&]",
                            ActiveTool::BitAnd,
                            palette.accent_ok,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "OR",
                            "Bitwise OR [|]",
                            ActiveTool::BitOr,
                            palette.accent_heading,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "XOR",
                            "Bitwise XOR [^]",
                            ActiveTool::BitXor,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::FunctionBlock),
                            "NOT",
                            "Bitwise NOT [~]",
                            ActiveTool::BitNot,
                            palette.accent_danger,
                            palette,
                        );
                        ui.separator();
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::ShiftReg),
                            "SHL",
                            "Shift Left (SHL)",
                            ActiveTool::ShiftLeft,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::ShiftReg),
                            "SHR",
                            "Shift Right (SHR)",
                            ActiveTool::ShiftRight,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::ShiftReg),
                            "SFT",
                            "Shift Register (Array)",
                            ActiveTool::ShiftReg,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::ShiftReg),
                            "SBIT",
                            "Bit Shift In (SFT_BIT)",
                            ActiveTool::ShiftRegInbit,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::ShiftReg),
                            "SHV",
                            "Shift Variable (SHV)",
                            ActiveTool::ShiftVariable,
                            palette.accent_warn,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::ShiftReg),
                            "SRST",
                            "Shift Reg Reset (SRST)",
                            ActiveTool::ShiftRegReset,
                            palette.accent_danger,
                            palette,
                        );
                    });

                    // 8. Communication & Serial
                    egui::CollapsingHeader::new(
                        RichText::new("📡 Communication")
                            .size(11.5)
                            .strong()
                            .color(palette.accent_heading),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::SerialComm),
                            "TX",
                            "UART Send (TX)",
                            ActiveTool::UartSend,
                            palette.accent_heading,
                            palette,
                        );
                        component_accordion_item(
                            ui,
                            app,
                            Some(VectorIcon::SerialComm),
                            "RX",
                            "UART Recv (RX)",
                            ActiveTool::UartRecv,
                            palette.accent_ok,
                            palette,
                        );
                    });
                }
            });
    });
    });
}

fn network_action_tile(
    ui: &mut Ui,
    icon: VectorIcon,
    label: &str,
    accent: Color32,
    enabled: bool,
    width: f32,
    palette: ThemePalette,
) -> egui::Response {
    let height = 34.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());

    if ui.is_rect_visible(rect) {
        let hovered = enabled && response.hovered();
        let pressed = enabled && response.is_pointer_button_down_on();

        let bg = if !enabled {
            if palette.is_light() {
                Color32::from_rgb(240, 243, 248)
            } else {
                palette.pill_bg.gamma_multiply(0.4)
            }
        } else if pressed {
            accent.gamma_multiply(0.3)
        } else if hovered {
            palette.rung_badge_hov_bg
        } else {
            palette.card_bg
        };

        let border_color = if !enabled {
            palette.rung_badge_border.gamma_multiply(0.35)
        } else if hovered || pressed {
            accent.gamma_multiply(0.85)
        } else {
            palette.rung_badge_border
        };

        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(5), bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(5),
            Stroke::new(if hovered { 1.3 } else { 1.0 }, border_color),
            StrokeKind::Inside,
        );

        // Icon color
        let icon_color = if !enabled {
            palette.rung_text_dim.gamma_multiply(0.35)
        } else if hovered {
            accent
        } else {
            accent.gamma_multiply(0.85)
        };

        // Top line: Icon
        let icon_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.min.y + 11.5),
            Vec2::new(14.0, 14.0),
        );
        paint_icon(painter, icon_rect, icon, icon_color);

        // Bottom line: Label text
        let lbl_color = if !enabled {
            palette.rung_text_dim.gamma_multiply(0.4)
        } else if hovered {
            palette.text_primary
        } else {
            palette.rung_text_num
        };
        painter.text(
            Pos2::new(rect.center().x, rect.min.y + 24.5),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(9.5),
            lbl_color,
        );
    }

    response
}

fn quick_tool_tile(
    ui: &mut Ui,
    app: &mut CanvasApp,
    symbol: &str,
    label: &str,
    tool: ActiveTool,
    accent: Color32,
    width: f32,
    palette: ThemePalette,
) -> egui::Response {
    let is_active = app.active_tool == tool;
    let height = 35.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let bg = if is_active {
            if palette.is_light() {
                Color32::from_rgb(218, 232, 252)
            } else {
                Color32::from_rgb(40, 58, 86)
            }
        } else if hovered {
            palette.rung_badge_hov_bg
        } else {
            palette.card_bg
        };

        let border_color = if is_active {
            if palette.is_light() {
                Color32::from_rgb(35, 110, 215)
            } else {
                Color32::from_rgb(85, 145, 230)
            }
        } else if hovered {
            accent.gamma_multiply(0.8)
        } else {
            palette.rung_badge_border
        };

        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(5), bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(5),
            Stroke::new(if is_active { 1.5 } else { 1.0 }, border_color),
            StrokeKind::Inside,
        );

        // Top line: Symbol in monospace bold
        let sym_color = if is_active {
            if palette.is_light() {
                Color32::from_rgb(18, 75, 175)
            } else {
                Color32::WHITE
            }
        } else {
            accent
        };
        painter.text(
            Pos2::new(rect.center().x, rect.min.y + 11.0),
            Align2::CENTER_CENTER,
            symbol,
            FontId::monospace(11.0),
            sym_color,
        );

        // Bottom line: Label
        let lbl_color = if is_active {
            if palette.is_light() {
                Color32::from_rgb(20, 28, 45)
            } else {
                palette.text_primary
            }
        } else {
            palette.rung_text_dim
        };
        painter.text(
            Pos2::new(rect.center().x, rect.min.y + 24.5),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(9.5),
            lbl_color,
        );
    }

    if response.drag_started() || (response.dragged() && app.canvas.dragging_tool.is_none()) {
        app.canvas.start_tool_drag(tool, label);
    }
    if response.clicked() {
        if app.active_tool == tool {
            app.active_tool = ActiveTool::Select;
        } else {
            app.active_tool = tool;
        }
    }

    response.on_hover_text(format!("{} (Click to select, drag to canvas)", tool.name()))
}

fn component_accordion_item(
    ui: &mut Ui,
    app: &mut CanvasApp,
    icon: Option<VectorIcon>,
    badge: &str,
    name: &str,
    tool: ActiveTool,
    accent: Color32,
    palette: ThemePalette,
) {
    let is_active = app.active_tool == tool;
    let width = ui.available_width();
    let height = 26.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let bg = if is_active {
            if palette.is_light() {
                Color32::from_rgb(220, 234, 252)
            } else {
                Color32::from_rgb(38, 55, 82)
            }
        } else if hovered {
            palette.rung_badge_hov_bg
        } else {
            Color32::TRANSPARENT
        };

        let border_color = if is_active {
            if palette.is_light() {
                Color32::from_rgb(45, 120, 225)
            } else {
                Color32::from_rgb(85, 145, 230)
            }
        } else if hovered {
            palette.card_border
        } else {
            Color32::TRANSPARENT
        };

        let painter = ui.painter();
        if is_active || hovered {
            painter.rect_filled(rect, CornerRadius::same(4), bg);
            painter.rect_stroke(
                rect,
                CornerRadius::same(4),
                Stroke::new(1.0, border_color),
                StrokeKind::Inside,
            );
        }

        // Left Symbol Badge
        let badge_w = 34.0;
        let badge_rect = Rect::from_min_max(
            Pos2::new(rect.min.x + 3.0, rect.min.y + 3.0),
            Pos2::new(rect.min.x + 3.0 + badge_w, rect.max.y - 3.0),
        );
        let badge_bg = if is_active {
            accent.gamma_multiply(0.25)
        } else {
            palette.pill_bg
        };
        painter.rect_filled(badge_rect, CornerRadius::same(3), badge_bg);
        painter.rect_stroke(
            badge_rect,
            CornerRadius::same(3),
            Stroke::new(
                0.8,
                if is_active {
                    accent
                } else {
                    palette.banner_border.gamma_multiply(0.6)
                },
            ),
            StrokeKind::Inside,
        );

        if let Some(ico) = icon {
            let ico_color = if is_active {
                accent
            } else {
                accent.gamma_multiply(0.85)
            };
            paint_icon(painter, badge_rect.shrink(2.0), ico, ico_color);
        } else {
            let badge_text_color = if is_active {
                accent
            } else if palette.is_light() {
                Color32::from_rgb(30, 40, 55)
            } else {
                palette.rung_text_num
            };
            painter.text(
                badge_rect.center(),
                Align2::CENTER_CENTER,
                badge,
                FontId::monospace(10.0),
                badge_text_color,
            );
        }

        // Right Name text
        let text_color = if is_active {
            if palette.is_light() {
                Color32::from_rgb(18, 25, 40)
            } else {
                Color32::WHITE
            }
        } else if hovered {
            palette.text_primary
        } else {
            palette.rung_text_num
        };

        painter.text(
            Pos2::new(rect.min.x + badge_w + 8.0, rect.center().y),
            Align2::LEFT_CENTER,
            name,
            FontId::proportional(11.0),
            text_color,
        );
    }

    if response.drag_started() || (response.dragged() && app.canvas.dragging_tool.is_none()) {
        app.canvas.start_tool_drag(tool, name);
    }
    if response.clicked() {
        if app.active_tool == tool {
            app.active_tool = ActiveTool::Select;
        } else {
            app.active_tool = tool;
        }
    }

    response.on_hover_text(format!("{} (Click to select, drag to canvas)", tool.name()));
}

struct ComponentDef {
    icon: Option<VectorIcon>,
    badge: &'static str,
    name: &'static str,
    tool: ActiveTool,
    category: &'static str,
    accent_kind: u8,
}

impl ComponentDef {
    fn accent(&self, palette: ThemePalette) -> Color32 {
        match self.accent_kind {
            0 => palette.accent_ok,
            1 => palette.accent_warn,
            2 => palette.accent_heading,
            3 => palette.coil_idle,
            4 => palette.accent_danger,
            _ => palette.wire_idle,
        }
    }
}

const ALL_COMPONENTS: &[ComponentDef] = &[
    // Bit Logic
    ComponentDef {
        icon: Some(VectorIcon::ContactNo),
        badge: "-[ ]-",
        name: "NO Contact [ ]",
        tool: ActiveTool::ContactNo,
        category: "Bit Logic",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::ContactNc),
        badge: "-[/]-",
        name: "NC Contact [/]",
        tool: ActiveTool::ContactNc,
        category: "Bit Logic",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::PulseEdge),
        badge: "-[P]-",
        name: "P Contact (Rising)",
        tool: ActiveTool::ContactP,
        category: "Bit Logic",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::PulseEdge),
        badge: "-[N]-",
        name: "N Contact (Falling)",
        tool: ActiveTool::ContactN,
        category: "Bit Logic",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::Coil),
        badge: "-( )-",
        name: "Coil (Normal Out)",
        tool: ActiveTool::CoilNormal,
        category: "Bit Logic",
        accent_kind: 3,
    },
    ComponentDef {
        icon: Some(VectorIcon::Coil),
        badge: "-(/)-",
        name: "Negated Coil (/)",
        tool: ActiveTool::CoilNegated,
        category: "Bit Logic",
        accent_kind: 4,
    },
    ComponentDef {
        icon: Some(VectorIcon::CoilSet),
        badge: "-(S)-",
        name: "Set Latch Coil (S)",
        tool: ActiveTool::CoilSet,
        category: "Bit Logic",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::CoilReset),
        badge: "-(R)-",
        name: "Reset Coil (R)",
        tool: ActiveTool::CoilReset,
        category: "Bit Logic",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::PulseEdge),
        badge: "-(P)-",
        name: "Positive Edge Coil",
        tool: ActiveTool::CoilP,
        category: "Bit Logic",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::PulseEdge),
        badge: "-(N)-",
        name: "Negative Edge Coil",
        tool: ActiveTool::CoilN,
        category: "Bit Logic",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "SR",
        name: "SR Bistable FB",
        tool: ActiveTool::Sr,
        category: "Bit Logic",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "RS",
        name: "RS Bistable FB",
        tool: ActiveTool::Rs,
        category: "Bit Logic",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::MasterRelay),
        badge: "MCR",
        name: "Master Relay (MC)",
        tool: ActiveTool::MasterRelay,
        category: "Bit Logic",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::ParallelBranch),
        badge: "-[∥]-",
        name: "Parallel Branch",
        tool: ActiveTool::Branch,
        category: "Bit Logic",
        accent_kind: 0,
    },
    ComponentDef {
        icon: None,
        badge: "───",
        name: "Short Circuit Wire",
        tool: ActiveTool::Wire,
        category: "Bit Logic",
        accent_kind: 5,
    },
    ComponentDef {
        icon: None,
        badge: "─ ─",
        name: "Open Circuit",
        tool: ActiveTool::OpenCircuit,
        category: "Bit Logic",
        accent_kind: 4,
    },
    // Timers & Counters
    ComponentDef {
        icon: Some(VectorIcon::Timer),
        badge: "TON",
        name: "TON (On-Delay)",
        tool: ActiveTool::Ton,
        category: "Timers & Counters",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::Timer),
        badge: "TOF",
        name: "TOF (Off-Delay)",
        tool: ActiveTool::Tof,
        category: "Timers & Counters",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::Timer),
        badge: "TP",
        name: "TP (Pulse Timer)",
        tool: ActiveTool::Tp,
        category: "Timers & Counters",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::Counter),
        badge: "CTU",
        name: "CTU (Count Up)",
        tool: ActiveTool::Ctu,
        category: "Timers & Counters",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::Counter),
        badge: "CTD",
        name: "CTD (Count Down)",
        tool: ActiveTool::Ctd,
        category: "Timers & Counters",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::Counter),
        badge: "CTUD",
        name: "CTUD (Up/Down)",
        tool: ActiveTool::Ctud,
        category: "Timers & Counters",
        accent_kind: 1,
    },
    // Edge Triggers
    ComponentDef {
        icon: Some(VectorIcon::PulseEdge),
        badge: "R_TRG",
        name: "R_TRIG (Rising Edge)",
        tool: ActiveTool::RTrig,
        category: "Edge Triggers",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::PulseEdge),
        badge: "F_TRG",
        name: "F_TRIG (Falling Edge)",
        tool: ActiveTool::FTrig,
        category: "Edge Triggers",
        accent_kind: 1,
    },
    // Math & Compare
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: ":=",
        name: "Move [:=]",
        tool: ActiveTool::Move,
        category: "Math & Compare",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "CMP",
        name: "Compare [==, !=, ...]",
        tool: ActiveTool::Compare,
        category: "Math & Compare",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "+=",
        name: "Add [+=]",
        tool: ActiveTool::Add,
        category: "Math & Compare",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "-=",
        name: "Sub [-=]",
        tool: ActiveTool::Sub,
        category: "Math & Compare",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "*=",
        name: "Mul [*=]",
        tool: ActiveTool::Mul,
        category: "Math & Compare",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "/=",
        name: "Div [/=]",
        tool: ActiveTool::Div,
        category: "Math & Compare",
        accent_kind: 4,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "%=",
        name: "Mod [%=]",
        tool: ActiveTool::Mod,
        category: "Math & Compare",
        accent_kind: 2,
    },
    // Program Flow
    ComponentDef {
        icon: Some(VectorIcon::MoveDown),
        badge: "JMP",
        name: "JMP (Jump to Label)",
        tool: ActiveTool::Jump,
        category: "Program Flow",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::MoveUp),
        badge: "LBL",
        name: "LBL (Target Label)",
        tool: ActiveTool::Label,
        category: "Program Flow",
        accent_kind: 0,
    },
    // Process Control
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "PID",
        name: "PID Controller",
        tool: ActiveTool::Pid,
        category: "Process Control",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "SCL",
        name: "Scale (Linear Map)",
        tool: ActiveTool::Scale,
        category: "Process Control",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "LIM",
        name: "Limit (Clamp Min/Max)",
        tool: ActiveTool::Limit,
        category: "Process Control",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "ADC",
        name: "A/D Converter Read",
        tool: ActiveTool::Adc,
        category: "Process Control",
        accent_kind: 2,
    },
    // Bitwise & Shift
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "AND",
        name: "Bitwise AND [&]",
        tool: ActiveTool::BitAnd,
        category: "Bitwise & Shift",
        accent_kind: 0,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "OR",
        name: "Bitwise OR [|]",
        tool: ActiveTool::BitOr,
        category: "Bitwise & Shift",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "XOR",
        name: "Bitwise XOR [^]",
        tool: ActiveTool::BitXor,
        category: "Bitwise & Shift",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::FunctionBlock),
        badge: "NOT",
        name: "Bitwise NOT [~]",
        tool: ActiveTool::BitNot,
        category: "Bitwise & Shift",
        accent_kind: 4,
    },
    ComponentDef {
        icon: Some(VectorIcon::ShiftReg),
        badge: "SHL",
        name: "Shift Left (SHL)",
        tool: ActiveTool::ShiftLeft,
        category: "Bitwise & Shift",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::ShiftReg),
        badge: "SHR",
        name: "Shift Right (SHR)",
        tool: ActiveTool::ShiftRight,
        category: "Bitwise & Shift",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::ShiftReg),
        badge: "SFT",
        name: "Shift Register (Array)",
        tool: ActiveTool::ShiftReg,
        category: "Bitwise & Shift",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::ShiftReg),
        badge: "SBIT",
        name: "Bit Shift In (SFT_BIT)",
        tool: ActiveTool::ShiftRegInbit,
        category: "Bitwise & Shift",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::ShiftReg),
        badge: "SHV",
        name: "Shift Variable (SHV)",
        tool: ActiveTool::ShiftVariable,
        category: "Bitwise & Shift",
        accent_kind: 1,
    },
    ComponentDef {
        icon: Some(VectorIcon::ShiftReg),
        badge: "SRST",
        name: "Shift Reg Reset (SRST)",
        tool: ActiveTool::ShiftRegReset,
        category: "Bitwise & Shift",
        accent_kind: 4,
    },
    // Communication
    ComponentDef {
        icon: Some(VectorIcon::SerialComm),
        badge: "TX",
        name: "UART Send (TX)",
        tool: ActiveTool::UartSend,
        category: "Communication",
        accent_kind: 2,
    },
    ComponentDef {
        icon: Some(VectorIcon::SerialComm),
        badge: "RX",
        name: "UART Recv (RX)",
        tool: ActiveTool::UartRecv,
        category: "Communication",
        accent_kind: 0,
    },
];
