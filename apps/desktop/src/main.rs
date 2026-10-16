use eframe::NativeOptions;
use my_canvas_ui::{AppSettings, CanvasApp};

fn main() -> eframe::Result<()> {
    let settings = AppSettings::load();
    let (width, height) = settings.clamped_window_size();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("uPLC Studio")
        .with_inner_size([width, height])
        .with_min_inner_size([800.0, 600.0]);

    if let (Some(x), Some(y)) = (settings.window_pos_x, settings.window_pos_y) {
        if x >= 0.0 && y >= 0.0 && x < 10000.0 && y < 10000.0 {
            viewport = viewport.with_position([x, y]);
        }
    }

    if settings.window_maximized {
        viewport = viewport.with_maximized(true);
    }

    let native_options = NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "uPLC Studio",
        native_options,
        Box::new(|_cc| Ok(Box::new(CanvasApp::default()))),
    )
}
