use eframe::NativeOptions;
use my_canvas_ui::{AppSettings, CanvasApp};

fn generate_app_icon() -> egui::IconData {
    const SIZE: usize = 128;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];

    let set_pixel = |buf: &mut [u8], x: usize, y: usize, color: [u8; 4]| {
        if x < SIZE && y < SIZE {
            let idx = (y * SIZE + x) * 4;
            let sa = color[3] as f32 / 255.0;
            let da = buf[idx + 3] as f32 / 255.0;
            let out_a = sa + da * (1.0 - sa);
            if out_a > 0.0 {
                for c in 0..3 {
                    let sc = color[c] as f32;
                    let dc = buf[idx + c] as f32;
                    buf[idx + c] = ((sc * sa + dc * da * (1.0 - sa)) / out_a).round() as u8;
                }
                buf[idx + 3] = (out_a * 255.0).round() as u8;
            }
        }
    };

    // Metallic IC Pins on Top and Bottom
    let pin_xs = [32..40, 48..56, 72..80, 88..96];
    for yr in [8..22, 106..120] {
        for y in yr {
            for px in &pin_xs {
                for x in px.clone() {
                    // Metallic pin highlight gradient
                    let pin_val = if x == px.start || x == px.end - 1 { 140 } else { 210 };
                    set_pixel(&mut rgba, x, y, [pin_val, pin_val + 10, pin_val + 25, 255]);
                }
            }
        }
    }

    // Rounded squircle chip body (center: 64, 64, half-size: 44, corner radius: 18)
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = if x < 38 { 38.0 - x as f32 } else if x > 89 { x as f32 - 89.0 } else { 0.0 };
            let dy = if y < 38 { 38.0 - y as f32 } else if y > 89 { y as f32 - 89.0 } else { 0.0 };
            let dist = (dx * dx + dy * dy).sqrt();

            if x >= 20 && x <= 107 && y >= 20 && y <= 107 && dist <= 18.0 {
                if dist >= 14.5 || x <= 22 || x >= 105 || y <= 22 || y >= 105 {
                    // Outer cyan neon border
                    set_pixel(&mut rgba, x, y, [14, 165, 233, 245]); // Cyan #0EA5E9
                } else if dist >= 12.5 {
                    // Border glow blend
                    set_pixel(&mut rgba, x, y, [0, 220, 255, 180]);
                } else {
                    // Dark metallic chip background with gradient
                    let grad = ((y as f32 - 20.0) / 88.0 * 28.0) as u8;
                    set_pixel(&mut rgba, x, y, [15 + grad / 2, 22 + grad / 2, 36 + grad, 255]);
                }
            }
        }
    }

    // Left Rail (L1 Hot Line) - Energetic Amber Gold
    for y in 36..92 {
        for x in 31..35 {
            set_pixel(&mut rgba, x, y, [245, 158, 11, 255]); // #F59E0B
        }
        set_pixel(&mut rgba, 32, y, [251, 191, 36, 255]); // Core highlight
        set_pixel(&mut rgba, 33, y, [251, 191, 36, 255]);
    }

    // Right Rail (L2 Neutral Line) - Slate Silver
    for y in 36..92 {
        for x in 93..97 {
            set_pixel(&mut rgba, x, y, [148, 163, 184, 255]); // #94A3B8
        }
        set_pixel(&mut rgba, 94, y, [203, 213, 225, 255]); // Core highlight
        set_pixel(&mut rgba, 95, y, [203, 213, 225, 255]);
    }

    // Wire from L1 to Contact
    for x in 35..46 {
        for y in 62..66 {
            set_pixel(&mut rgba, x, y, [0, 230, 255, 255]);
        }
    }

    // Ladder Normally-Open Contact ┤ ├ in Bright Electric Cyan
    for y in 50..78 {
        for x in 46..50 {
            set_pixel(&mut rgba, x, y, [0, 240, 255, 255]);
        }
        for x in 56..60 {
            set_pixel(&mut rgba, x, y, [0, 240, 255, 255]);
        }
    }

    // Middle Wire from Contact to Coil
    for x in 60..70 {
        for y in 62..66 {
            set_pixel(&mut rgba, x, y, [0, 230, 255, 255]);
        }
    }

    // Ladder Coil ( ) in Vivid Emerald Green
    for y in 50..78 {
        let curve = match y {
            50 | 77 => 4,
            51..=53 | 74..=76 => 3,
            54..=56 | 71..=73 => 2,
            57..=59 | 68..=70 => 1,
            _ => 0,
        };
        // Left parenthesis '('
        for dx in 0..4 {
            set_pixel(&mut rgba, 72 + curve + dx, y, [16, 225, 130, 255]);
        }
        // Right parenthesis ')'
        for dx in 0..4 {
            set_pixel(&mut rgba, 84 - curve - dx, y, [16, 225, 130, 255]);
        }
    }

    // Wire from Coil to L2 Rail
    for x in 82..93 {
        for y in 62..66 {
            set_pixel(&mut rgba, x, y, [0, 230, 255, 255]);
        }
    }

    // Microchip Pin-1 Index Indicator (Top-Left circle)
    for dy in -3..=3 {
        for dx in -3..=3 {
            if dx * dx + dy * dy <= 10 {
                let px = (29 + dx) as usize;
                let py = (29 + dy) as usize;
                set_pixel(&mut rgba, px, py, [0, 220, 255, 255]);
            }
        }
    }

    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}

fn main() -> eframe::Result<()> {
    let settings = AppSettings::load();
    let (width, height) = settings.clamped_window_size();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("uPLC Studio")
        .with_app_id("uplc-studio")
        .with_icon(generate_app_icon())
        .with_inner_size([width, height])
        .with_min_inner_size([960.0, 600.0]);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_app_icon() {
        let icon = generate_app_icon();
        assert_eq!(icon.width, 128);
        assert_eq!(icon.height, 128);
        assert_eq!(icon.rgba.len(), 128 * 128 * 4);

        // Export icon for desktop shortcut and packaging
        let target_dir = if std::path::Path::new("crates").exists() {
            "assets"
        } else {
            "../../assets"
        };
        let _ = std::fs::create_dir_all(target_dir);
        if let Some(img) = image::RgbaImage::from_raw(icon.width, icon.height, icon.rgba) {
            let _ = img.save(format!("{}/uplc-studio.png", target_dir));
        }
    }
}


