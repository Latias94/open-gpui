use crate::{
    AnyElement, AnyImageCache, App, Asset, AssetLogger, Bounds, DefiniteLength, Element, ElementId,
    Entity, GlobalElementId, Hitbox, Image, ImageCache, InspectorElementId, InteractiveElement,
    Interactivity, LayoutId, Length, ObjectFit, Pixels, PreparedSubtreeClip, RenderImage, Resource,
    SharedString, SharedUri, StyleRefinement, Styled, Task, Window, decode_gif_image,
    decode_static_image, decode_webp_image, px,
};
use anyhow::Result;

use futures::Future;
use image::{ImageError, ImageFormat};
use open_gpui_core_util::ResultExt;
use open_gpui_scheduler::Instant;
#[cfg(test)]
use smallvec::SmallVec;
use std::{
    fs, io,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use thiserror::Error;

use super::{Stateful, StatefulInteractiveElement};

/// The delay before showing the loading state.
pub const LOADING_DELAY: Duration = Duration::from_millis(200);

/// A type alias to the resource loader that the `img()` element uses.
///
/// Note: that this is only for Resources, like URLs or file paths.
/// Custom loaders, or external images will not use this asset loader
pub type ImgResourceLoader = AssetLogger<ImageAssetLoader>;

/// A source of image content.
#[derive(Clone)]
pub enum ImageSource {
    /// The image content will be loaded from some resource location
    Resource(Resource),
    /// Cached image data
    Render(Arc<RenderImage>),
    /// Cached image data
    Image(Arc<Image>),
    /// A custom loading function to use
    Custom(Arc<dyn Fn(&mut Window, &mut App) -> Option<Result<Arc<RenderImage>, ImageCacheError>>>),
}

fn is_uri(uri: &str) -> bool {
    url::Url::from_str(uri).is_ok()
}

impl From<SharedUri> for ImageSource {
    fn from(value: SharedUri) -> Self {
        Self::Resource(Resource::Uri(value))
    }
}

impl<'a> From<&'a str> for ImageSource {
    fn from(s: &'a str) -> Self {
        if is_uri(s) {
            Self::Resource(Resource::Uri(s.to_string().into()))
        } else {
            Self::Resource(Resource::Embedded(s.to_string().into()))
        }
    }
}

impl From<String> for ImageSource {
    fn from(s: String) -> Self {
        if is_uri(&s) {
            Self::Resource(Resource::Uri(s.into()))
        } else {
            Self::Resource(Resource::Embedded(s.into()))
        }
    }
}

impl From<SharedString> for ImageSource {
    fn from(s: SharedString) -> Self {
        s.as_ref().into()
    }
}

impl From<&Path> for ImageSource {
    fn from(value: &Path) -> Self {
        Self::Resource(value.to_path_buf().into())
    }
}

impl From<Arc<Path>> for ImageSource {
    fn from(value: Arc<Path>) -> Self {
        Self::Resource(value.into())
    }
}

impl From<PathBuf> for ImageSource {
    fn from(value: PathBuf) -> Self {
        Self::Resource(value.into())
    }
}

impl From<Arc<RenderImage>> for ImageSource {
    fn from(value: Arc<RenderImage>) -> Self {
        Self::Render(value)
    }
}

impl From<Arc<Image>> for ImageSource {
    fn from(value: Arc<Image>) -> Self {
        Self::Image(value)
    }
}

impl<F> From<F> for ImageSource
where
    F: Fn(&mut Window, &mut App) -> Option<Result<Arc<RenderImage>, ImageCacheError>> + 'static,
{
    fn from(value: F) -> Self {
        Self::Custom(Arc::new(value))
    }
}

/// The style of an image element.
pub struct ImageStyle {
    grayscale: bool,
    object_fit: ObjectFit,
    loading: Option<Box<dyn Fn() -> AnyElement>>,
    fallback: Option<Box<dyn Fn() -> AnyElement>>,
}

impl Default for ImageStyle {
    fn default() -> Self {
        Self {
            grayscale: false,
            object_fit: ObjectFit::Contain,
            loading: None,
            fallback: None,
        }
    }
}

/// Style an image element.
pub trait StyledImage: Sized {
    /// Get a mutable [ImageStyle] from the element.
    fn image_style(&mut self) -> &mut ImageStyle;

    /// Set the image to be displayed in grayscale.
    fn grayscale(mut self, grayscale: bool) -> Self {
        self.image_style().grayscale = grayscale;
        self
    }

    /// Set the object fit for the image.
    fn object_fit(mut self, object_fit: ObjectFit) -> Self {
        self.image_style().object_fit = object_fit;
        self
    }

    /// Set a fallback function that will be invoked to render an error view should
    /// the image fail to load.
    fn with_fallback(mut self, fallback: impl Fn() -> AnyElement + 'static) -> Self {
        self.image_style().fallback = Some(Box::new(fallback));
        self
    }

    /// Set a fallback function that will be invoked to render a view while the image
    /// is still being loaded.
    fn with_loading(mut self, loading: impl Fn() -> AnyElement + 'static) -> Self {
        self.image_style().loading = Some(Box::new(loading));
        self
    }
}

impl StyledImage for Img {
    fn image_style(&mut self) -> &mut ImageStyle {
        &mut self.style
    }
}

impl StyledImage for Stateful<Img> {
    fn image_style(&mut self) -> &mut ImageStyle {
        &mut self.element.style
    }
}

/// An image element.
pub struct Img {
    interactivity: Interactivity,
    source: ImageSource,
    style: ImageStyle,
    image_cache: Option<AnyImageCache>,
}

/// Create a new image element.
#[track_caller]
pub fn img(source: impl Into<ImageSource>) -> Img {
    Img {
        interactivity: Interactivity::new(),
        source: source.into(),
        style: ImageStyle::default(),
        image_cache: None,
    }
}

impl Img {
    /// A list of all format extensions currently supported by this img element
    pub fn extensions() -> &'static [&'static str] {
        // This is the list in [image::ImageFormat::from_extension] + `svg`
        &[
            "avif", "jpg", "jpeg", "png", "gif", "webp", "tif", "tiff", "tga", "dds", "bmp", "ico",
            "hdr", "exr", "pbm", "pam", "ppm", "pgm", "ff", "farbfeld", "qoi", "svg",
        ]
    }

    /// Sets the image cache for the current node.
    ///
    /// If the `image_cache` is not explicitly provided, the function will determine the image cache by:
    ///
    /// 1. Checking if any ancestor node of the current node contains an `ImageCacheElement`, If such a node exists, the image cache specified by that ancestor will be used.
    /// 2. If no ancestor node contains an `ImageCacheElement`, the global image cache will be used as a fallback.
    ///
    /// This mechanism provides a flexible way to manage image caching, allowing precise control when needed,
    /// while ensuring a default behavior when no cache is explicitly specified.
    #[inline]
    pub fn image_cache<I: ImageCache>(self, image_cache: &Entity<I>) -> Self {
        Self {
            image_cache: Some(image_cache.clone().into()),
            ..self
        }
    }
}

