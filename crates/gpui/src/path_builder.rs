use anyhow::{Error, bail};
use etagere::euclid::{Point2D, Vector2D};
use lyon_path::geom::{Angle, CubicBezierSegment, QuadraticBezierSegment};
use lyon_path::math::{Vector, vector};
use lyon_path::traits::SvgPathBuilder;
use lyon_path::{ArcFlags, Event, Polygon};

pub use lyon_path::math::Transform;
pub use lyon_path::{FillRule, LineCap, LineJoin};

use crate::{
    Bounds, PathCurve, PathTiles, Pixels, Point, ScaledPixels, SubtreeTransformError, point, px,
};
use std::sync::Arc;

/// Style of the PathBuilder
pub enum PathStyle {
    /// Stroke style
    Stroke(StrokeOptions),
    /// Fill style
    Fill(FillOptions),
}

/// How to stroke a path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeOptions {
    /// Width of the stroke.
    pub line_width: f32,
    /// Shape of the ends of open subpaths and dashes.
    pub line_cap: LineCap,
    /// Shape of corners.
    pub line_join: LineJoin,
    /// How long a miter join may get, as a multiple of the line width,
    /// before it is beveled instead.
    pub miter_limit: f32,
}

impl Default for StrokeOptions {
    fn default() -> Self {
        Self {
            line_width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
        }
    }
}

/// How to fill a path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FillOptions {
    /// How overlapping contours combine into the filled region.
    pub fill_rule: FillRule,
}

impl Default for FillOptions {
    fn default() -> Self {
        Self {
            fill_rule: FillRule::EvenOdd,
        }
    }
}

/// A [`Path`] builder.
pub struct PathBuilder {
    raw: lyon_path::builder::WithSvg<lyon_path::BuilderImpl>,
    transform: Option<lyon_path::math::Transform>,
    /// PathStyle of the PathBuilder
    pub style: PathStyle,
    dash_array: Option<Vec<Pixels>>,
}

impl From<lyon_path::Builder> for PathBuilder {
    fn from(builder: lyon_path::Builder) -> Self {
        Self {
            raw: builder.with_svg(),
            ..Default::default()
        }
    }
}

impl From<lyon_path::builder::WithSvg<lyon_path::BuilderImpl>> for PathBuilder {
    fn from(raw: lyon_path::builder::WithSvg<lyon_path::BuilderImpl>) -> Self {
        Self {
            raw,
            ..Default::default()
        }
    }
}

impl From<lyon_path::math::Point> for Point<Pixels> {
    fn from(p: lyon_path::math::Point) -> Self {
        point(px(p.x), px(p.y))
    }
}

impl From<Point<Pixels>> for lyon_path::math::Point {
    fn from(p: Point<Pixels>) -> Self {
        lyon_path::math::point(p.x.0, p.y.0)
    }
}

impl From<Point<Pixels>> for Vector {
    fn from(p: Point<Pixels>) -> Self {
        vector(p.x.0, p.y.0)
    }
}

impl From<Point<Pixels>> for Point2D<f32, Pixels> {
    fn from(p: Point<Pixels>) -> Self {
        Point2D::new(p.x.0, p.y.0)
    }
}

impl Default for PathBuilder {
    fn default() -> Self {
        Self {
            raw: lyon_path::Path::builder().with_svg(),
            style: PathStyle::Fill(FillOptions::default()),
            transform: None,
            dash_array: None,
        }
    }
}

impl PathBuilder {
    /// Creates a new [`PathBuilder`] to build a Stroke path.
    pub fn stroke(width: Pixels) -> Self {
        Self {
            style: PathStyle::Stroke(StrokeOptions {
                line_width: width.0,
                ..Default::default()
            }),
            ..Self::default()
        }
    }

    /// Creates a new [`PathBuilder`] to build a Fill path.
    pub fn fill() -> Self {
        Self::default()
    }

