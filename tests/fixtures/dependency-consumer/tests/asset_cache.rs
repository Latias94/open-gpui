use open_gpui::{App, Asset, ImageCacheError, ImageCacheItem, RenderImage, Resource, Window};
use std::{future::Future, sync::Arc};

struct ConsumerAsset;

impl Asset for ConsumerAsset {
    type Source = String;
    type Output = Result<String, &'static str>;

    fn load(
        source: Self::Source,
        _: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        std::future::ready(Ok(source))
    }
}

#[test]
fn downstream_asset_consumers_receive_optional_outputs_without_task_handles() {
    let _: fn(&mut App, &String) -> Option<Result<String, &'static str>> =
        App::fetch_asset::<ConsumerAsset>;
    let _: fn(&mut App, &String) = App::remove_asset::<ConsumerAsset>;
    let _: fn(&mut Window, &String, &mut App) -> Option<Result<String, &'static str>> =
        Window::use_asset::<ConsumerAsset>;
    let _: fn(&mut Window, &String, &mut App) -> Option<Result<String, &'static str>> =
        Window::get_asset::<ConsumerAsset>;
    let _consumer = |cx: &mut App, window: &mut Window| {
        let source = String::from("downstream owning asset");
        let fetched: Option<Result<String, &'static str>> =
            cx.fetch_asset::<ConsumerAsset>(&source);
        let subscribed: Option<Result<String, &'static str>> =
            window.use_asset::<ConsumerAsset>(&source, cx);
        let queried: Option<Result<String, &'static str>> =
            window.get_asset::<ConsumerAsset>(&source, cx);
        cx.remove_asset::<ConsumerAsset>(&source);
        (fetched, subscribed, queried)
    };
}

#[test]
fn downstream_image_cache_consumers_own_opaque_entries_and_borrow_results() {
    let _: fn(&Resource, &mut App) -> ImageCacheItem = ImageCacheItem::new;
    let _: fn(&ImageCacheItem) -> Option<Result<Arc<RenderImage>, ImageCacheError>> =
        ImageCacheItem::get;
    let _: fn(&ImageCacheItem, &Window) -> Option<Result<Arc<RenderImage>, ImageCacheError>> =
        ImageCacheItem::use_image;
    let _consumer = |source: &Resource, cx: &mut App, window: &Window| {
        let entry: ImageCacheItem = ImageCacheItem::new(source, cx);
        let shared: &ImageCacheItem = &entry;
        let queried: Option<Result<Arc<RenderImage>, ImageCacheError>> = shared.get();
        let subscribed: Option<Result<Arc<RenderImage>, ImageCacheError>> =
            shared.use_image(window);
        (entry, queried, subscribed)
    };
}
