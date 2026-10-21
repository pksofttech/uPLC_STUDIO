use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Rect, Response, Sense,
    Stroke, StrokeKind, Ui,
};
use crate::theme::ThemePalette;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorIcon {
    // Rung / Ladder Tools
    Undo,
    Redo,
    NewRung,
    DeleteRung,
    MoveUp,
    MoveDown,
    CopyRung,
    CopyUp,
    CopyDown,
    // Insertion Positions
    InsertEnd,
    InsertLeft,
    InsertRight,
    InsertTopPar,
    InsertBotPar,
    // Ladder Component Icons
    CursorSelect,
    Comment,
    ContactNo,
    ContactNc,
    Coil,
    CoilSet,
    CoilReset,
    MasterRelay,
    Timer,
    Counter,
    ShiftReg,
    FunctionBlock,
    PulseEdge,
    SerialComm,
    ParallelBranch,
    // System, File & Simulation Icons
    NewFile,
    OpenFile,
    SaveFile,
    ExportCode,
    ConfigGear,
    Play,
    Step,
    Stop,
    Reset,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    HandPan,
}

/// Draw a crisp, font-independent vector icon inside `rect`
pub fn paint_icon(painter: &Painter, rect: Rect, icon: VectorIcon, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.42;
    let s = Stroke::new(1.7, color);
    let s_thin = Stroke::new(1.3, color);
    let s_bold = Stroke::new(2.1, color);

    match icon {
        VectorIcon::Undo => {
            // Horizontal bar + top return hook + left arrow head
            let p_right = pos2(c.x + r * 0.75, c.y + r * 0.45);
            let p_mid = pos2(c.x + r * 0.75, c.y - r * 0.25);
            let p_left = pos2(c.x - r * 0.65, c.y - r * 0.25);
            painter.line_segment([pos2(c.x - r * 0.2, p_right.y), p_right], s);
            painter.line_segment([p_right, p_mid], s);
            painter.line_segment([p_mid, p_left], s);
            // Left arrow tip
            painter.line_segment([p_left, pos2(p_left.x + r * 0.45, p_left.y - r * 0.4)], s);
            painter.line_segment([p_left, pos2(p_left.x + r * 0.45, p_left.y + r * 0.4)], s);
        }
        VectorIcon::Redo => {
            let p_left = pos2(c.x - r * 0.75, c.y + r * 0.45);
            let p_mid = pos2(c.x - r * 0.75, c.y - r * 0.25);
            let p_right = pos2(c.x + r * 0.65, c.y - r * 0.25);
            painter.line_segment([pos2(c.x + r * 0.2, p_left.y), p_left], s);
            painter.line_segment([p_left, p_mid], s);
            painter.line_segment([p_mid, p_right], s);
            // Right arrow tip
            painter.line_segment([p_right, pos2(p_right.x - r * 0.45, p_right.y - r * 0.4)], s);
            painter.line_segment([p_right, pos2(p_right.x - r * 0.45, p_right.y + r * 0.4)], s);
        }
        VectorIcon::NewRung => {
            // Left rail + rung wire + plus symbol
            let lx = c.x - r * 0.85;
            painter.line_segment([pos2(lx, c.y - r * 0.85), pos2(lx, c.y + r * 0.85)], s_bold);
            painter.line_segment([pos2(lx, c.y), pos2(c.x - r * 0.15, c.y)], s);
            // Plus sign on right
            let px = c.x + r * 0.38;
            let pr = r * 0.52;
            painter.line_segment([pos2(px - pr, c.y), pos2(px + pr, c.y)], s_bold);
            painter.line_segment([pos2(px, c.y - pr), pos2(px, c.y + pr)], s_bold);
        }
        VectorIcon::DeleteRung => {
            // Modern Trash Can icon
            let top_y = c.y - r * 0.55;
            let bot_y = c.y + r * 0.80;
            let w_top = r * 0.72;
            let w_bot = r * 0.56;
            // Lid
            painter.line_segment([pos2(c.x - r * 0.85, top_y), pos2(c.x + r * 0.85, top_y)], s);
            painter.line_segment(
                [pos2(c.x - r * 0.3, top_y - r * 0.25), pos2(c.x + r * 0.3, top_y - r * 0.25)],
                s,
            );
            // Body
            painter.line_segment([pos2(c.x - w_top, top_y), pos2(c.x - w_bot, bot_y)], s);
            painter.line_segment([pos2(c.x + w_top, top_y), pos2(c.x + w_bot, bot_y)], s);
            painter.line_segment([pos2(c.x - w_bot, bot_y), pos2(c.x + w_bot, bot_y)], s);
            // Inner ribs
            painter.line_segment(
                [pos2(c.x - r * 0.22, top_y + r * 0.25), pos2(c.x - r * 0.18, bot_y - r * 0.2)],
                s_thin,
            );
            painter.line_segment(
                [pos2(c.x + r * 0.22, top_y + r * 0.25), pos2(c.x + r * 0.18, bot_y - r * 0.2)],
                s_thin,
            );
        }
        VectorIcon::MoveUp => {
            // Top bar + upward arrow
            painter.line_segment(
                [pos2(c.x - r * 0.75, c.y - r * 0.82), pos2(c.x + r * 0.75, c.y - r * 0.82)],
                s_thin,
            );
            let tip = pos2(c.x, c.y - r * 0.45);
            painter.line_segment([tip, pos2(c.x, c.y + r * 0.85)], s_bold);
            painter.line_segment([tip, pos2(c.x - r * 0.55, c.y + r * 0.1)], s_bold);
            painter.line_segment([tip, pos2(c.x + r * 0.55, c.y + r * 0.1)], s_bold);
        }
        VectorIcon::MoveDown => {
            // Bottom bar + downward arrow
            painter.line_segment(
                [pos2(c.x - r * 0.75, c.y + r * 0.82), pos2(c.x + r * 0.75, c.y + r * 0.82)],
                s_thin,
            );
            let tip = pos2(c.x, c.y + r * 0.45);
            painter.line_segment([pos2(c.x, c.y - r * 0.85), tip], s_bold);
            painter.line_segment([tip, pos2(c.x - r * 0.55, c.y - r * 0.1)], s_bold);
            painter.line_segment([tip, pos2(c.x + r * 0.55, c.y - r * 0.1)], s_bold);
        }
        VectorIcon::CopyRung => {
            // Back sheet + front sheet (theme-independent vector strokes)
            let r1 = Rect::from_min_max(
                pos2(c.x - r * 0.75, c.y - r * 0.75),
                pos2(c.x + r * 0.35, c.y + r * 0.40),
            );
            let r2 = Rect::from_min_max(
                pos2(c.x - r * 0.35, c.y - r * 0.35),
                pos2(c.x + r * 0.75, c.y + r * 0.80),
            );
            painter.line_segment([r1.left_top(), r1.right_top()], s_thin);
            painter.line_segment([r1.right_top(), pos2(r1.right(), r2.top())], s_thin);
            painter.line_segment([r1.left_top(), r1.left_bottom()], s_thin);
            painter.line_segment([r1.left_bottom(), pos2(r2.left(), r1.bottom())], s_thin);
            painter.rect_stroke(r2, 2.0, s, StrokeKind::Inside);
        }
        VectorIcon::CopyUp => {
            // Small sheet + up arrow with plus
            let tip = pos2(c.x - r * 0.35, c.y - r * 0.75);
            painter.line_segment([tip, pos2(tip.x, c.y + r * 0.75)], s);
            painter.line_segment([tip, pos2(tip.x - r * 0.4, tip.y + r * 0.45)], s);
            painter.line_segment([tip, pos2(tip.x + r * 0.4, tip.y + r * 0.45)], s);
            // Plus on right
            let px = c.x + r * 0.45;
            let py = c.y + r * 0.15;
            painter.line_segment([pos2(px - r * 0.35, py), pos2(px + r * 0.35, py)], s);
            painter.line_segment([pos2(px, py - r * 0.35), pos2(px, py + r * 0.35)], s);
        }
        VectorIcon::CopyDown => {
            let tip = pos2(c.x - r * 0.35, c.y + r * 0.75);
            painter.line_segment([pos2(tip.x, c.y - r * 0.75), tip], s);
            painter.line_segment([tip, pos2(tip.x - r * 0.4, tip.y - r * 0.45)], s);
            painter.line_segment([tip, pos2(tip.x + r * 0.4, tip.y - r * 0.45)], s);
            // Plus on right
            let px = c.x + r * 0.45;
            let py = c.y - r * 0.15;
            painter.line_segment([pos2(px - r * 0.35, py), pos2(px + r * 0.35, py)], s);
            painter.line_segment([pos2(px, py - r * 0.35), pos2(px, py + r * 0.35)], s);
        }
        VectorIcon::InsertEnd => {
            painter.line_segment([pos2(c.x - r * 0.85, c.y), pos2(c.x + r * 0.25, c.y)], s);
            painter.circle_filled(pos2(c.x + r * 0.55, c.y), r * 0.32, color);
        }
        VectorIcon::InsertLeft => {
            let tip = pos2(c.x - r * 0.65, c.y);
            painter.line_segment([tip, pos2(c.x + r * 0.75, c.y)], s);
            painter.line_segment([tip, pos2(tip.x + r * 0.45, c.y - r * 0.45)], s);
            painter.line_segment([tip, pos2(tip.x + r * 0.45, c.y + r * 0.45)], s);
        }
        VectorIcon::InsertRight => {
            let tip = pos2(c.x + r * 0.65, c.y);
            painter.line_segment([pos2(c.x - r * 0.75, c.y), tip], s);
            painter.line_segment([tip, pos2(tip.x - r * 0.45, c.y - r * 0.45)], s);
            painter.line_segment([tip, pos2(tip.x - r * 0.45, c.y + r * 0.45)], s);
        }
        VectorIcon::InsertTopPar => {
            let y_main = c.y + r * 0.45;
            let y_top = c.y - r * 0.45;
            painter.line_segment([pos2(c.x - r * 0.8, y_main), pos2(c.x + r * 0.8, y_main)], s_thin);
            painter.line_segment([pos2(c.x - r * 0.5, y_main), pos2(c.x - r * 0.5, y_top)], s);
            painter.line_segment([pos2(c.x + r * 0.5, y_main), pos2(c.x + r * 0.5, y_top)], s);
            painter.line_segment([pos2(c.x - r * 0.5, y_top), pos2(c.x + r * 0.5, y_top)], s_bold);
        }
        VectorIcon::InsertBotPar => {
            let y_main = c.y - r * 0.45;
            let y_bot = c.y + r * 0.45;
            painter.line_segment([pos2(c.x - r * 0.8, y_main), pos2(c.x + r * 0.8, y_main)], s_thin);
            painter.line_segment([pos2(c.x - r * 0.5, y_main), pos2(c.x - r * 0.5, y_bot)], s);
            painter.line_segment([pos2(c.x + r * 0.5, y_main), pos2(c.x + r * 0.5, y_bot)], s);
            painter.line_segment([pos2(c.x - r * 0.5, y_bot), pos2(c.x + r * 0.5, y_bot)], s_bold);
        }
        VectorIcon::CursorSelect => {
            // Crisp pointer arrow
            let p0 = pos2(c.x - r * 0.55, c.y - r * 0.80);
            let p1 = pos2(c.x - r * 0.55, c.y + r * 0.65);
            let p2 = pos2(c.x - r * 0.10, c.y + r * 0.25);
            let p3 = pos2(c.x + r * 0.55, c.y + r * 0.20);
            painter.line_segment([p0, p1], s_bold);
            painter.line_segment([p0, p3], s_bold);
            painter.line_segment([p1, p2], s);
            painter.line_segment([p3, p2], s);
            painter.line_segment([p2, pos2(c.x + r * 0.25, c.y + r * 0.80)], s_bold);
        }
        VectorIcon::Comment => {
            // Double slash //
            painter.line_segment(
                [pos2(c.x - r * 0.55, c.y + r * 0.65), pos2(c.x - r * 0.05, c.y - r * 0.65)],
                s_bold,
            );
            painter.line_segment(
                [pos2(c.x + r * 0.05, c.y + r * 0.65), pos2(c.x + r * 0.55, c.y - r * 0.65)],
                s_bold,
            );
        }
        VectorIcon::ContactNo => {
            // ─┤ ├─
            let gap = r * 0.34;
            let bh = r * 0.68;
            painter.line_segment([pos2(c.x - r, c.y), pos2(c.x - gap, c.y)], s_thin);
            painter.line_segment([pos2(c.x + gap, c.y), pos2(c.x + r, c.y)], s_thin);
            painter.line_segment([pos2(c.x - gap, c.y - bh), pos2(c.x - gap, c.y + bh)], s_bold);
            painter.line_segment([pos2(c.x + gap, c.y - bh), pos2(c.x + gap, c.y + bh)], s_bold);
        }
        VectorIcon::ContactNc => {
            // ─┤/├─
            let gap = r * 0.34;
            let bh = r * 0.68;
            painter.line_segment([pos2(c.x - r, c.y), pos2(c.x - gap, c.y)], s_thin);
            painter.line_segment([pos2(c.x + gap, c.y), pos2(c.x + r, c.y)], s_thin);
            painter.line_segment([pos2(c.x - gap, c.y - bh), pos2(c.x - gap, c.y + bh)], s_bold);
            painter.line_segment([pos2(c.x + gap, c.y - bh), pos2(c.x + gap, c.y + bh)], s_bold);
            painter.line_segment(
                [pos2(c.x - gap * 1.4, c.y + bh * 0.85), pos2(c.x + gap * 1.4, c.y - bh * 0.85)],
                s,
            );
        }
        VectorIcon::Coil | VectorIcon::CoilSet | VectorIcon::CoilReset => {
            // Standard IEC 61131-3 Parentheses: ─( )─
            let gap = r * 0.48;
            let h = r * 0.65;
            let tip = r * 0.20;
            painter.line_segment([pos2(c.x - r, c.y), pos2(c.x - gap, c.y)], s_thin);
            painter.line_segment([pos2(c.x + gap, c.y), pos2(c.x + r, c.y)], s_thin);

            let lp0 = pos2(c.x - gap + tip, c.y - h);
            let lp1 = pos2(c.x - gap - tip, c.y);
            let lp2 = pos2(c.x - gap + tip, c.y + h);
            let mut prev_l = lp0;
            for i in 1..=8 {
                let t = i as f32 / 8.0;
                let omt = 1.0 - t;
                let cur = pos2(
                    omt * omt * lp0.x + 2.0 * omt * t * lp1.x + t * t * lp2.x,
                    omt * omt * lp0.y + 2.0 * omt * t * lp1.y + t * t * lp2.y,
                );
                painter.line_segment([prev_l, cur], s_bold);
                prev_l = cur;
            }

            let rp0 = pos2(c.x + gap - tip, c.y - h);
            let rp1 = pos2(c.x + gap + tip, c.y);
            let rp2 = pos2(c.x + gap - tip, c.y + h);
            let mut prev_r = rp0;
            for i in 1..=8 {
                let t = i as f32 / 8.0;
                let omt = 1.0 - t;
                let cur = pos2(
                    omt * omt * rp0.x + 2.0 * omt * t * rp1.x + t * t * rp2.x,
                    omt * omt * rp0.y + 2.0 * omt * t * rp1.y + t * t * rp2.y,
                );
                painter.line_segment([prev_r, cur], s_bold);
                prev_r = cur;
            }

            if icon == VectorIcon::CoilSet {
                painter.text(c, Align2::CENTER_CENTER, "S", FontId::monospace(r * 0.95), color);
            } else if icon == VectorIcon::CoilReset {
                painter.text(c, Align2::CENTER_CENTER, "R", FontId::monospace(r * 0.95), color);
            }
        }
        VectorIcon::MasterRelay => {
            let box_r = Rect::from_center_size(c, vec2(r * 1.5, r * 1.25));
            painter.rect_stroke(box_r, 3.0, s, StrokeKind::Inside);
            painter.text(c, Align2::CENTER_CENTER, "M", FontId::monospace(r * 1.0), color);
        }
        VectorIcon::Timer => {
            // Stopwatch / Clock icon
            let cr = r * 0.72;
            painter.circle_stroke(pos2(c.x, c.y + r * 0.08), cr, s);
            // Top button
            painter.line_segment(
                [pos2(c.x - r * 0.25, c.y - r * 0.85), pos2(c.x + r * 0.25, c.y - r * 0.85)],
                s,
            );
            // Clock hands
            let cc = pos2(c.x, c.y + r * 0.08);
            painter.line_segment([cc, pos2(cc.x, cc.y - cr * 0.62)], s);
            painter.line_segment([cc, pos2(cc.x + cr * 0.52, cc.y + cr * 0.15)], s);
        }
        VectorIcon::Counter => {
            // Digital counter badge [12]
            let box_r = Rect::from_center_size(c, vec2(r * 1.65, r * 1.3));
            painter.rect_stroke(box_r, 3.0, s, StrokeKind::Inside);
            painter.text(c, Align2::CENTER_CENTER, "12", FontId::monospace(r * 0.95), color);
        }
        VectorIcon::ShiftReg => {
            // 3 bit cells + right arrow
            let box_r = Rect::from_center_size(pos2(c.x, c.y - r * 0.2), vec2(r * 1.7, r * 0.75));
            painter.rect_stroke(box_r, 2.0, s_thin, StrokeKind::Inside);
            painter.line_segment(
                [pos2(c.x - r * 0.28, box_r.min.y), pos2(c.x - r * 0.28, box_r.max.y)],
                s_thin,
            );
            painter.line_segment(
                [pos2(c.x + r * 0.28, box_r.min.y), pos2(c.x + r * 0.28, box_r.max.y)],
                s_thin,
            );
            // Arrow below
            let ay = c.y + r * 0.55;
            painter.line_segment([pos2(c.x - r * 0.65, ay), pos2(c.x + r * 0.65, ay)], s);
            painter.line_segment(
                [pos2(c.x + r * 0.65, ay), pos2(c.x + r * 0.25, ay - r * 0.28)],
                s,
            );
        }
        VectorIcon::FunctionBlock => {
            let box_r = Rect::from_center_size(c, vec2(r * 1.65, r * 1.35));
            painter.rect_stroke(box_r, 3.0, s, StrokeKind::Inside);
            painter.text(c, Align2::CENTER_CENTER, "fx", FontId::monospace(r * 0.95), color);
        }
        VectorIcon::PulseEdge => {
            // Rising step pulse waveform _/‾\_
            let y_lo = c.y + r * 0.55;
            let y_hi = c.y - r * 0.55;
            let x0 = c.x - r * 0.85;
            let x1 = c.x - r * 0.25;
            let x2 = c.x + r * 0.35;
            let x3 = c.x + r * 0.85;
            painter.line_segment([pos2(x0, y_lo), pos2(x1, y_lo)], s);
            painter.line_segment([pos2(x1, y_lo), pos2(x1, y_hi)], s_bold);
            painter.line_segment([pos2(x1, y_hi), pos2(x2, y_hi)], s);
            painter.line_segment([pos2(x2, y_hi), pos2(x2, y_lo)], s);
            painter.line_segment([pos2(x2, y_lo), pos2(x3, y_lo)], s);
        }
        VectorIcon::SerialComm => {
            // Two opposing data arrows (TX / RX)
            let y1 = c.y - r * 0.35;
            let y2 = c.y + r * 0.35;
            painter.line_segment([pos2(c.x - r * 0.75, y1), pos2(c.x + r * 0.75, y1)], s);
            painter.line_segment(
                [pos2(c.x + r * 0.75, y1), pos2(c.x + r * 0.3, y1 - r * 0.32)],
                s,
            );
            painter.line_segment([pos2(c.x - r * 0.75, y2), pos2(c.x + r * 0.75, y2)], s);
            painter.line_segment(
                [pos2(c.x - r * 0.75, y2), pos2(c.x - r * 0.3, y2 + r * 0.32)],
                s,
            );
        }
        VectorIcon::ParallelBranch => {
            let y_top = c.y - r * 0.45;
            let y_bot = c.y + r * 0.45;
            let xl = c.x - r * 0.65;
            let xr = c.x + r * 0.65;
            painter.line_segment([pos2(c.x - r, y_top), pos2(c.x + r, y_top)], s_thin);
            painter.line_segment([pos2(xl, y_top), pos2(xl, y_bot)], s);
            painter.line_segment([pos2(xr, y_top), pos2(xr, y_bot)], s);
            painter.line_segment([pos2(xl, y_bot), pos2(xr, y_bot)], s_bold);
            painter.circle_filled(pos2(xl, y_top), 2.0, color);
            painter.circle_filled(pos2(xr, y_top), 2.0, color);
        }
        VectorIcon::NewFile => {
            let w = r * 0.70;
            let h = r * 0.90;
            let fold = r * 0.45;
            let p_tl = pos2(c.x - w, c.y - h);
            let p_tr_fold = pos2(c.x + w - fold, c.y - h);
            let p_fold_r = pos2(c.x + w, c.y - h + fold);
            let p_br = pos2(c.x + w, c.y + h);
            let p_bl = pos2(c.x - w, c.y + h);

            painter.line_segment([p_tl, p_tr_fold], s);
            painter.line_segment([p_tr_fold, p_fold_r], s);
            painter.line_segment([p_fold_r, p_br], s);
            painter.line_segment([p_br, p_bl], s);
            painter.line_segment([p_bl, p_tl], s);

            let p_corner = pos2(c.x + w - fold, c.y - h + fold);
            painter.line_segment([p_tr_fold, p_corner], s_thin);
            painter.line_segment([p_corner, p_fold_r], s_thin);

            let px = c.x;
            let py = c.y + r * 0.20;
            let pr = r * 0.35;
            painter.line_segment([pos2(px - pr, py), pos2(px + pr, py)], s_bold);
            painter.line_segment([pos2(px, py - pr), pos2(px, py + pr)], s_bold);
        }
        VectorIcon::OpenFile => {
            let w = r * 0.85;
            let h = r * 0.65;
            let tab_w = r * 0.50;
            let tab_h = r * 0.22;
            let y_top = c.y - h * 0.5;
            let y_bot = c.y + h * 0.75;
            let x_l = c.x - w;
            let x_r = c.x + w;

            painter.line_segment([pos2(x_l, y_top), pos2(x_l + tab_w, y_top)], s);
            painter.line_segment(
                [pos2(x_l + tab_w, y_top), pos2(x_l + tab_w + r * 0.2, y_top + tab_h)],
                s,
            );
            painter.line_segment(
                [pos2(x_l + tab_w + r * 0.2, y_top + tab_h), pos2(x_r - r * 0.15, y_top + tab_h)],
                s_thin,
            );

            let flap_tl = pos2(x_l - r * 0.05, y_top + tab_h);
            let flap_tr = pos2(x_r + r * 0.05, y_top + tab_h);
            let flap_br = pos2(x_r - r * 0.15, y_bot);
            let flap_bl = pos2(x_l, y_bot);

            painter.line_segment([flap_tl, flap_tr], s);
            painter.line_segment([flap_tr, flap_br], s);
            painter.line_segment([flap_br, flap_bl], s);
            painter.line_segment([flap_bl, flap_tl], s);
        }
        VectorIcon::SaveFile => {
            let w = r * 0.78;
            let h = r * 0.82;
            let box_r = Rect::from_center_size(c, vec2(w * 2.0, h * 2.0));
            painter.rect_stroke(box_r, 2.0, s, StrokeKind::Inside);

            let shut_r = Rect::from_min_max(
                pos2(c.x - w * 0.55, box_r.min.y),
                pos2(c.x + w * 0.55, c.y - h * 0.2),
            );
            painter.rect_stroke(shut_r, 1.0, s_thin, StrokeKind::Inside);
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(c.x - w * 0.25, box_r.min.y + 1.0),
                    pos2(c.x - w * 0.05, c.y - h * 0.35),
                ),
                0.0,
                color,
            );

            let lbl_r = Rect::from_min_max(
                pos2(c.x - w * 0.65, c.y + h * 0.05),
                pos2(c.x + w * 0.65, box_r.max.y - 1.5),
            );
            painter.rect_stroke(lbl_r, 1.0, s_thin, StrokeKind::Inside);
        }
        VectorIcon::ExportCode => {
            let lx = c.x - r * 0.60;
            painter.line_segment([pos2(lx + r * 0.32, c.y - r * 0.52), pos2(lx, c.y)], s_bold);
            painter.line_segment([pos2(lx, c.y), pos2(lx + r * 0.32, c.y + r * 0.52)], s_bold);

            painter.line_segment(
                [pos2(c.x - r * 0.12, c.y + r * 0.70), pos2(c.x + r * 0.12, c.y - r * 0.70)],
                s,
            );

            let rx = c.x + r * 0.60;
            painter.line_segment([pos2(rx - r * 0.32, c.y - r * 0.52), pos2(rx, c.y)], s_bold);
            painter.line_segment([pos2(rx, c.y), pos2(rx - r * 0.32, c.y + r * 0.52)], s_bold);
        }
        VectorIcon::ConfigGear => {
            let y1 = c.y - r * 0.55;
            let y2 = c.y;
            let y3 = c.y + r * 0.55;
            let x_min = c.x - r * 0.85;
            let x_max = c.x + r * 0.85;

            painter.line_segment([pos2(x_min, y1), pos2(x_max, y1)], s_thin);
            painter.line_segment([pos2(x_min, y2), pos2(x_max, y2)], s_thin);
            painter.line_segment([pos2(x_min, y3), pos2(x_max, y3)], s_thin);

            let k_h = r * 0.35;
            let k_w = r * 0.22;
            let k1_x = c.x - r * 0.30;
            painter.rect_filled(Rect::from_center_size(pos2(k1_x, y1), vec2(k_w, k_h)), 1.5, color);

            let k2_x = c.x + r * 0.35;
            painter.rect_filled(Rect::from_center_size(pos2(k2_x, y2), vec2(k_w, k_h)), 1.5, color);

            let k3_x = c.x - r * 0.10;
            painter.rect_filled(Rect::from_center_size(pos2(k3_x, y3), vec2(k_w, k_h)), 1.5, color);
        }
        VectorIcon::Play => {
            let left_x = c.x - r * 0.55;
            let right_x = c.x + r * 0.75;
            let top_y = c.y - r * 0.75;
            let bot_y = c.y + r * 0.75;

            let p0 = pos2(left_x, top_y);
            let p1 = pos2(right_x, c.y);
            let p2 = pos2(left_x, bot_y);

            let mesh = egui::epaint::Mesh {
                indices: vec![0, 1, 2],
                vertices: vec![
                    egui::epaint::Vertex { pos: p0, uv: egui::epaint::WHITE_UV, color },
                    egui::epaint::Vertex { pos: p1, uv: egui::epaint::WHITE_UV, color },
                    egui::epaint::Vertex { pos: p2, uv: egui::epaint::WHITE_UV, color },
                ],
                ..Default::default()
            };
            painter.add(egui::Shape::mesh(mesh));
        }
        VectorIcon::Step => {
            let left_x = c.x - r * 0.65;
            let right_x = c.x + r * 0.30;
            let top_y = c.y - r * 0.65;
            let bot_y = c.y + r * 0.65;

            let p0 = pos2(left_x, top_y);
            let p1 = pos2(right_x, c.y);
            let p2 = pos2(left_x, bot_y);

            let mesh = egui::epaint::Mesh {
                indices: vec![0, 1, 2],
                vertices: vec![
                    egui::epaint::Vertex { pos: p0, uv: egui::epaint::WHITE_UV, color },
                    egui::epaint::Vertex { pos: p1, uv: egui::epaint::WHITE_UV, color },
                    egui::epaint::Vertex { pos: p2, uv: egui::epaint::WHITE_UV, color },
                ],
                ..Default::default()
            };
            painter.add(egui::Shape::mesh(mesh));

            let bar_x = c.x + r * 0.65;
            painter.line_segment([pos2(bar_x, top_y), pos2(bar_x, bot_y)], s_bold);
        }
        VectorIcon::Stop => {
            let s_size = r * 1.35;
            let sq_rect = Rect::from_center_size(c, vec2(s_size, s_size));
            painter.rect_filled(sq_rect, CornerRadius::same(3), color);
        }
        VectorIcon::Reset => {
            let radius = r * 0.72;
            let start_angle = -std::f32::consts::FRAC_PI_2 + 0.3;
            let end_angle = std::f32::consts::PI + 0.9;
            let steps = 14;
            let mut points = Vec::with_capacity(steps + 1);
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let angle = start_angle + t * (end_angle - start_angle);
                points.push(pos2(c.x + radius * angle.cos(), c.y + radius * angle.sin()));
            }
            for i in 0..steps {
                painter.line_segment([points[i], points[i + 1]], s);
            }
            let tip = points[0];
            painter.line_segment([tip, pos2(tip.x + r * 0.38, tip.y - r * 0.12)], s);
            painter.line_segment([tip, pos2(tip.x + r * 0.12, tip.y + r * 0.38)], s);
        }
        VectorIcon::ZoomIn => {
            let glass_c = pos2(c.x - r * 0.18, c.y - r * 0.18);
            let glass_r = r * 0.58;
            painter.circle_stroke(glass_c, glass_r, s);
            let h_start = pos2(glass_c.x + glass_r * 0.707, glass_c.y + glass_r * 0.707);
            let h_end = pos2(c.x + r * 0.85, c.y + r * 0.85);
            painter.line_segment([h_start, h_end], s_bold);
            let pl = r * 0.30;
            painter.line_segment([pos2(glass_c.x - pl, glass_c.y), pos2(glass_c.x + pl, glass_c.y)], s);
            painter.line_segment([pos2(glass_c.x, glass_c.y - pl), pos2(glass_c.x, glass_c.y + pl)], s);
        }
        VectorIcon::ZoomOut => {
            let glass_c = pos2(c.x - r * 0.18, c.y - r * 0.18);
            let glass_r = r * 0.58;
            painter.circle_stroke(glass_c, glass_r, s);
            let h_start = pos2(glass_c.x + glass_r * 0.707, glass_c.y + glass_r * 0.707);
            let h_end = pos2(c.x + r * 0.85, c.y + r * 0.85);
            painter.line_segment([h_start, h_end], s_bold);
            let pl = r * 0.30;
            painter.line_segment([pos2(glass_c.x - pl, glass_c.y), pos2(glass_c.x + pl, glass_c.y)], s);
        }
        VectorIcon::ZoomFit => {
            let box_r = r * 0.68;
            let rect = Rect::from_center_size(c, vec2(box_r * 2.0, box_r * 2.0));
            painter.rect_stroke(rect, CornerRadius::same(2), s_thin, StrokeKind::Outside);
            let dot_r = r * 0.22;
            painter.circle_filled(c, dot_r, color);
        }
        VectorIcon::HandPan => {
            let wrist = pos2(c.x, c.y + r * 0.80);
            let palm = pos2(c.x, c.y + r * 0.15);
            painter.line_segment([wrist, palm], s_bold);
            let f1 = pos2(c.x - r * 0.45, c.y - r * 0.45);
            let f2 = pos2(c.x - r * 0.15, c.y - r * 0.80);
            let f3 = pos2(c.x + r * 0.15, c.y - r * 0.75);
            let f4 = pos2(c.x + r * 0.45, c.y - r * 0.40);
            painter.line_segment([pos2(c.x - r * 0.30, palm.y), f1], s);
            painter.line_segment([pos2(c.x - r * 0.10, palm.y), f2], s);
            painter.line_segment([pos2(c.x + r * 0.10, palm.y), f3], s);
            painter.line_segment([pos2(c.x + r * 0.30, palm.y), f4], s);
        }
    }
}