impl Deref for Stateful<Img> {
    type Target = Img;

    fn deref(&self) -> &Self::Target {
        &self.element
    }
}

impl DerefMut for Stateful<Img> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.element
    }
}

/// The image state between frames
struct ImgState {
    frame_index: usize,
    last_frame_time: Option<Instant>,
    started_loading: Option<(Instant, Task<()>)>,
}

/// The image layout state between frames
pub struct ImgLayoutState {
    frame_index: usize,
    replacement: Option<AnyElement>,
}

impl Element for Img {
    type RequestLayoutState = ImgLayoutState;
    type PrepaintState = (Option<Hitbox>, Option<PreparedSubtreeClip>);

    fn id(&self) -> Option<ElementId> {
        self.interactivity.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        self.interactivity.source_location()
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut layout_state = ImgLayoutState {
            frame_index: 0,
            replacement: None,
        };

        window.with_optional_element_state(global_id, |state, window| {
            let mut state = state.map(|state| {
                state.unwrap_or(ImgState {
                    frame_index: 0,
                    last_frame_time: None,
                    started_loading: None,
                })
            });

            let mut frame_index = state.as_ref().map(|state| state.frame_index).unwrap_or(0);

            let layout_id = self.interactivity.request_layout(
                global_id,
                inspector_id,
                window,
                cx,
                |mut style, window, cx| {
                    let mut replacement_id = None;

                    match self.source.use_data(
                        self.image_cache
                            .clone()
                            .or_else(|| window.image_cache_stack.last().cloned()),
                        window,
                        cx,
                    ) {
                        Some(Ok(data)) => {
                            let frame_count = data.frame_count();
                            let max_frame_index = frame_count.saturating_sub(1);

                            if let Some(state) = &mut state {
                                state.frame_index = state.frame_index.min(max_frame_index);
                                if frame_count > 1 && !cx.reduce_motion() {
                                    if window.is_window_active() {
                                        let current_time = cx.background_executor().now();
                                        if let Some(last_frame_time) = state.last_frame_time {
                                            let elapsed = current_time - last_frame_time;
                                            let frame_duration =
                                                Duration::from(data.delay(state.frame_index));

                                            if elapsed >= frame_duration {
                                                state.frame_index =
                                                    (state.frame_index + 1) % frame_count;
                                                state.last_frame_time =
                                                    Some(current_time - (elapsed - frame_duration));
                                            }
                                        } else {
                                            state.last_frame_time = Some(current_time);
                                        }
                                    } else {
                                        state.last_frame_time = None;
                                    }
                                } else {
                                    state.last_frame_time = None;
                                }
                                state.started_loading = None;
                                frame_index = state.frame_index;
                            }

                            let image_size = data.render_size(frame_index);
                            if style.aspect_ratio.is_none() {
                                style.aspect_ratio = Some(image_size.width / image_size.height);
                            }

                            if let Length::Auto = style.size.width {
                                style.size.width = match style.size.height {
                                    Length::Definite(DefiniteLength::Absolute(abs_length)) => {
                                        let height_px = abs_length.to_pixels(window.rem_size());
                                        Length::Definite(
                                            px(image_size.width.0 * height_px.0
                                                / image_size.height.0)
                                            .into(),
                                        )
                                    }
                                    _ => Length::Definite(image_size.width.into()),
                                };
                            }

                            if let Length::Auto = style.size.height {
                                style.size.height = match style.size.width {
                                    Length::Definite(DefiniteLength::Absolute(abs_length)) => {
                                        let width_px = abs_length.to_pixels(window.rem_size());
                                        Length::Definite(
                                            px(image_size.height.0 * width_px.0
                                                / image_size.width.0)
                                            .into(),
                                        )
                                    }
                                    _ => Length::Definite(image_size.height.into()),
                                };
                            }

                            if global_id.is_some()
                                && data.frame_count() > 1
                                && window.is_window_active()
                                && !cx.reduce_motion()
                            {
                                window.request_animation_frame();
                            }
                        }
                        Some(_err) => {
                            if let Some(fallback) = self.style.fallback.as_ref() {
                                let mut element = fallback();
                                replacement_id = Some(element.request_layout(window, cx));
                                layout_state.replacement = Some(element);
                            }
                            if let Some(state) = &mut state {
                                state.started_loading = None;
                            }
                        }
                        None => {
                            if let Some(state) = &mut state {
                                if let Some((started_loading, _)) = state.started_loading {
                                    if cx.background_executor().now() - started_loading
                                        >= LOADING_DELAY
                                        && let Some(loading) = self.style.loading.as_ref()
                                    {
                                        let mut element = loading();
                                        replacement_id = Some(element.request_layout(window, cx));
                                        layout_state.replacement = Some(element);
                                    }
                                } else {
                                    let current_view = window.current_view();
                                    let task = window.spawn(cx, async move |cx| {
                                        cx.background_executor().timer(LOADING_DELAY).await;
                                        cx.update(move |_, cx| {
                                            cx.notify(current_view);
                                        })
                                        .ok();
                                    });
                                    state.started_loading =
                                        Some((cx.background_executor().now(), task));
                                }
                            }
                        }
                    }

                    window.request_layout(style, replacement_id, cx)
                },
            );

            layout_state.frame_index = frame_index;

            ((layout_id, layout_state), state)
        })
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            bounds.size,
            window,
            cx,
            |_, _, hitbox, window, cx| {
                if let Some(replacement) = &mut request_layout.replacement {
                    replacement.prepaint(window, cx);
                }

                hitbox
            },
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_state: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let source = self.source.clone();
        self.interactivity.paint(
            global_id,
            inspector_id,
            bounds,
            prepaint.0.as_ref(),
            prepaint.1.as_ref(),
            window,
            cx,
            |style, window, cx| {
                if let Some(Ok(data)) = source.use_data(
                    self.image_cache
                        .clone()
                        .or_else(|| window.image_cache_stack.last().cloned()),
                    window,
                    cx,
                ) {
                    if data.frame_count() == 0 {
                        return;
                    }
                    let new_bounds = self
                        .style
                        .object_fit
                        .get_bounds(bounds, data.size(layout_state.frame_index));
                    let corner_radii = style.corner_radii.to_pixels(window.rem_size());
                    window
                        .paint_image(
                            bounds,
                            new_bounds,
                            corner_radii,
                            data,
                            layout_state.frame_index,
                            self.style.grayscale,
                        )
                        .log_err();
                } else if let Some(replacement) = &mut layout_state.replacement {
                    replacement.paint(window, cx);
                }
            },
        )
    }
}

