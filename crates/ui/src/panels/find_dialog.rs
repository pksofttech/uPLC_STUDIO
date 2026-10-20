//! Find / Replace window (Ctrl+F, F3 / Shift+F3).
//!
//! Searches element `name / symbol / comment / target_reg / fb.in1 / fb.in2`
//! via [`Document::find_text`](my_canvas_core::Document::find_text) and supports
//! program-wide Replace All (single undo step).

use egui::{RichText, Window};
use my_canvas_core::document::{FindHit, FindOptions};

use crate::app::CanvasApp;

/// UI state of the Find window.
#[derive(Debug, Default)]
pub struct FindState {
    /// Window visible.
    pub open: bool,
    /// Text to find.
    pub query: String,
    /// Replacement text for Replace All.
    pub replace: String,
    /// Match case / Whole word.
    pub opts: FindOptions,
    /// Latest results (refreshed every frame while the window is open).
    pub results: Vec<FindHit>,
    /// Index into `results` of the current hit.
    pub cursor: Option<usize>,
    /// Focus the query field on the next frame.
    pub request_focus: bool,
    /// Feedback line (e.g. "Replaced 3 occurrence(s)").
    pub status: String,
}

impl FindState {
    /// Re-run the search against the current document.
    pub fn refresh(&mut self, doc: &my_canvas_core::Document) {
        self.results = doc.find_text(&self.query, self.opts);
        if let Some(c) = self.cursor {
            if c >= self.results.len() {
                self.cursor = if self.results.is_empty() { None } else { Some(self.results.len() - 1) };
            }
        }
    }

    /// Step the cursor by `delta` (wrapping). Returns the new index.
    pub fn step(&mut self, delta: isize) -> Option<usize> {
        let n = self.results.len();
        if n == 0 {
            self.cursor = None;
            return None;
        }
        let next = match self.cursor {
            None if delta >= 0 => 0,
            None => n - 1,
            Some(c) => (c as isize + delta).rem_euclid(n as isize) as usize,
        };
        self.cursor = Some(next);
        Some(next)
    }
}

/// Render the floating Find window (non-modal).
pub fn render(ctx: &egui::Context, app: &mut CanvasApp) {
    if !app.find.open {
        return;
    }
    app.find.refresh(&app.doc);

    let mut open = app.find.open;
    let mut goto: Option<usize> = None;
    let mut do_replace = false;
    let mut step: Option<isize> = None;

    Window::new("🔍 Find / Replace")
        .open(&mut open)
        .default_size([520.0, 360.0])
        .collapsible(false)
        .resizable(true)
        .show(ctx, |ui| {
            let st = &mut app.find;
            egui::Grid::new("find_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label("Find:");
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut st.query)
                        .hint_text("variable / address / symbol / comment")
                        .desired_width(300.0),
                );
                if st.request_focus {
                    resp.request_focus();
                    st.request_focus = false;
                }
                if resp.changed() {
                    st.cursor = None;
                    st.status.clear();
                }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    step = Some(if ui.input(|i| i.modifiers.shift) { -1 } else { 1 });
                    resp.request_focus();
                }
                ui.end_row();

                ui.label("Replace:");
                ui.add(egui::TextEdit::singleline(&mut st.replace).desired_width(300.0));
                ui.end_row();
            });

            ui.horizontal(|ui| {
                ui.checkbox(&mut st.opts.match_case, "Match case");
                ui.checkbox(&mut st.opts.whole_word, "Whole word");
                ui.separator();
                if ui.button("◀ Prev (Shift+F3)").clicked() {
                    step = Some(-1);
                }
                if ui.button("Next (F3) ▶").clicked() {
                    step = Some(1);
                }
                if ui
                    .add_enabled(!st.query.is_empty() && !st.results.is_empty(), egui::Button::new("Replace All"))
                    .clicked()
                {
                    do_replace = true;
                }
            });

            let summary = match (st.results.len(), st.cursor) {
                (0, _) if st.query.is_empty() => String::new(),
                (0, _) => "No matches".to_string(),
                (n, Some(c)) => format!("{} / {} match(es)", c + 1, n),
                (n, None) => format!("{} match(es)", n),
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(summary).weak());
                if !st.status.is_empty() {
                    ui.separator();
                    ui.label(RichText::new(&st.status).strong());
                }
            });
            ui.separator();

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("find_results")
                    .num_columns(4)
                    .striped(true)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Network #").strong());
                        ui.label(RichText::new("Type").strong());
                        ui.label(RichText::new("Field").strong());
                        ui.label(RichText::new("Text").strong());
                        ui.end_row();
                        for (i, hit) in st.results.iter().enumerate() {
                            let selected = st.cursor == Some(i);
                            if ui.selectable_label(selected, format!("{:02}", hit.rung_idx + 1)).clicked() {
                                goto = Some(i);
                            }
                            ui.label(format!("{:?}", hit.element_type));
                            ui.label(hit.field.label());
                            if ui.selectable_label(selected, RichText::new(&hit.text).monospace()).clicked() {
                                goto = Some(i);
                            }
                            ui.end_row();
                        }
                    });
            });
        });

    app.find.open = open;

    if do_replace {
        let n = app.doc.replace_all(&app.find.query, &app.find.replace, app.find.opts);
        app.find.status = format!("Replaced {} occurrence(s)", n);
        app.find.cursor = None;
        app.find.refresh(&app.doc);
    }
    if let Some(d) = step {
        app.find_step(d);
    } else if let Some(i) = goto {
        app.find.cursor = Some(i);
        app.goto_find_result(i);
    }
}