    /// Sets the style of the [`PathBuilder`].
    pub fn with_style(self, style: PathStyle) -> Self {
        Self { style, ..self }
    }

    /// Sets the dash array of the [`PathBuilder`]. Fills ignore it.
    ///
    /// [MDN](https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Attribute/stroke-dasharray)
    pub fn dash_array(mut self, dash_array: &[Pixels]) -> Self {
        // If an odd number of values is provided, then the list of values is repeated to yield an even number of values.
        // Thus, 5,3,2 is equivalent to 5,3,2,5,3,2.
        let array = if dash_array.len() % 2 == 1 {
            let mut new_dash_array = dash_array.to_vec();
            new_dash_array.extend_from_slice(dash_array);
            new_dash_array
        } else {
            dash_array.to_vec()
        };

        self.dash_array = Some(array);
        self
    }

    /// Move the current point to the given point.
    #[inline]
    pub fn move_to(&mut self, to: Point<Pixels>) {
        self.raw.move_to(to.into());
    }

    /// Draw a straight line from the current point to the given point.
    #[inline]
    pub fn line_to(&mut self, to: Point<Pixels>) {
        self.raw.line_to(to.into());
    }

    /// Draw a curve from the current point to the given point, using the given control point.
    #[inline]
    pub fn curve_to(&mut self, to: Point<Pixels>, ctrl: Point<Pixels>) {
        self.raw.quadratic_bezier_to(ctrl.into(), to.into());
    }

    /// Adds a cubic Bézier to the [`Path`] given its two control points
    /// and its end point.
    #[inline]
    pub fn cubic_bezier_to(
        &mut self,
        to: Point<Pixels>,
        control_a: Point<Pixels>,
        control_b: Point<Pixels>,
    ) {
        self.raw
            .cubic_bezier_to(control_a.into(), control_b.into(), to.into());
    }

    /// Adds an elliptical arc.
    pub fn arc_to(
        &mut self,
        radii: Point<Pixels>,
        x_rotation: Pixels,
        large_arc: bool,
        sweep: bool,
        to: Point<Pixels>,
    ) {
        self.raw.arc_to(
            radii.into(),
            Angle::degrees(x_rotation.into()),
            ArcFlags { large_arc, sweep },
            to.into(),
        );
    }

    /// Equivalent to `arc_to` in relative coordinates.
    pub fn relative_arc_to(
        &mut self,
        radii: Point<Pixels>,
        x_rotation: Pixels,
        large_arc: bool,
        sweep: bool,
        to: Point<Pixels>,
    ) {
        self.raw.relative_arc_to(
            radii.into(),
            Angle::degrees(x_rotation.into()),
            ArcFlags { large_arc, sweep },
            to.into(),
        );
    }

    /// Adds a polygon.
    pub fn add_polygon(&mut self, points: &[Point<Pixels>], closed: bool) {
        let points = points.iter().copied().map(|p| p.into()).collect::<Vec<_>>();
        self.raw.add_polygon(Polygon {
            points: points.as_ref(),
            closed,
        });
    }

    /// Close the current sub-path.
    #[inline]
    pub fn close(&mut self) {
        self.raw.close();
    }

    /// Applies a transform to the path.
    #[inline]
    pub fn transform(&mut self, transform: Transform) {
        self.transform = Some(transform);
    }

    /// Applies a translation to the path.
    #[inline]
    pub fn translate(&mut self, to: Point<Pixels>) {
        if let Some(transform) = self.transform {
            self.transform = Some(transform.then_translate(Vector2D::new(to.x.0, to.y.0)));
        } else {
            self.transform = Some(Transform::translation(to.x.0, to.y.0))
        }
    }

    /// Applies a scale to the path.
    #[inline]
    pub fn scale(&mut self, scale: f32) {
        if let Some(transform) = self.transform {
            self.transform = Some(transform.then_scale(scale, scale));
        } else {
            self.transform = Some(Transform::scale(scale, scale));
        }
    }