impl Styled for Img {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.interactivity.base_style
    }
}

impl InteractiveElement for Img {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

impl StatefulInteractiveElement for Img {}

impl ImageSource {
    pub(crate) fn use_data(
        &self,
        cache: Option<AnyImageCache>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        match self {
            ImageSource::Resource(resource) => {
                if let Some(cache) = cache {
                    cache.load(resource, window, cx)
                } else {
                    window.use_asset::<ImgResourceLoader>(resource, cx)
                }
            }
            ImageSource::Custom(loading_fn) => loading_fn(window, cx),
            ImageSource::Render(data) => Some(Ok(data.to_owned())),
            ImageSource::Image(data) => window.use_asset::<AssetLogger<ImageDecoder>>(data, cx),
        }
    }

    pub(crate) fn get_data(
        &self,
        cache: Option<AnyImageCache>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        match self {
            ImageSource::Resource(resource) => {
                if let Some(cache) = cache {
                    cache.load(resource, window, cx)
                } else {
                    window.get_asset::<ImgResourceLoader>(resource, cx)
                }
            }
            ImageSource::Custom(loading_fn) => loading_fn(window, cx),
            ImageSource::Render(data) => Some(Ok(data.to_owned())),
            ImageSource::Image(data) => window.get_asset::<AssetLogger<ImageDecoder>>(data, cx),
        }
    }

