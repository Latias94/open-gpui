use open_gpui::{Bounds, DevicePixels, Rgba, Size};

pub(super) struct ColorGlyphBitmap {
    width: usize,
    height: usize,
    blue: Vec<f32>,
    green: Vec<f32>,
    red: Vec<f32>,
    alpha: Vec<f32>,
}

impl ColorGlyphBitmap {
    pub(super) fn new(size: Size<DevicePixels>) -> Self {
        let width = size.width.0.max(0) as usize;
        let height = size.height.0.max(0) as usize;
        let len = width * height;
        Self {
            width,
            height,
            blue: vec![0.0; len],
            green: vec![0.0; len],
            red: vec![0.0; len],
            alpha: vec![0.0; len],
        }
    }

    pub(super) fn composite(
        &mut self,
        color: Rgba,
        bounds: Bounds<i32>,
        coverage: &[u8],
        gamma_ratios: [f32; 4],
        grayscale_enhanced_contrast: f32,
    ) -> Result<(), &'static str> {
        let Rgba {
            r: red,
            g: green,
            b: blue,
            a: color_alpha,
        } = color;
        let brightness = color_brightness([red, green, blue]);
        let contrast = light_on_dark_contrast(grayscale_enhanced_contrast, brightness);

        let width = self.width as i32;
        let height = self.height as i32;
        let left = bounds.origin.x.clamp(0, width);
        let right = bounds
            .origin
            .x
            .saturating_add(bounds.size.width)
            .clamp(left, width);
        let top = bounds.origin.y.clamp(0, height);
        let bottom = bounds
            .origin
            .y
            .saturating_add(bounds.size.height)
            .clamp(top, height);
        if left == right || top == bottom {
            return Ok(());
        }
        let len = (right - left) as usize;
        let skipped_columns = (left - bounds.origin.x) as usize;
        for row in top..bottom {
            let coverage_start =
                (row - bounds.origin.y) as usize * bounds.size.width as usize + skipped_columns;
            let pixel_start = row as usize * self.width + left as usize;
            let pixels = pixel_start..pixel_start + len;
            let (
                Some(coverage_row),
                Some(blue_row),
                Some(green_row),
                Some(red_row),
                Some(alpha_row),
            ) = (
                coverage.get(coverage_start..coverage_start + len),
                self.blue.get_mut(pixels.clone()),
                self.green.get_mut(pixels.clone()),
                self.red.get_mut(pixels.clone()),
                self.alpha.get_mut(pixels),
            )
            else {
                return Err("color glyph layer coverage is smaller than its bounds");
            };
            for index in 0..len {
                let corrected = apply_alpha_correction(
                    enhance_contrast(coverage_row[index] as f32 / 255.0, contrast),
                    brightness,
                    gamma_ratios,
                );
                let alpha = (corrected * color_alpha).clamp(0.0, 1.0);
                let remaining = 1.0 - alpha;
                blue_row[index] = blue * alpha + blue_row[index] * remaining;
                green_row[index] = green * alpha + green_row[index] * remaining;
                red_row[index] = red * alpha + red_row[index] * remaining;
                alpha_row[index] = alpha + alpha_row[index] * remaining;
            }
        }
        Ok(())
    }

    pub(super) fn into_straight_bgra(self) -> Vec<u8> {
        let len = self.alpha.len();
        let mut bytes = vec![0u8; len * 4];
        let (blue, green, red, alpha) = (
            &self.blue[..len],
            &self.green[..len],
            &self.red[..len],
            &self.alpha[..len],
        );
        for (index, pixel) in bytes.chunks_exact_mut(4).enumerate() {
            let pixel_alpha = alpha[index].clamp(0.0, 1.0);
            let unpremultiply = if pixel_alpha > 0.0 {
                1.0 / pixel_alpha
            } else {
                0.0
            };
            pixel[0] = to_unorm8(blue[index] * unpremultiply);
            pixel[1] = to_unorm8(green[index] * unpremultiply);
            pixel[2] = to_unorm8(red[index] * unpremultiply);
            pixel[3] = to_unorm8(pixel_alpha);
        }
        bytes
    }
}

pub(super) fn union_glyph_bounds(
    bounds: impl IntoIterator<Item = Bounds<DevicePixels>>,
) -> Bounds<DevicePixels> {
    bounds
        .into_iter()
        .filter(|bounds| !bounds.is_empty())
        .reduce(|union, bounds| union.union(&bounds))
        .unwrap_or_default()
}

fn to_unorm8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn color_brightness([red, green, blue]: [f32; 3]) -> f32 {
    red * 0.30 + green * 0.59 + blue * 0.11
}

fn light_on_dark_contrast(enhanced_contrast: f32, brightness: f32) -> f32 {
    enhanced_contrast * (4.0 * (0.75 - brightness)).clamp(0.0, 1.0)
}

fn enhance_contrast(alpha: f32, contrast: f32) -> f32 {
    alpha * (contrast + 1.0) / (alpha * contrast + 1.0)
}

fn apply_alpha_correction(alpha: f32, brightness: f32, gamma_ratios: [f32; 4]) -> f32 {
    let brightness_adjustment = gamma_ratios[0] * brightness + gamma_ratios[1];
    let correction =
        brightness_adjustment * alpha + (gamma_ratios[2] * brightness + gamma_ratios[3]);
    alpha + alpha * (1.0 - alpha) * correction
}

#[cfg(test)]
mod tests;