    /// Applies a rotation to the path.
    ///
    /// The `angle` is in degrees value in the range 0.0 to 360.0.
    #[inline]
    pub fn rotate(&mut self, angle: f32) {
        let radians = angle.to_radians();
        if let Some(transform) = self.transform {
            self.transform = Some(transform.then_rotate(Angle::radians(radians)));
        } else {
            self.transform = Some(Transform::rotation(Angle::radians(radians)));
        }
    }

    /// Builds into a [`Path`].
    pub fn build(self) -> Result<Path, Error> {
        let (curves, fill_rule) = self.outline()?;
        Ok(Path::from_curves(curves, fill_rule))
    }

    /// The closed contours to fill, and the rule to fill them by.
    pub(crate) fn outline(self) -> Result<(Vec<PathCurve>, FillRule), Error> {
        let path = if let Some(transform) = self.transform {
            self.raw.build().transformed(&transform)
        } else {
            self.raw.build()
        };
        let finite = |point: lyon_path::math::Point| point.x.is_finite() && point.y.is_finite();
        if !path.iter().all(|event| match event {
            Event::Begin { at } => finite(at),
            Event::Line { to, .. } => finite(to),
            Event::Quadratic { ctrl, to, .. } => finite(ctrl) && finite(to),
            Event::Cubic {
                ctrl1, ctrl2, to, ..
            } => finite(ctrl1) && finite(ctrl2) && finite(to),
            Event::End { .. } => true,
        }) {
            bail!("path has a non-finite point");
        }

        Ok(match self.style {
            PathStyle::Stroke(options) => (
                Self::stroke_outline(&path, &options, self.dash_array.as_deref())?,
                FillRule::NonZero,
            ),
            PathStyle::Fill(options) => (Self::fill_outline(&path)?, options.fill_rule),
        })
    }

    fn stroke_outline(
        path: &lyon_path::Path,
        options: &StrokeOptions,
        dash_array: Option<&[Pixels]>,
    ) -> Result<Vec<PathCurve>, Error> {
        let outline = skia_path(path)
            .map(|skia_path| match dash_array {
                Some(dash_array) => {
                    let array = dash_array.iter().map(|length| length.0).collect();
                    match tiny_skia_path::StrokeDash::new(array, 0.0) {
                        Some(dash) => skia_path
                            .dash(&dash, STROKE_RESOLUTION_SCALE)
                            .unwrap_or(skia_path),
                        // An invalid dash array strokes undashed, as in SVG.
                        None => skia_path,
                    }
                }
                None => skia_path,
            })
            .and_then(|skia_path| {
                skia_path.stroke(
                    &tiny_skia_path::Stroke {
                        width: options.line_width,
                        miter_limit: options.miter_limit,
                        line_cap: match options.line_cap {
                            LineCap::Butt => tiny_skia_path::LineCap::Butt,
                            LineCap::Square => tiny_skia_path::LineCap::Square,
                            LineCap::Round => tiny_skia_path::LineCap::Round,
                        },
                        line_join: match options.line_join {
                            LineJoin::Miter => tiny_skia_path::LineJoin::Miter,
                            LineJoin::MiterClip => tiny_skia_path::LineJoin::MiterClip,
                            LineJoin::Round => tiny_skia_path::LineJoin::Round,
                            LineJoin::Bevel => tiny_skia_path::LineJoin::Bevel,
                        },
                        dash: None,
                    },
                    STROKE_RESOLUTION_SCALE,
                )
            });
        let Some(outline) = outline else {
            return Ok(Vec::new());
        };

        let mut output = Outline::default();
        let at = |p: tiny_skia_path::Point| point(p.x, p.y);
        for segment in outline.segments() {
            match segment {
                tiny_skia_path::PathSegment::MoveTo(p) => output.move_to(at(p)),
                tiny_skia_path::PathSegment::LineTo(p) => output.line_to(at(p)),
                tiny_skia_path::PathSegment::QuadTo(ctrl, p) => output.curve_to(at(p), at(ctrl)),
                tiny_skia_path::PathSegment::CubicTo(ctrl1, ctrl2, p) => {
                    output.cubic_to(at(p), at(ctrl1), at(ctrl2))
                }
                tiny_skia_path::PathSegment::Close => output.close(),
            }
        }
        output.into_curves()
    }

