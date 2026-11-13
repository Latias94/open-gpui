//! Primitive ordering, Scene sorting and batching, and full frames through GPU submission.

use std::{fmt, rc::Rc, time::Duration};

use open_gpui::{
    App, AtlasTextureId, AtlasTextureKind, AtlasTile, Background, BenchAppContext, Bounds,
    BoxShadow, Context, Corners, DevicePixels, Hsla, IntoElement, PaintedPath, Path, PathBuilder,
    Pixels, Render, Scene, SharedString, Styled, TextAlign, TextRun, TileId, UnderlineStyle,
    Window,
    bench_scene::SceneBuilder,
    canvas, fill, hsla, point,
    private::rand::{Rng as _, SeedableRng as _, rngs::StdRng},
    px, size,
};

#[derive(Clone, Copy)]
enum Pattern {
    Grid,
    Stripes,
    Overlap,
    Nested,
    Clusters,
    Overlay,
    Random { seed: u64 },
}

struct Case {
    pattern: Pattern,
    count: usize,
}

impl fmt::Display for Case {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.pattern {
            Pattern::Grid => "grid",
            Pattern::Stripes => "stripes",
            Pattern::Overlap => "overlap",
            Pattern::Nested => "nested",
            Pattern::Clusters => "clusters",
            Pattern::Overlay => "overlay",
            Pattern::Random { seed } => {
                return write!(formatter, "random-{}-seed-{seed}", self.count);
            }
        };
        write!(formatter, "{name}-{}", self.count)
    }
}

