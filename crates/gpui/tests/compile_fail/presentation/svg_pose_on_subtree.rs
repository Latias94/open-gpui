use open_gpui::{Transformation, div, radians};

fn main() {
    let _ = div().with_transformation(Transformation::rotate(radians(0.5)));
}
