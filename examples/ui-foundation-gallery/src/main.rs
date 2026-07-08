fn main() {
    let page = initial_page(std::env::args().skip(1));

    open_gpui_platform::application()
        .run(move |cx| open_gpui_ui_foundation_gallery::open_gallery_page(page, cx));
}

fn initial_page(
    args: impl IntoIterator<Item = String>,
) -> open_gpui_ui_foundation_gallery::GalleryPage {
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        let page_id = if let Some(page_id) = arg.strip_prefix("--page=") {
            Some(page_id.to_owned())
        } else if arg == "--page" {
            args.next()
        } else {
            None
        };

        if let Some(page) =
            page_id.and_then(|id| open_gpui_ui_foundation_gallery::GalleryPage::from_id(&id))
        {
            return page;
        }
    }

    open_gpui_ui_foundation_gallery::GalleryPage::Tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_foundation_gallery::GalleryPage;

    #[test]
    fn initial_page_parses_equals_form() {
        assert_eq!(
            initial_page(["--page=components".to_string()]),
            GalleryPage::Components
        );
    }

    #[test]
    fn initial_page_parses_split_form_and_falls_back() {
        assert_eq!(
            initial_page(["--page".to_string(), "overlay".to_string()]),
            GalleryPage::Overlay
        );
        assert_eq!(
            initial_page(["--page=devtools".to_string()]),
            GalleryPage::Devtools
        );
        assert_eq!(
            initial_page(["--page".to_string(), "missing".to_string()]),
            GalleryPage::Tokens
        );
    }
}