fn cases() -> Vec<Case> {
    let (seeds, _) = open_gpui::calculate_seeds(2, &[]);
    let seeds = seeds.collect::<Vec<_>>();
    [256, 1_024, 4_096]
        .into_iter()
        .flat_map(|count| {
            seeds
                .iter()
                .map(|&seed| Pattern::Random { seed })
                .chain([
                    Pattern::Grid,
                    Pattern::Stripes,
                    Pattern::Overlap,
                    Pattern::Nested,
                    Pattern::Clusters,
                    Pattern::Overlay,
                ])
                .map(move |pattern| Case { pattern, count })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[derive(Clone)]
enum Kind {
    Quad,
    Shadow,
    Underline,
    Path { path: Path, painted: PaintedPath },
    Text(SharedString),
}

#[derive(Clone)]
struct Rectangle {
    bounds: Bounds<Pixels>,
    background: Background,
    color: Hsla,
    kind: Kind,
}

fn random_rectangles(seed: u64, count: usize, extent: f32) -> Rc<[Rectangle]> {
    let mut rng = StdRng::seed_from_u64(seed);
    let (smallest, largest) = (2f32.ln(), (extent * 0.6).ln());
    (0..count)
        .map(|_| {
            let width = rng.random_range(smallest..largest).exp();
            let height = rng.random_range(smallest..largest).exp();
            let origin = point(
                px(32. + rng.random_range(0.0..extent - width)),
                px(32. + rng.random_range(0.0..extent - height)),
            );
            let bounds = Bounds::new(origin, size(px(width), px(height)));
            let color = hsla(rng.random_range(0.0..1.0), 0.8, 0.5, 1.);
            let kind = match rng.random_range(0..20) {
                0..9 => Kind::Quad,
                9..11 => Kind::Shadow,
                11..13 => Kind::Underline,
                13..16 => triangle(bounds).map_or(Kind::Quad, |path| Kind::Path {
                    painted: PaintedPath::new(path.clone(), 1., scene_viewport(), color.into())
                        .expect("fixture path must bin within the viewport"),
                    path,
                }),
                _ => {
                    let length = rng.random_range(1..12);
                    Kind::Text("Ag3 Wq7 fixture"[..length].to_string().into())
                }
            };
            Rectangle {
                bounds,
                background: color.into(),
                color,
                kind,
            }
        })
        .collect::<Vec<_>>()
        .into()
}

fn triangle(bounds: Bounds<Pixels>) -> Option<Path> {
    let mut path = PathBuilder::fill();
    path.move_to(bounds.origin);
    path.line_to(bounds.top_right());
    path.line_to(bounds.bottom_left() + point(bounds.size.width / 2., px(0.)));
    path.close();
    path.build().ok()
}

fn rectangles(case: &Case, extent: f32) -> Rc<[Rectangle]> {
    if let Pattern::Random { seed } = case.pattern {
        return random_rectangles(seed, case.count, extent);
    }
    let columns = (case.count as f32).sqrt().ceil() as usize;
    let cell = extent / columns as f32;
    let stripe_count = (extent / 4.).floor() as usize;
    (0..case.count)
        .map(|index| {
            let grid_origin = point(
                px(32. + (index % columns) as f32 * cell),
                px(32. + (index / columns) as f32 * cell),
            );
            let bounds = match case.pattern {
                Pattern::Grid => Bounds::new(grid_origin, size(px(cell * 0.4), px(cell * 0.4))),
                Pattern::Stripes => Bounds::new(
                    point(px(32.), px(32. + (index % stripe_count) as f32 * 4.)),
                    size(px(extent), px(2.)),
                ),
                Pattern::Overlap => {
                    Bounds::new(point(px(32.), px(32.)), size(px(extent), px(extent)))
                }
                Pattern::Nested => {
                    let inset = extent * 0.3 * index as f32 / case.count as f32;
                    Bounds::new(
                        point(px(32. + inset), px(32. + inset)),
                        size(px(extent - 2. * inset), px(extent - 2. * inset)),
                    )
                }
                Pattern::Clusters => {
                    let origin = 32. + (index % 2) as f32 * extent * 0.55;
                    Bounds::new(
                        point(px(origin), px(origin)),
                        size(px(extent * 0.4), px(extent * 0.4)),
                    )
                }
                Pattern::Overlay if index % 64 == 0 => {
                    Bounds::new(point(px(32.), px(32.)), size(px(extent), px(extent)))
                }
                Pattern::Overlay | Pattern::Random { .. } => {
                    Bounds::new(grid_origin, size(px(cell * 0.4), px(cell * 0.4)))
                }
            };
            let color = hsla(index as f32 / case.count as f32, 1., 0.5, 1.);
            Rectangle {
                bounds,
                background: color.into(),
                color,
                kind: Kind::Quad,
            }
        })
        .collect::<Vec<_>>()
        .into()
}

struct OrderingView {
    rectangles: Rc<[Rectangle]>,
}

impl Render for OrderingView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rectangles = self.rectangles.clone();
        canvas(
            |_, _, _| {},
            move |_, _, window, cx| {
                for rectangle in rectangles.iter() {
                    paint(rectangle, window, cx);
                }
            },
        )
        .size_full()
    }
}

fn paint(rectangle: &Rectangle, window: &mut Window, cx: &mut App) {
    match &rectangle.kind {
        Kind::Quad => window.paint_quad(fill(rectangle.bounds, rectangle.background)),
        Kind::Shadow => window.paint_drop_shadows(
            rectangle.bounds,
            Corners::all(px(4.)),
            &[BoxShadow {
                color: rectangle.color,
                offset: point(px(0.), px(2.)),
                blur_radius: px(6.),
                spread_radius: px(0.),
                inset: false,
            }],
        ),
        Kind::Underline => window.paint_underline(
            rectangle.bounds.origin,
            rectangle.bounds.size.width,
            &UnderlineStyle {
                thickness: rectangle.bounds.size.height.min(px(4.)),
                color: Some(rectangle.color),
                wavy: false,
            },
        ),
        Kind::Path { path, .. } => window.paint_path(path.clone(), rectangle.color),
        Kind::Text(text) => {
            let font_size = rectangle.bounds.size.height.clamp(px(8.), px(32.));
            let run = TextRun {
                len: text.len(),
                font: window.text_style().font(),
                color: rectangle.color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line = window
                .text_system()
                .shape_line(text.clone(), font_size, &[run], None);
            line.paint(
                rectangle.bounds.origin,
                font_size,
                TextAlign::Left,
                None,
                window,
                cx,
            )
            .expect("fixture text must paint through the production text system");
        }
    }
}

fn glyph_tile(index: usize) -> AtlasTile {
    AtlasTile {
        texture_id: AtlasTextureId {
            index: (index % 3) as u32,
            kind: AtlasTextureKind::Monochrome,
        },
        tile_id: TileId(index as u32),
        padding: 0,
        bounds: Bounds::new(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(16), DevicePixels(16)),
        ),
        texture_generation: 1,
        texture_generation_padding: 0,
    }
}

fn scene_viewport() -> Bounds<Pixels> {
    Bounds::new(point(px(0.), px(0.)), size(px(1_024.), px(1_024.)))
}

fn build_scene(builder: &mut SceneBuilder, rectangles: &[Rectangle]) {
    insert_primitives(builder, rectangles);
    builder.finish();
}

fn insert_primitives(builder: &mut SceneBuilder, rectangles: &[Rectangle]) {
    builder.clear();
    for (index, rectangle) in rectangles.iter().enumerate() {
        let bounds = rectangle.bounds;
        match &rectangle.kind {
            Kind::Quad => builder.quad(bounds, rectangle.background),
            Kind::Shadow => builder.shadow(bounds, rectangle.color, px(6.), Corners::all(px(4.))),
            Kind::Underline => {
                builder.underline(bounds, rectangle.color, bounds.size.height.min(px(4.)));
            }
            Kind::Path { painted, .. } => builder.path(painted.clone()),
            Kind::Text(text) => {
                let glyph_size = bounds.size.height.min(px(32.));
                for glyph in 0..text.len() {
                    let origin = bounds.origin + point(glyph_size * (glyph as f32 * 0.6), px(0.));
                    builder.monochrome_sprite(
                        Bounds::new(origin, size(glyph_size * 0.6, glyph_size)),
                        glyph_tile(index + glyph),
                        rectangle.color,
                    );
                }
            }
        }
    }
}

fn scene_finish(criterion: &mut criterion::Criterion<open_gpui::BenchMeasurement>) {
    let mut group = criterion.benchmark_group("Primitives/finish");
    for case in cases() {
        let mut fixture = None;
        group.bench_function(case.to_string(), |bencher| {
            let rectangles = fixture.get_or_insert_with(|| rectangles(&case, 512.));
            bencher.iter_batched_ref(
                || {
                    let mut scene = Scene::default();
                    let mut builder = SceneBuilder::new(&mut scene, scene_viewport());
                    insert_primitives(&mut builder, rectangles);
                    scene
                },
                |scene| scene.finish(),
                criterion::BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn scene_ordering(criterion: &mut criterion::Criterion<open_gpui::BenchMeasurement>) {
    let mut group = criterion.benchmark_group("Primitives/scene");
    for case in cases() {
        let mut fixture = None;
        let report = open_gpui_bench_metrics::MetricReport::new();
        group.bench_function(case.to_string(), |bencher| {
            let (rectangles, scene) =
                fixture.get_or_insert_with(|| (rectangles(&case, 512.), Scene::default()));
            let mut builder = SceneBuilder::new(scene, scene_viewport());
            report.iter(bencher, || {
                build_scene(&mut builder, rectangles);
                std::hint::black_box(builder.scene());
            });
        });
        report.print("  ");
        if std::env::var("GPUI_BENCH_SCENE_STATS").as_deref() == Ok("1")
            && let Some((_, scene)) = &fixture
        {
            eprintln!(
                "scene stats Primitives/scene/{case}: batches={}",
                scene.batches().count()
            );
        }
    }
    group.finish();
}

#[open_gpui::bench(inputs = cases(), input_name = "case", group = "Primitives/paint", fps = 120)]
fn paint_rectangles(case: &Case, cx: &mut BenchAppContext) {
    assert!(
        cfg!(any(target_os = "macos", target_os = "linux")),
        "primitive painting requires the platform's headless GPU renderer"
    );
    assert!(
        !cfg!(all(target_os = "macos", not(feature = "font-kit")))
            && !cfg!(all(
                target_os = "linux",
                not(any(feature = "x11", feature = "wayland"))
            )),
        "primitive painting requires production text shaping: font-kit on macOS, x11/wayland on Linux"
    );
    let mut window = cx.add_empty_window();
    let extent = window.update(|window, _| {
        let viewport = window.viewport_size();
        (viewport.width.as_f32().min(viewport.height.as_f32()) - 64.).min(512.)
    });
    let rectangles = rectangles(case, extent);
    let root =
        window.update(|window, cx| window.replace_root(cx, |_, _| OrderingView { rectangles }));
    cx.run_until_idle();
    window.update(|window, _| window.refresh());
    cx.run_until_idle();
    cx.bench_renderer(root, |_, _, cx| cx.notify());
    window.update(|window, _| {
        if std::env::var("GPUI_BENCH_SCENE_STATS").as_deref() == Ok("1") {
            eprintln!(
                "scene stats Primitives/paint/{case}: batches={}",
                window.rendered_scene().batches().count(),
            );
        }
    });
    window.update(|window, cx| window.remove_window(cx));
    cx.run_until_idle();
}

open_gpui::bench_group!(
    name = benches;
    config = criterion::Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1))
        .sample_size(20);
    targets = paint_rectangles, scene_ordering, scene_finish
);
open_gpui::bench_main!(benches);