/// Modern versatile toolbar button with vector icon, label, active state, disabled state, and drag-and-drop
pub fn modern_tool_button(
    ui: &mut Ui,
    icon: Option<VectorIcon>,
    label: &str,
    accent: Color32,
    is_active: bool,
    is_enabled: bool,
    allow_drag: bool,
    width: f32,
    palette: ThemePalette,
) -> Response {
    let height = 26.0;
    let sense = if !is_enabled {
        Sense::hover()
    } else if allow_drag {
        Sense::click_and_drag()
    } else {
        Sense::click()
    };

    let (rect, response) = ui.allocate_exact_size(vec2(width, height), sense);

    if ui.is_rect_visible(rect) {
        let hovered = is_enabled && response.hovered();
        let active_press = is_enabled && response.is_pointer_button_down_on();

        let bg = if !is_enabled {
            palette.pill_bg.gamma_multiply(0.45)
        } else if is_active {
            accent.gamma_multiply(0.24)
        } else if active_press {
            accent.gamma_multiply(0.38)
        } else if hovered {
            palette.rung_badge_hov_bg
        } else {
            palette.card_bg
        };

        let border = if !is_enabled {
            palette.rung_badge_border.gamma_multiply(0.45)
        } else if is_active {
            accent
        } else if hovered || active_press {
            accent.gamma_multiply(0.85)
        } else {
            palette.rung_badge_border
        };

        let fg = if !is_enabled {
            palette.rung_text_dim.gamma_multiply(0.48)
        } else if is_active {
            if palette.is_light() {
                Color32::from_rgb(18, 25, 40)
            } else {
                palette.text_primary
            }
        } else if hovered {
            palette.text_primary
        } else {
            palette.rung_text_num
        };

        let icon_color = if !is_enabled {
            palette.rung_text_dim.gamma_multiply(0.42)
        } else if is_active || hovered {
            accent.gamma_multiply(0.95)
        } else {
            accent.gamma_multiply(0.80)
        };

        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(3), bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(3),
            Stroke::new(if is_active || hovered { 1.3 } else { 1.0 }, border),
            StrokeKind::Inside,
        );

        // Active bottom indicator bar
        if is_active && is_enabled {
            let ind_rect = Rect::from_min_max(
                pos2(rect.min.x + 3.0, rect.max.y - 2.5),
                pos2(rect.max.x - 3.0, rect.max.y - 0.8),
            );
            painter.rect_filled(ind_rect, CornerRadius::same(1), accent);
        }

        if let Some(ico) = icon {
            if label.is_empty() {
                let icon_rect = Rect::from_center_size(rect.center(), vec2(15.0, 15.0));
                paint_icon(painter, icon_rect, ico, icon_color);
            } else {
                let icon_rect = Rect::from_center_size(
                    pos2(rect.min.x + 11.5, rect.center().y),
                    vec2(14.5, 14.5),
                );
                paint_icon(painter, icon_rect, ico, icon_color);

                painter.text(
                    pos2(rect.min.x + 22.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(11.0),
                    fg,
                );
            }
        } else {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(11.0),
                fg,
            );
        }
    }

    response
}