    /// Remove this image source from the asset system
    pub fn remove_asset(&self, cx: &mut App) {
        match self {
            ImageSource::Resource(resource) => {
                cx.remove_asset::<ImgResourceLoader>(resource);
            }
            ImageSource::Custom(_) | ImageSource::Render(_) => {}
            ImageSource::Image(data) => cx.remove_asset::<AssetLogger<ImageDecoder>>(data),
        }
    }

    /// Returns whether this source is loading or loaded in the asset cache without fetching it.
    #[cfg(any(test, feature = "test-support"))]
    pub fn is_asset_cached(&self, cx: &App) -> bool {
        match self {
            ImageSource::Resource(resource) => cx.has_asset::<ImgResourceLoader>(resource),
            ImageSource::Custom(_) | ImageSource::Render(_) => false,
            ImageSource::Image(data) => cx.has_asset::<AssetLogger<ImageDecoder>>(data),
        }
    }
}

#[derive(Clone)]
enum ImageDecoder {}

impl Asset for ImageDecoder {
    type Source = Arc<Image>;
    type Output = Result<Arc<RenderImage>, ImageCacheError>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let renderer = cx.svg_renderer();
        async move { source.to_image_data(renderer).map_err(Into::into) }
    }
}

/// An image loader for the GPUI asset system
#[derive(Clone)]
pub enum ImageAssetLoader {}

impl Asset for ImageAssetLoader {
    type Source = Resource;
    type Output = Result<Arc<RenderImage>, ImageCacheError>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let client = cx.http_client();
        // TODO: Can we make SVGs always rescale?
        // let scale_factor = cx.scale_factor();
        let svg_renderer = cx.svg_renderer();
        let asset_source = cx.asset_source().clone();
        async move {
            let bytes = match source.clone() {
                Resource::Path(uri) => fs::read(uri.as_ref())?,
                Resource::Uri(uri) => {
                    use anyhow::Context as _;
                    use futures::AsyncReadExt as _;

                    let mut response = client
                        .get(uri.as_ref(), ().into(), true)
                        .await
                        .with_context(|| format!("loading image asset from {uri:?}"))?;
                    let mut body = Vec::new();
                    response.body_mut().read_to_end(&mut body).await?;
                    if !response.status().is_success() {
                        let mut body = String::from_utf8_lossy(&body).into_owned();
                        let first_line = body.lines().next().unwrap_or("").trim_end();
                        body.truncate(first_line.len());
                        return Err(ImageCacheError::BadStatus {
                            uri,
                            status: response.status(),
                            body,
                        });
                    }
                    body
                }
                Resource::Embedded(path) => {
                    let data = asset_source.load(&path).ok().flatten();
                    if let Some(data) = data {
                        data.to_vec()
                    } else {
                        return Err(ImageCacheError::Asset(
                            format!("Embedded resource not found: {}", path).into(),
                        ));
                    }
                }
            };

            if let Ok(format) = image::guess_format(&bytes) {
                let data = match format {
                    ImageFormat::Gif => decode_gif_image(&bytes)?,
                    ImageFormat::WebP => decode_webp_image(&bytes)?,
                    _ => decode_static_image(&bytes, format)?,
                };

                Ok(Arc::new(RenderImage::new(data)))
            } else {
                svg_renderer
                    .render_single_frame(&bytes, 1.0)
                    .map_err(Into::into)
            }
        }
    }
}

/// An error that can occur when interacting with the image cache.
#[derive(Debug, Error, Clone)]
pub enum ImageCacheError {
    /// Some other kind of error occurred
    #[error("error: {0}")]
    Other(#[from] Arc<anyhow::Error>),
    /// An error that occurred while reading the image from disk.
    #[error("IO error: {0}")]
    Io(Arc<std::io::Error>),
    /// An error that occurred while processing an image.
    #[error("unexpected http status for {uri}: {status}, body: {body}")]
    BadStatus {
        /// The URI of the image.
        uri: SharedUri,
        /// The HTTP status code.
        status: open_gpui_http_client::StatusCode,
        /// The HTTP response body.
        body: String,
    },
    /// An error that occurred while processing an asset.
    #[error("asset error: {0}")]
    Asset(SharedString),
    /// An error that occurred while processing an image.
    #[error("image error: {0}")]
    Image(Arc<ImageError>),
    /// An error that occurred while processing an SVG.
    #[error("svg error: {0}")]
    Usvg(Arc<usvg::Error>),
}

impl From<anyhow::Error> for ImageCacheError {
    fn from(value: anyhow::Error) -> Self {
        Self::Other(Arc::new(value))
    }
}

impl From<io::Error> for ImageCacheError {
    fn from(value: io::Error) -> Self {
        Self::Io(Arc::new(value))
    }
}

impl From<usvg::Error> for ImageCacheError {
    fn from(value: usvg::Error) -> Self {
        Self::Usvg(Arc::new(value))
    }
}

impl From<image::ImageError> for ImageCacheError {
    fn from(value: image::ImageError) -> Self {
        Self::Image(Arc::new(value))
    }
}

#[cfg(test)]
mod tests {
    mod reduced_motion;

