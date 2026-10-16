use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ColorRgb {
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255 };
    pub const RED: Self = Self { r: 240, g: 80, b: 80 };
    pub const GREEN: Self = Self { r: 80, g: 220, b: 120 };
    pub const BLUE: Self = Self { r: 80, g: 150, b: 240 };
    pub const YELLOW: Self = Self { r: 250, g: 220, b: 80 };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShapeKind {
    Line {
        start: Point2D,
        end: Point2D,
    },
    Rectangle {
        min: Point2D,
        max: Point2D,
    },
    Circle {
        center: Point2D,
        radius: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shape {
    pub id: u64,
    pub kind: ShapeKind,
    pub color: ColorRgb,
    pub stroke_width: f32,
}

impl Shape {
    pub fn new(id: u64, kind: ShapeKind, color: ColorRgb, stroke_width: f32) -> Self {
        Self {
            id,
            kind,
            color,
            stroke_width,
        }
    }
}