/// Modern compact tile button with a custom vector icon on the left and a crisp label
pub fn modern_tool_tile(
    ui: &mut Ui,
    icon: VectorIcon,
    label: &str,
    accent: Color32,
    enabled: bool,
    width: f32,
    palette: ThemePalette,
) -> Response {
    modern_tool_button(
        ui,
        Some(icon),
        label,
        accent,
        false,
        enabled,
        false,
        width,
        palette,
    )
}

/// Modern component button with schematic vector icon + left-aligned label + active pill border
pub fn modern_component_button(
    ui: &mut Ui,
    icon: VectorIcon,
    label: &str,
    accent: Color32,
    is_selected: bool,
    palette: ThemePalette,
) -> Response {
    let width = ui.available_width();
    let height = 28.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let bg = if is_selected {
            palette.rung_badge_sel_bg
        } else if hovered {
            palette.rung_badge_hov_bg
        } else {
            palette.card_bg
        };

        let border = if is_selected {
            palette.rung_badge_sel_border
        } else if hovered {
            accent.gamma_multiply(0.85)
        } else {
            palette.rung_badge_border
        };

        let fg = if is_selected {
            palette.rung_text_sel
        } else if hovered {
            palette.text_primary
        } else {
            palette.rung_text_num
        };

        let icon_fg = if is_selected || hovered {
            accent.gamma_multiply(0.95)
        } else {
            accent.gamma_multiply(0.82)
        };

        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(6), bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(if is_selected || hovered { 1.4 } else { 1.0 }, border),
            StrokeKind::Inside,
        );

        // Left icon badge pill
        let badge_rect = Rect::from_min_max(
            pos2(rect.min.x + 4.0, rect.min.y + 4.0),
            pos2(rect.min.x + 28.0, rect.max.y - 4.0),
        );
        painter.rect_filled(
            badge_rect,
            CornerRadius::same(4),
            accent.gamma_multiply(0.12),
        );
        paint_icon(painter, badge_rect.shrink(2.0), icon, icon_fg);

        painter.text(
            pos2(rect.min.x + 35.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.0),
            fg,
        );
    }

    response
}
