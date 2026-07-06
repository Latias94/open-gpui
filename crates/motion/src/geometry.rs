//! Renderer-neutral geometry values for UI component state.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// A renderer-neutral logical pixel scalar.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct MotionPx(f32);

impl MotionPx {
    /// Zero logical pixels.
    pub const ZERO: Self = Self(0.0);

    /// One logical pixel.
    pub const ONE: Self = Self(1.0);

    /// Creates a logical pixel value.
    pub const fn new(value: f32) -> Self {
        Self(value)
    }

    /// Returns the raw scalar value.
    pub const fn as_f32(self) -> f32 {
        self.0
    }

    /// Returns half of this value.
    pub const fn half(self) -> Self {
        Self(self.0 * 0.5)
    }

    /// Returns the smaller of two pixel values.
    pub const fn min(self, other: Self) -> Self {
        if self.0 <= other.0 { self } else { other }
    }

    /// Returns the larger of two pixel values.
    pub const fn max(self, other: Self) -> Self {
        if self.0 >= other.0 { self } else { other }
    }
}

impl Add for MotionPx {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for MotionPx {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Neg for MotionPx {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Mul<f32> for MotionPx {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Div<f32> for MotionPx {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self(self.0 / rhs)
    }
}

/// Creates a renderer-neutral logical pixel scalar.
pub const fn motion_px(value: f32) -> MotionPx {
    MotionPx::new(value)
}

/// A renderer-neutral point.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MotionPoint {
    /// Horizontal coordinate.
    pub x: MotionPx,
    /// Vertical coordinate.
    pub y: MotionPx,
}

impl MotionPoint {
    /// Creates a point from x and y coordinates.
    pub const fn new(x: MotionPx, y: MotionPx) -> Self {
        Self { x, y }
    }
}

/// Creates a renderer-neutral point.
pub const fn motion_point(x: MotionPx, y: MotionPx) -> MotionPoint {
    MotionPoint::new(x, y)
}

/// A renderer-neutral size.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MotionSize {
    /// Horizontal extent.
    pub width: MotionPx,
    /// Vertical extent.
    pub height: MotionPx,
}

impl MotionSize {
    /// Creates a size from width and height.
    pub const fn new(width: MotionPx, height: MotionPx) -> Self {
        Self { width, height }
    }
}

/// Creates a renderer-neutral size.
pub const fn motion_size(width: MotionPx, height: MotionPx) -> MotionSize {
    MotionSize::new(width, height)
}

/// A renderer-neutral rectangle.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MotionRect {
    /// Top-left rectangle origin.
    pub origin: MotionPoint,
    /// Rectangle extent.
    pub size: MotionSize,
}

impl MotionRect {
    /// Creates a rectangle from origin and size.
    pub const fn new(origin: MotionPoint, size: MotionSize) -> Self {
        Self { origin, size }
    }

    /// Returns the rectangle top-left point.
    pub const fn top_left(self) -> MotionPoint {
        self.origin
    }

    /// Returns the rectangle top-center point.
    pub fn top_center(self) -> MotionPoint {
        MotionPoint::new(self.origin.x + self.size.width.half(), self.origin.y)
    }

    /// Returns the rectangle top-right point.
    pub fn top_right(self) -> MotionPoint {
        MotionPoint::new(self.origin.x + self.size.width, self.origin.y)
    }

    /// Returns the rectangle right-center point.
    pub fn right_center(self) -> MotionPoint {
        MotionPoint::new(
            self.origin.x + self.size.width,
            self.origin.y + self.size.height.half(),
        )
    }

    /// Returns the rectangle bottom-left point.
    pub fn bottom_left(self) -> MotionPoint {
        MotionPoint::new(self.origin.x, self.origin.y + self.size.height)
    }

    /// Returns the rectangle bottom-center point.
    pub fn bottom_center(self) -> MotionPoint {
        MotionPoint::new(
            self.origin.x + self.size.width.half(),
            self.origin.y + self.size.height,
        )
    }

    /// Returns the rectangle bottom-right point.
    pub fn bottom_right(self) -> MotionPoint {
        MotionPoint::new(
            self.origin.x + self.size.width,
            self.origin.y + self.size.height,
        )
    }

    /// Returns the rectangle left-center point.
    pub fn left_center(self) -> MotionPoint {
        MotionPoint::new(self.origin.x, self.origin.y + self.size.height.half())
    }

    /// Returns a rectangle inset by the same amount on every side.
    pub fn inset(self, amount: MotionPx) -> Self {
        let double = MotionPx::new(amount.as_f32() * 2.0);
        Self {
            origin: MotionPoint::new(self.origin.x + amount, self.origin.y + amount),
            size: MotionSize::new(self.size.width - double, self.size.height - double),
        }
    }
}

/// Creates a renderer-neutral rectangle.
pub const fn motion_rect(origin: MotionPoint, size: MotionSize) -> MotionRect {
    MotionRect::new(origin, size)
}

/// Renderer-neutral edge insets.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MotionEdges {
    /// Top edge inset.
    pub top: MotionPx,
    /// Right edge inset.
    pub right: MotionPx,
    /// Bottom edge inset.
    pub bottom: MotionPx,
    /// Left edge inset.
    pub left: MotionPx,
}

impl MotionEdges {
    /// Creates edge insets from top, right, bottom, and left values.
    pub const fn new(top: MotionPx, right: MotionPx, bottom: MotionPx, left: MotionPx) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Creates equal edge insets on every side.
    pub const fn uniform(value: MotionPx) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

/// Creates renderer-neutral edge insets.
pub const fn motion_edges(
    top: MotionPx,
    right: MotionPx,
    bottom: MotionPx,
    left: MotionPx,
) -> MotionEdges {
    MotionEdges::new(top, right, bottom, left)
}