    /// Convert a lyon path into a fill, closing every subpath.
    fn fill_outline(path: &lyon_path::Path) -> Result<Vec<PathCurve>, Error> {
        let mut output = Outline::default();
        let at = |p: lyon_path::math::Point| point(p.x, p.y);
        for event in path.iter() {
            match event {
                Event::Begin { at: p } => output.move_to(at(p)),
                Event::Line { to, .. } => output.line_to(at(to)),
                Event::Quadratic { ctrl, to, .. } => output.curve_to(at(to), at(ctrl)),
                Event::Cubic {
                    ctrl1, ctrl2, to, ..
                } => output.cubic_to(at(to), at(ctrl1), at(ctrl2)),
                Event::End { .. } => output.close(),
            }
        }
        output.into_curves()
    }
}

/// A filled path, ready to paint. Build one with [`PathBuilder`].
#[derive(Clone, Debug)]
pub struct Path {
    /// Closed contours of monotone curves, in logical pixels.
    curves: Arc<[PathCurve]>,
    even_odd: bool,
}

impl Path {
    pub(crate) fn from_curves(curves: Vec<PathCurve>, fill_rule: FillRule) -> Self {
        Self {
            curves: curves.into(),
            even_odd: matches!(fill_rule, FillRule::EvenOdd),
        }
    }

    pub(crate) fn even_odd(&self) -> bool {
        self.even_odd
    }