    use super::*;
    use crate::{
        AppContext as _, AtlasAccessOutcome, AtlasRemoveOutcome, Context, Empty, IntoElement,
        ParentElement as _, Render, TestAppContext, Window, canvas, div, point, px, size,
    };
    use image::{Frame, ImageBuffer, Rgba};

    const TEST_IMG_ID: &str = "test-img";

    fn test_image(frame_count: usize) -> Arc<RenderImage> {
        let frame = Frame::new(ImageBuffer::from_pixel(1, 1, Rgba([0, 0, 0, 0])));
        Arc::new(RenderImage::new(SmallVec::from_iter(
            (0..frame_count).map(|_| frame.clone()),
        )))
    }

    fn test_image_with_size(width: u32, height: u32) -> Arc<RenderImage> {
        let frame = Frame::new(ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 0])));
        Arc::new(RenderImage::new(SmallVec::from_elem(frame, 1)))
    }

    /// Overwrites the cached `frame_index` of the sibling `img` during paint.
    fn seed_frame_index(frame_index: usize) -> impl IntoElement {
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                window.with_global_id(TEST_IMG_ID.into(), |id, window| {
                    window.with_element_state::<ImgState, _>(id, |state, _| {
                        let mut state = state.expect("img state should be initialized");
                        state.frame_index = frame_index;
                        ((), state)
                    });
                });
            },
        )
    }

    struct ImageFrameView {
        image: Arc<RenderImage>,
    }

    impl Render for ImageFrameView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            img(ImageSource::Render(self.image.clone()))
        }
    }

    #[open_gpui::test]
    fn zero_frame_image_does_not_panic_on_paint(cx: &mut TestAppContext) {
        cx.add_empty_window()
            .draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
                img(ImageSource::Render(test_image(0))).into_any_element()
            });
    }

    #[open_gpui::test]
    fn image_object_fit_cover_crops_to_element_bounds(cx: &mut TestAppContext) {
        let window = cx.add_empty_window();
        let image = test_image_with_size(200, 100);
        window.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
            img(ImageSource::Render(image.clone()))
                .size_full()
                .object_fit(ObjectFit::Fill)
                .into_any_element()
        });
        let full_tile = window.update(|window, _| {
            window
                .rendered_frame
                .scene
                .polychrome_sprites
                .last()
                .expect("fill image should paint a sprite")
                .tile
        });

        window.draw(point(px(10.), px(20.)), size(px(100.), px(100.)), |_, _| {
            img(ImageSource::Render(image))
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element()
        });

        let (rendered_bounds, rendered_tile, scale_factor) = window.update(|window, _| {
            let sprite = window
                .rendered_frame
                .scene
                .polychrome_sprites
                .last()
                .expect("cover image should paint a sprite");
            assert_eq!(
                window.rendered_frame_image_paint_diagnostics()[0].tile,
                sprite.tile
            );
            (sprite.bounds, sprite.tile, window.scale_factor())
        });
        assert_eq!(
            rendered_bounds,
            Bounds {
                origin: point(px(10.).scale(scale_factor), px(20.).scale(scale_factor)),
                size: size(px(100.).scale(scale_factor), px(100.).scale(scale_factor)),
            }
        );
        assert_eq!(
            (
                rendered_tile.bounds.origin.x.0 - full_tile.bounds.origin.x.0,
                rendered_tile.bounds.origin.y.0 - full_tile.bounds.origin.y.0,
                rendered_tile.bounds.size.width.0,
                rendered_tile.bounds.size.height.0,
            ),
            (50, 0, 100, 100),
        );
        assert_eq!(
            rendered_tile.texture_instance(),
            full_tile.texture_instance()
        );
        assert_eq!(rendered_tile.tile_id, full_tile.tile_id);
        assert_eq!(rendered_tile.padding, full_tile.padding);
    }

    #[open_gpui::test]
    fn image_object_fit_cover_clamps_corner_radii_to_visible_bounds(cx: &mut TestAppContext) {
        let window = cx.add_empty_window();
        window.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
            img(ImageSource::Render(test_image_with_size(200, 100)))
                .size_full()
                .rounded(px(100.))
                .object_fit(ObjectFit::Cover)
                .into_any_element()
        });

        let (corner_radius, expected_corner_radius) = window.update(|window, _| {
            (
                window
                    .rendered_frame
                    .scene
                    .polychrome_sprites
                    .last()
                    .map(|sprite| sprite.corner_radii.top_left),
                px(50.).scale(window.scale_factor()),
            )
        });
        assert_eq!(corner_radius, Some(expected_corner_radius));
    }

    #[open_gpui::test]
    fn explicit_aspect_ratio_is_not_overridden_by_intrinsic_ratio(cx: &mut TestAppContext) {
        let window = cx.add_empty_window();

        // A portrait image in a square container
        window.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
            div()
                .size(px(100.))
                .overflow_hidden()
                .child(
                    img(ImageSource::Render(test_image_with_size(100, 200)))
                        .size_full()
                        .aspect_square()
                        .object_fit(ObjectFit::Contain),
                )
                .into_any_element()
        });

        let (rendered_bounds, scale_factor) = window.update(|window, _| {
            let sprite = window
                .rendered_frame
                .scene
                .polychrome_sprites
                .last()
                .expect("contained image should paint a sprite");
            (sprite.bounds, window.scale_factor())
        });

        // The element stays 100x100, so the image is letterboxed horizontally
        assert_eq!(
            rendered_bounds,
            Bounds {
                origin: point(px(25.).scale(scale_factor), px(0.).scale(scale_factor)),
                size: size(px(50.).scale(scale_factor), px(100.).scale(scale_factor)),
            }
        );
    }

    #[open_gpui::test]
    #[should_panic(expected = "StatefulInteractiveElement::on_drag requires a stable element ID")]
    fn idless_image_drag_source_fails_loudly(cx: &mut TestAppContext) {
        cx.add_empty_window()
            .draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
                img(ImageSource::Render(test_image(1)))
                    .on_drag((), |_, _, _, cx| cx.new(|_| Empty))
                    .into_any_element()
            });
    }

    #[open_gpui::test]
    fn stale_frame_index_is_clamped_when_image_changes(cx: &mut TestAppContext) {
        let window = cx.add_empty_window();

        // Assert that a cached frame_index from a previous multi-frame image
        // does not cause an out-of-bounds panic when the image is replaced
        // with one that has fewer frames.
        window.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
            div()
                .child(img(ImageSource::Render(test_image(5))).id(TEST_IMG_ID))
                .child(seed_frame_index(4))
                .into_any_element()
        });
        window.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
            img(ImageSource::Render(test_image(1)))
                .id(TEST_IMG_ID)
                .into_any_element()
        });
    }

    #[open_gpui::test]
    fn capture_frame_returns_metadata_when_pixels_are_unsupported(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();

        let first = cx.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.capture_frame()
        });
        let second = cx.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.capture_frame()
        });

        assert!(first.image.is_none());
        assert!(
            first
                .unsupported_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("render_to_image not available"))
        );
        assert_eq!(first.metadata.capture_generation, 1);
        assert_eq!(second.metadata.capture_generation, 2);
        assert!(
            second.metadata.framework_frame_generation > first.metadata.framework_frame_generation
        );
        assert_eq!(first.metadata.window_id, second.metadata.window_id);
        assert!(first.metadata.logical_viewport_size.width > px(0.));
        assert!(first.metadata.physical_viewport_size.width.0 > 0);
    }

    #[open_gpui::test]
    fn render_image_paint_records_frame_and_atlas_diagnostics(cx: &mut TestAppContext) {
        let image = test_image(1);
        let image_id = image.id;
        let cx = cx.add_empty_window();

        cx.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
            img(ImageSource::Render(image.clone())).into_any_element()
        });
        let (first_paints, first_accesses) = cx.update(|window, _| {
            (
                window.rendered_frame_image_paint_diagnostics().to_vec(),
                window.rendered_frame_atlas_access_diagnostics().to_vec(),
            )
        });

        assert_eq!(first_paints.len(), 1);
        assert_eq!(first_accesses.len(), 1);
        assert_eq!(first_paints[0].image.image_id, image_id);
        assert_eq!(
            first_paints[0].atlas_access.outcome,
            AtlasAccessOutcome::Inserted
        );
        assert_eq!(first_accesses[0].outcome, AtlasAccessOutcome::Inserted);

        cx.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
            img(ImageSource::Render(image.clone())).into_any_element()
        });
        let (second_paints, second_accesses) = cx.update(|window, _| {
            (
                window.rendered_frame_image_paint_diagnostics().to_vec(),
                window.rendered_frame_atlas_access_diagnostics().to_vec(),
            )
        });

        assert_eq!(second_paints.len(), 1);
        assert_eq!(second_accesses.len(), 1);
        assert_eq!(second_paints[0].image.image_id, image_id);
        assert_eq!(
            second_paints[0].atlas_access.outcome,
            AtlasAccessOutcome::Hit
        );
        assert_eq!(second_accesses[0].outcome, AtlasAccessOutcome::Hit);

        let remove = cx.update(|window, _| {
            window.drop_image(image.clone()).unwrap();
            *window
                .atlas_remove_diagnostics()
                .last()
                .expect("drop_image should record removal diagnostics")
        });
        assert_eq!(
            remove.image.expect("image-backed remove").image_id,
            image_id
        );
        assert_eq!(remove.outcome, AtlasRemoveOutcome::TextureRetained);
    }

    #[open_gpui::test]
    fn window_draw_advances_frame_generation_for_image_paint_diagnostics(cx: &mut TestAppContext) {
        let image = test_image(1);
        let (_view, cx) = cx.add_window_view({
            let image = image.clone();
            move |_, _| ImageFrameView { image }
        });

        let first_frame_generation = cx.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.rendered_frame_image_paint_diagnostics()[0].frame_generation
        });
        let second_frame_generation = cx.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.rendered_frame_image_paint_diagnostics()[0].frame_generation
        });

        assert!(second_frame_generation > first_frame_generation);
    }

    fn assert_same_frames(actual: &RenderImage, expected: &RenderImage) {
        assert_eq!(actual.frame_count(), expected.frame_count());
        for frame in 0..expected.frame_count() {
            assert_eq!(actual.size(frame), expected.size(frame));
            assert_eq!(actual.delay(frame), expected.delay(frame));
            assert_eq!(actual.as_bytes(frame), expected.as_bytes(frame));
        }
    }

    #[open_gpui::test]
    async fn path_and_http_loaders_share_the_public_image_decoder(cx: &mut TestAppContext) {
        use crate::{ImageFormat as EncodedFormat, SvgRenderer};
        use open_gpui_http_client::{FakeHttpClient, Response};

        for (name, format, bytes) in [
            (
                "red-blue-100ms.webp",
                EncodedFormat::Webp,
                include_bytes!("../../../../tests/fixtures/images/red-blue-100ms.webp").as_slice(),
            ),
            (
                "exif-orientation-rotate-180.jpg",
                EncodedFormat::Jpeg,
                include_bytes!("../../../../tests/fixtures/images/exif-orientation-rotate-180.jpg")
                    .as_slice(),
            ),
        ] {
            let expected = Image::from_bytes(format, bytes.to_vec())
                .to_image_data(SvgRenderer::new(Arc::new(())))
                .unwrap();
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/images")
                .join(name);
            let path_loaded = cx
                .update(|cx| {
                    let load: futures::future::BoxFuture<'static, _> =
                        Box::pin(ImageAssetLoader::load(Resource::Path(path.into()), cx));
                    load
                })
                .await
                .unwrap();
            assert_same_frames(&path_loaded, &expected);

            let client = FakeHttpClient::create(move |_| async move {
                Ok(Response::builder().status(200).body(bytes.into()).unwrap())
            });
            cx.update(|cx| cx.set_http_client(client));
            let resource = Resource::Uri(format!("https://test.example/{name}").into());
            let http_loaded = cx
                .update(|cx| {
                    let load: futures::future::BoxFuture<'static, _> =
                        Box::pin(ImageAssetLoader::load(resource, cx));
                    load
                })
                .await
                .unwrap();
            assert_same_frames(&http_loaded, &expected);
        }
    }

    #[open_gpui::test]
    async fn retained_resource_formats_decode_with_default_minus_avif_policy(
        cx: &mut TestAppContext,
    ) {
        use image::{DynamicImage, Rgb};
        use open_gpui_http_client::{FakeHttpClient, Response};

        let source = DynamicImage::ImageRgb8(ImageBuffer::from_fn(4, 4, |column, row| {
            if (column + row) % 2 == 0 {
                Rgb([255, 0, 0])
            } else {
                Rgb([0, 0, 255])
            }
        }));
        let expected = source.to_rgba8();
        let mut dds = vec![0; 136];
        dds[..4].copy_from_slice(b"DDS ");
        for (offset, value) in [
            (4, 124_u32),
            (8, 0x0008_1007),
            (12, 4),
            (16, 4),
            (20, 8),
            (76, 32),
            (80, 4),
            (108, 0x1000),
        ] {
            dds[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        dds[84..88].copy_from_slice(b"DXT1");
        dds[128..].copy_from_slice(&[0x00, 0xf8, 0x1f, 0x00, 0x44, 0x11, 0x44, 0x11]);

        for format in [
            ImageFormat::Dds,
            ImageFormat::OpenExr,
            ImageFormat::Farbfeld,
            ImageFormat::Hdr,
            ImageFormat::Qoi,
        ] {
            let bytes = if format == ImageFormat::Dds {
                dds.clone()
            } else {
                let source = match format {
                    ImageFormat::OpenExr | ImageFormat::Hdr => {
                        DynamicImage::ImageRgb32F(source.to_rgb32f())
                    }
                    ImageFormat::Farbfeld => DynamicImage::ImageRgba16(source.to_rgba16()),
                    _ => source.clone(),
                };
                let mut encoded = io::Cursor::new(Vec::new());
                source.write_to(&mut encoded, format).unwrap();
                encoded.into_inner()
            };
            assert_eq!(image::guess_format(&bytes).unwrap(), format);
            cx.update(|cx| {
                cx.set_http_client(FakeHttpClient::create(move |_| {
                    let bytes = bytes.clone();
                    async move { Ok(Response::builder().status(200).body(bytes.into()).unwrap()) }
                }));
            });
            let resource = Resource::Uri(format!("https://test.example/{format:?}").into());
            let image = cx
                .update(|cx| {
                    let load: futures::future::BoxFuture<'static, _> =
                        Box::pin(ImageAssetLoader::load(resource, cx));
                    load
                })
                .await
                .unwrap();

            assert_eq!(image.frame_count(), 1, "{format:?}");
            assert_eq!(image.size(0), size(4.into(), 4.into()), "{format:?}");
            let actual = image.as_bytes(0).unwrap();
            assert_eq!(actual.len(), 4 * 4 * 4, "{format:?}");
            for (pixel, source) in actual.chunks_exact(4).zip(expected.pixels()) {
                assert_eq!(
                    pixel,
                    &[source[2], source[1], source[0], source[3]],
                    "{format:?}"
                );
            }
        }
    }

    #[open_gpui::test]
    async fn header_only_avif_resource_returns_unsupported(cx: &mut TestAppContext) {
        use image::error::{ImageFormatHint, UnsupportedErrorKind};
        use open_gpui_http_client::{FakeHttpClient, Response};

        let header_only_avif: &[u8] = b"\0\0\0\x18ftypavif\0\0\0\0avifmif1";
        assert_eq!(
            image::guess_format(header_only_avif).unwrap(),
            ImageFormat::Avif
        );
        cx.update(|cx| {
            cx.set_http_client(FakeHttpClient::create(move |_| async move {
                Ok(Response::builder()
                    .status(200)
                    .body(header_only_avif.into())
                    .unwrap())
            }));
        });
        let resource = Resource::Uri("https://test.example/header-only.avif".into());
        let error = cx
            .update(|cx| {
                let load: futures::future::BoxFuture<'static, _> =
                    Box::pin(ImageAssetLoader::load(resource, cx));
                load
            })
            .await
            .err()
            .expect("AVIF decoding must remain unsupported without avif-native");
        let ImageCacheError::Other(error) = error else {
            panic!("recognized AVIF must fail in the shared decoder: {error}");
        };
        assert_eq!(error.to_string(), "creating image decoder");
        let Some(ImageError::Unsupported(unsupported)) = error.downcast_ref::<ImageError>() else {
            panic!("expected an unsupported AVIF decoder: {error:#}");
        };
        assert!(matches!(
            unsupported.kind(),
            UnsupportedErrorKind::Format(ImageFormatHint::Exact(ImageFormat::Avif))
        ));
    }

    #[open_gpui::test]
    async fn shared_decoder_errors_keep_context_and_http_status_is_separate(
        cx: &mut TestAppContext,
    ) {
        use open_gpui_http_client::{FakeHttpClient, Response, StatusCode};

        let resource = Resource::Uri("https://test.example/truncated.webp".into());
        let invalid =
            &include_bytes!("../../../../tests/fixtures/images/red-blue-100ms.webp")[..40];
        cx.update(|cx| {
            cx.set_http_client(FakeHttpClient::create(move |_| async move {
                Ok(Response::builder()
                    .status(200)
                    .body(invalid.into())
                    .unwrap())
            }));
        });
        let decoded = cx
            .update(|cx| {
                let load: futures::future::BoxFuture<'static, _> =
                    Box::pin(ImageAssetLoader::load(resource.clone(), cx));
                load
            })
            .await
            .err()
            .expect("the recognized but incomplete WebP must fail");
        assert!(matches!(decoded, ImageCacheError::Other(_)));
        assert!(!decoded.to_string().is_empty());

        cx.update(|cx| cx.set_http_client(FakeHttpClient::with_404_response()));
        let error = cx
            .update(|cx| {
                let load: futures::future::BoxFuture<'static, _> =
                    Box::pin(ImageAssetLoader::load(resource, cx));
                load
            })
            .await
            .err()
            .expect("the HTTP error must precede decoding");
        assert!(matches!(
            error,
            ImageCacheError::BadStatus {
                status: StatusCode::NOT_FOUND,
                ..
            }
        ));
    }
}
