//! Independently locked ordinary consumers of Zed
//! 635026fabc38440fce5e1767d125026a17b12012, source ccf4058b7a.
//! Value and style assertions need no test-support. The from_pixels check is
//! type-only: this fixture does not open a Window or exercise window layout.

use open_gpui::{
    AbsoluteLength, DefiniteLength, Length, Pixels, Rems, Styled as _, Window, div, px, rems,
};

#[test]
fn downstream_rem_values_support_zero_accumulation_and_arithmetic() {
    const ZERO: Rems = Rems::ZERO;
    assert_eq!(ZERO, rems(0.));
    assert_eq!(ZERO, Rems::default());

    let mut total = ZERO;
    total += rems(1.5);
    total += rems(0.75);
    total += rems(-0.25);
    assert_eq!(total, rems(2.));
    assert_eq!(total + rems(0.5), rems(2.5));
    assert_eq!(total - rems(0.5), rems(1.5));
    assert_eq!(total * 2., rems(4.));
    assert_eq!(total / 2., rems(1.));
    assert_eq!(-total, rems(-2.));
    assert_eq!(total.to_pixels(px(16.)), px(32.));
    assert_eq!(total.to_pixels(px(24.)), px(48.));
    assert_eq!(ZERO.to_pixels(px(24.)), Pixels::ZERO);

    let mut element = div().w(total).h(ZERO);
    assert_eq!(
        element.style().size.width,
        Some(Length::Definite(DefiniteLength::Absolute(
            AbsoluteLength::Rems(total)
        )))
    );
    assert_eq!(
        element.style().size.height,
        Some(Length::Definite(DefiniteLength::Absolute(
            AbsoluteLength::Rems(ZERO)
        )))
    );
}

#[test]
fn downstream_from_pixels_has_the_public_window_conversion_signature() {
    let _: fn(Pixels, &Window) -> Rems = Rems::from_pixels;
}