    /// The local geometry bounds after display scaling, before the primitive
    /// transform. Gradients and stripes use these bounds in local device pixels.
    pub(crate) fn try_bounds(
        &self,
        scale_factor: f32,
    ) -> Result<Bounds<ScaledPixels>, SubtreeTransformError> {
        if !scale_factor.is_normal() || scale_factor <= 0.0 {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        if self.curves.is_empty() {
            return Ok(Bounds::default());
        }
        let mut min = point(f32::INFINITY, f32::INFINITY);
        let mut max = point(f32::NEG_INFINITY, f32::NEG_INFINITY);
        for endpoint in self.curves.iter().flat_map(|curve| [curve.p0, curve.p1]) {
            let endpoint = point(endpoint.x * scale_factor, endpoint.y * scale_factor);
            if !endpoint.x.is_finite() || !endpoint.y.is_finite() {
                return Err(SubtreeTransformError::UnrepresentableResult);
            }
            min.x = min.x.min(endpoint.x);
            min.y = min.y.min(endpoint.y);
            max.x = max.x.max(endpoint.x);
            max.y = max.y.max(endpoint.y);
        }
        if !(max.x - min.x).is_finite() || !(max.y - min.y).is_finite() {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        Ok(Bounds::from_corners(
            point(ScaledPixels(min.x), ScaledPixels(min.y)),
            point(ScaledPixels(max.x), ScaledPixels(max.y)),
        ))
    }

    /// Bin the curves into tiles in device pixels, for painting at the given
    /// display scale.
    pub(crate) fn bin(&self, scale: f32) -> PathTiles {
        crate::path_winding::bin(&self.curves, self.even_odd, scale)
    }

    /// Bin after display scaling and the positive axis-aligned primitive
    /// transform. Curves, tile bounds and coverage margins are in final device pixels.
    /// Unrepresentable curve coordinates fail conversion; grid overflow still skips painting.
    pub(crate) fn try_bin_transformed(
        &self,
        scale_factor: f32,
        transform: crate::PrimitiveTransform,
    ) -> Result<PathTiles, SubtreeTransformError> {
        if !scale_factor.is_normal() || scale_factor <= 0.0 {
            return Err(SubtreeTransformError::UnrepresentableResult);
        }
        if self.curves.is_empty() {
            return Ok(PathTiles::default());
        }
        let checked = |value: f64| {
            let value = value as f32;
            if value.is_finite() {
                Ok(value)
            } else {
                Err(SubtreeTransformError::UnrepresentableResult)
            }
        };
        let scale_factor = f64::from(scale_factor);
        if transform.is_identity() {
            for component in self
                .curves
                .iter()
                .flat_map(|curve| [curve.p0, curve.p1, curve.a, curve.b])
            {
                checked(f64::from(component.x) * scale_factor)?;
                checked(f64::from(component.y) * scale_factor)?;
            }
            return Ok(self.bin(scale_factor as f32));
        }
        let scale = transform.scale();
        let [_, _, translation_x, translation_y] = transform.components();
        let project_point = |endpoint: Point<f32>| -> Result<_, SubtreeTransformError> {
            Ok(point(
                checked(
                    (f64::from(endpoint.x) * scale_factor)
                        .mul_add(f64::from(scale.width), f64::from(translation_x)),
                )?,
                checked(
                    (f64::from(endpoint.y) * scale_factor)
                        .mul_add(f64::from(scale.height), f64::from(translation_y)),
                )?,
            ))
        };
        let project_vector = |vector: Point<f32>| -> Result<_, SubtreeTransformError> {
            Ok(point(
                checked(f64::from(vector.x) * scale_factor * f64::from(scale.width))?,
                checked(f64::from(vector.y) * scale_factor * f64::from(scale.height))?,
            ))
        };
        let curves = self
            .curves
            .iter()
            .map(|curve| {
                Ok(PathCurve {
                    p0: project_point(curve.p0)?,
                    p1: project_point(curve.p1)?,
                    a: project_vector(curve.a)?,
                    b: project_vector(curve.b)?,
                    sign: curve.sign,
                    pad: 0,
                })
            })
            .collect::<Result<Vec<_>, SubtreeTransformError>>()?;
        Ok(crate::path_winding::bin(&curves, self.even_odd, 1.0))
    }
}

/// A path's contours as monotone curves, collected by [`PathBuilder`].
#[derive(Default)]
pub(crate) struct Outline {
    curves: Vec<PathCurve>,
    start: Point<f32>,
    current: Point<f32>,
    non_finite: bool,
}

impl Outline {
    /// Whether segments through these points can be added.
    fn accepts(&mut self, points: &[Point<f32>]) -> bool {
        self.non_finite |= !points
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite());
        !self.non_finite
    }

    /// Close the current contour and start a new one at the given point.
    pub(crate) fn move_to(&mut self, to: Point<f32>) {
        if !self.accepts(&[to]) {
            return;
        }
        self.close();
        self.start = to;
        self.current = to;
    }

    pub(crate) fn line_to(&mut self, to: Point<f32>) {
        if self.accepts(&[to]) && to != self.current {
            self.curves.push(PathCurve::line(self.current, to));
            self.current = to;
        }
    }

    pub(crate) fn curve_to(&mut self, to: Point<f32>, ctrl: Point<f32>) {
        if !self.accepts(&[to, ctrl]) || (to == self.current && ctrl == self.current) {
            return;
        }
        PathCurve::push_monotone(&mut self.curves, self.current, ctrl, to);
        self.current = to;
    }

    pub(crate) fn cubic_to(&mut self, to: Point<f32>, ctrl1: Point<f32>, ctrl2: Point<f32>) {
        /// Maximum error, in logical pixels.
        const TOLERANCE: f32 = 0.25;
        if !self.accepts(&[to, ctrl1, ctrl2]) {
            return;
        }
        let lyon_point = |p: Point<f32>| lyon_path::math::point(p.x, p.y);
        let cubic = CubicBezierSegment {
            from: lyon_point(self.current),
            ctrl1: lyon_point(ctrl1),
            ctrl2: lyon_point(ctrl2),
            to: lyon_point(to),
        };
        cubic.for_each_quadratic_bezier(TOLERANCE, &mut |quadratic| {
            self.curve_to(
                point(quadratic.to.x, quadratic.to.y),
                point(quadratic.ctrl.x, quadratic.ctrl.y),
            );
        });
    }

    /// Close the current contour with a straight line back to its start.
    pub(crate) fn close(&mut self) {
        let start = self.start;
        self.line_to(start);
    }

    /// The curves, with every contour closed.
    pub(crate) fn into_curves(mut self) -> Result<Vec<PathCurve>, Error> {
        if self.non_finite {
            bail!("path has a non-finite point");
        }
        self.close();
        Ok(self.curves)
    }
}

impl PathCurve {
    fn new(p0: Point<f32>, ctrl: Point<f32>, p1: Point<f32>, sign: f32) -> Self {
        Self {
            p0,
            p1,
            a: point(p0.x - 2.0 * ctrl.x + p1.x, p0.y - 2.0 * ctrl.y + p1.y),
            b: point(2.0 * (ctrl.x - p0.x), 2.0 * (ctrl.y - p0.y)),
            sign,
            pad: 0,
        }
    }

