//! Renderer-neutral geometry values for UI component state.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// A renderer-neutral logical pixel scalar.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct UiPx(f32);

impl UiPx {
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

impl Add for UiPx {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for UiPx {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Neg for UiPx {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Mul<f32> for UiPx {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Div<f32> for UiPx {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self(self.0 / rhs)
    }
}

/// Creates a renderer-neutral logical pixel scalar.
pub const fn ui_px(value: f32) -> UiPx {
    UiPx::new(value)
}

/// A renderer-neutral point.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UiPoint {
    /// Horizontal coordinate.
    pub x: UiPx,
    /// Vertical coordinate.
    pub y: UiPx,
}

impl UiPoint {
    /// Creates a point from x and y coordinates.
    pub const fn new(x: UiPx, y: UiPx) -> Self {
        Self { x, y }
    }
}

/// Creates a renderer-neutral point.
pub const fn ui_point(x: UiPx, y: UiPx) -> UiPoint {
    UiPoint::new(x, y)
}

/// A renderer-neutral size.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UiSize {
    /// Horizontal extent.
    pub width: UiPx,
    /// Vertical extent.
    pub height: UiPx,
}

impl UiSize {
    /// Creates a size from width and height.
    pub const fn new(width: UiPx, height: UiPx) -> Self {
        Self { width, height }
    }
}

/// Creates a renderer-neutral size.
pub const fn ui_size(width: UiPx, height: UiPx) -> UiSize {
    UiSize::new(width, height)
}

/// A renderer-neutral rectangle.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UiRect {
    /// Top-left rectangle origin.
    pub origin: UiPoint,
    /// Rectangle extent.
    pub size: UiSize,
}

impl UiRect {
    /// Creates a rectangle from origin and size.
    pub const fn new(origin: UiPoint, size: UiSize) -> Self {
        Self { origin, size }
    }

    /// Returns the rectangle top-left point.
    pub const fn top_left(self) -> UiPoint {
        self.origin
    }

    /// Returns the rectangle top-center point.
    pub fn top_center(self) -> UiPoint {
        UiPoint::new(self.origin.x + self.size.width.half(), self.origin.y)
    }

    /// Returns the rectangle top-right point.
    pub fn top_right(self) -> UiPoint {
        UiPoint::new(self.origin.x + self.size.width, self.origin.y)
    }

    /// Returns the rectangle right-center point.
    pub fn right_center(self) -> UiPoint {
        UiPoint::new(
            self.origin.x + self.size.width,
            self.origin.y + self.size.height.half(),
        )
    }

    /// Returns the rectangle bottom-left point.
    pub fn bottom_left(self) -> UiPoint {
        UiPoint::new(self.origin.x, self.origin.y + self.size.height)
    }

    /// Returns the rectangle bottom-center point.
    pub fn bottom_center(self) -> UiPoint {
        UiPoint::new(
            self.origin.x + self.size.width.half(),
            self.origin.y + self.size.height,
        )
    }

    /// Returns the rectangle bottom-right point.
    pub fn bottom_right(self) -> UiPoint {
        UiPoint::new(
            self.origin.x + self.size.width,
            self.origin.y + self.size.height,
        )
    }

    /// Returns the rectangle left-center point.
    pub fn left_center(self) -> UiPoint {
        UiPoint::new(self.origin.x, self.origin.y + self.size.height.half())
    }

    /// Returns a rectangle inset by the same amount on every side.
    pub fn inset(self, amount: UiPx) -> Self {
        let double = UiPx::new(amount.as_f32() * 2.0);
        Self {
            origin: UiPoint::new(self.origin.x + amount, self.origin.y + amount),
            size: UiSize::new(self.size.width - double, self.size.height - double),
        }
    }
}

/// Creates a renderer-neutral rectangle.
pub const fn ui_rect(origin: UiPoint, size: UiSize) -> UiRect {
    UiRect::new(origin, size)
}

/// Renderer-neutral edge insets.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UiEdges {
    /// Top edge inset.
    pub top: UiPx,
    /// Right edge inset.
    pub right: UiPx,
    /// Bottom edge inset.
    pub bottom: UiPx,
    /// Left edge inset.
    pub left: UiPx,
}

impl UiEdges {
    /// Creates edge insets from top, right, bottom, and left values.
    pub const fn new(top: UiPx, right: UiPx, bottom: UiPx, left: UiPx) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Creates equal edge insets on every side.
    pub const fn uniform(value: UiPx) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

/// Creates renderer-neutral edge insets.
pub const fn ui_edges(top: UiPx, right: UiPx, bottom: UiPx, left: UiPx) -> UiEdges {
    UiEdges::new(top, right, bottom, left)
}