    /// A line, with exactly zero quadratic terms.
    fn line(from: Point<f32>, to: Point<f32>) -> Self {
        let (p0, p1, sign) = downward(from, to);
        Self {
            p0,
            p1,
            a: point(0.0, 0.0),
            b: point(p1.x - p0.x, p1.y - p0.y),
            sign,
            pad: 0,
        }
    }

    /// Split a quadratic segment at its extrema into xy-monotone curves.
    fn push_monotone(curves: &mut Vec<Self>, p0: Point<f32>, ctrl: Point<f32>, p1: Point<f32>) {
        let curve = QuadraticBezierSegment {
            from: lyon_path::math::point(p0.x, p0.y),
            ctrl: lyon_path::math::point(ctrl.x, ctrl.y),
            to: lyon_path::math::point(p1.x, p1.y),
        };
        // Unlike `split_range`, this also clamps each piece's control point
        // back into its endpoint box when rounding pushes it out.
        curve.for_each_monotonic(&mut |monotone| {
            let from = point(monotone.from.x, monotone.from.y);
            let to = point(monotone.to.x, monotone.to.y);
            if from == to {
                return;
            }
            // Horizontal curves are kept: upward rays cross them.
            let (p0, p1, sign) = downward(from, to);
            let ctrl = point(
                monotone.ctrl.x.clamp(p0.x.min(p1.x), p0.x.max(p1.x)),
                monotone.ctrl.y.clamp(p0.y, p1.y),
            );
            curves.push(Self::new(p0, ctrl, p1, sign));
        });
    }
}

fn downward(from: Point<f32>, to: Point<f32>) -> (Point<f32>, Point<f32>, f32) {
    if from.y > to.y {
        (to, from, -1.0)
    } else {
        (from, to, 1.0)
    }
}

/// Strokes are expanded in logical pixels. tiny-skia keeps offset curves
/// within `0.25 / STROKE_RESOLUTION_SCALE` of exact, a quarter device pixel at
/// 2x, and flattens curves to within `0.5 / STROKE_RESOLUTION_SCALE` to measure
/// dashes. Round joins and caps ignore it: their arcs are approximated to 0.25
/// logical pixels.
const STROKE_RESOLUTION_SCALE: f32 = 2.0;

/// Returns [`None`] for empty geometry.
fn skia_path(path: &lyon_path::Path) -> Option<tiny_skia_path::Path> {
    let mut builder = tiny_skia_path::PathBuilder::new();
    for event in path.iter() {
        match event {
            Event::Begin { at } => builder.move_to(at.x, at.y),
            Event::Line { to, .. } => builder.line_to(to.x, to.y),
            Event::Quadratic { ctrl, to, .. } => builder.quad_to(ctrl.x, ctrl.y, to.x, to.y),
            Event::Cubic {
                ctrl1, ctrl2, to, ..
            } => builder.cubic_to(ctrl1.x, ctrl1.y, ctrl2.x, ctrl2.y, to.x, to.y),
            Event::End { close, .. } => {
                if close {
                    builder.close();
                }
            }
        }
    }
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PrimitiveTransform, size};

    #[test]
    fn primitive_transform_matches_device_space_path() {
        let mut builder = PathBuilder::fill();
        builder.move_to(point(px(-8.0), px(2.0)));
        builder.curve_to(point(px(24.0), px(18.0)), point(px(4.0), px(2.0)));
        builder.line_to(point(px(-8.0), px(18.0)));
        builder.close();
        let path = builder.build().unwrap();

        let transform = PrimitiveTransform::try_new(
            size(2.0, 0.5),
            point(ScaledPixels(7.0), ScaledPixels(-3.0)),
        )
        .unwrap();
        let actual = path.try_bin_transformed(1.5, transform).unwrap();

        let mut expected = PathBuilder::fill();
        expected.move_to(point(px(-17.0), px(-1.5)));
        expected.curve_to(point(px(79.0), px(10.5)), point(px(19.0), px(-1.5)));
        expected.line_to(point(px(-17.0), px(10.5)));
        expected.close();
        let expected = expected.build().unwrap().bin(1.0);
        assert!(!actual.tiles().is_empty());
        assert_eq!(actual.bounds(), expected.bounds());
        assert_eq!(actual.painted_bounds(), expected.painted_bounds());
        assert_eq!(actual.tiles(), expected.tiles());
        assert_eq!(actual.curves(), expected.curves());
        assert_eq!(
            path.try_bounds(1.5).unwrap(),
            Bounds::from_corners(
                point(ScaledPixels(-12.0), ScaledPixels(3.0)),
                point(ScaledPixels(36.0), ScaledPixels(27.0)),
            )
        );

        let identity = path
            .try_bin_transformed(1.5, PrimitiveTransform::IDENTITY)
            .unwrap();
        let scaled = path.bin(1.5);
        assert_eq!(identity.bounds(), scaled.bounds());
        assert_eq!(identity.tiles(), scaled.tiles());
        assert_eq!(identity.curves(), scaled.curves());
    }

    #[test]
    fn local_bounds_include_curve_extrema() {
        let mut builder = PathBuilder::fill();
        builder.move_to(point(px(0.0), px(0.0)));
        builder.curve_to(point(px(0.0), px(100.0)), point(px(100.0), px(100.0)));
        let path = builder.build().unwrap();
        assert_eq!(
            path.try_bounds(1.5).unwrap(),
            Bounds::from_corners(
                point(ScaledPixels(0.0), ScaledPixels(0.0)),
                point(ScaledPixels(75.0), ScaledPixels(150.0)),
            )
        );
    }

    #[test]
    fn representable_projection_survives_intermediate_overflow() {
        let mut builder = PathBuilder::fill();
        builder.move_to(point(px(0.0), px(0.0)));
        builder.curve_to(point(px(1.0e38), px(10.0)), point(px(1.0e38), px(5.0)));
        builder.line_to(point(px(0.0), px(10.0)));
        builder.close();
        let path = builder.build().unwrap();
        assert!(
            path.curves
                .iter()
                .any(|curve| !(curve.b.x * 2.0).is_finite())
        );
        let transform = PrimitiveTransform::try_new(
            size(1.0e-36, 1.0),
            point(ScaledPixels(0.0), ScaledPixels(0.0)),
        )
        .unwrap();
        let tiles = path.try_bin_transformed(2.0, transform).unwrap();
        assert!(!tiles.tiles().is_empty());
        assert!((tiles.bounds().size.width.0 - 200.0).abs() < 0.001);
        assert_eq!(tiles.bounds().size.height.0, 20.0);
        assert!(tiles.curves().iter().all(|curve| {
            [curve.p0, curve.p1, curve.a, curve.b]
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        }));
        let painted = crate::PaintedPath::new_transformed(
            path,
            2.0,
            transform,
            Bounds::new(point(px(0.0), px(0.0)), size(px(128.0), px(16.0))),
            crate::black().into(),
        )
        .unwrap();
        assert!(painted.tile_count() > 0);
    }

    #[test]
    fn unrepresentable_coefficients_fail_painted_path_conversion() {
        let mut builder = PathBuilder::fill();
        builder.move_to(point(px(0.0), px(0.0)));
        builder.curve_to(point(px(1.0e38), px(10.0)), point(px(1.0e38), px(5.0)));
        builder.line_to(point(px(0.0), px(10.0)));
        builder.close();
        let path = builder.build().unwrap();
        let transform = PrimitiveTransform::try_new(
            size(2.0, 1.0),
            point(ScaledPixels(0.0), ScaledPixels(0.0)),
        )
        .unwrap();
        for (scale_factor, transform) in [(2.0, PrimitiveTransform::IDENTITY), (1.0, transform)] {
            let bounds = path.try_bounds(scale_factor).unwrap();
            assert!(transform.try_project_bounds(bounds).is_ok());
            assert!(matches!(
                path.try_bin_transformed(scale_factor, transform),
                Err(SubtreeTransformError::UnrepresentableResult)
            ));
            assert!(matches!(
                crate::PaintedPath::new_transformed(
                    path.clone(),
                    scale_factor,
                    transform,
                    Bounds::new(point(px(0.0), px(0.0)), size(px(128.0), px(16.0))),
                    crate::black().into(),
                ),
                Err(SubtreeTransformError::UnrepresentableResult)
            ));
        }
    }

    #[test]
    fn invalid_dash_arrays_fall_back_to_solid_strokes() {
        let outline = |dash: Option<&[Pixels]>| {
            let mut builder = PathBuilder::stroke(px(2.0));
            if let Some(dash) = dash {
                builder = builder.dash_array(dash);
            }
            builder.move_to(point(px(0.0), px(0.0)));
            builder.line_to(point(px(100.0), px(10.0)));
            builder.outline().unwrap().0
        };
        let solid = outline(None);
        assert!(!solid.is_empty());
        for dash in [
            vec![],
            vec![px(0.0)],
            vec![px(0.0), px(0.0)],
            vec![px(-1.0), px(1.0)],
            vec![px(f32::NAN), px(2.0)],
            vec![px(f32::INFINITY), px(2.0)],
            vec![px(f32::MAX), px(f32::MAX)],
        ] {
            assert_eq!(outline(Some(&dash)), solid, "dash {dash:?}");
        }
    }

    #[test]
    fn empty_geometry_has_no_curves_or_tiles() {
        for move_only in [false, true] {
            for mut builder in [
                PathBuilder::fill(),
                PathBuilder::stroke(px(2.0)),
                PathBuilder::stroke(px(2.0)).dash_array(&[]),
            ] {
                if move_only {
                    builder.move_to(point(px(100.0), px(200.0)));
                }
                let path = builder.build().unwrap();
                assert!(path.curves.is_empty());
                assert_eq!(path.try_bounds(2.0).unwrap(), Bounds::default());
                assert!(path.bin(2.0).tiles().is_empty());
            }
        }
    }

    #[test]
    fn non_finite_builder_transform_fails_path() {
        for mut builder in [PathBuilder::fill(), PathBuilder::stroke(px(2.0))] {
            builder.move_to(point(px(10.0), px(10.0)));
            builder.line_to(point(px(20.0), px(20.0)));
            builder.scale(f32::MAX);
            assert!(builder.build().is_err());
        }
    }
}
