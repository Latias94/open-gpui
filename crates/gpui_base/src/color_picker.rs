use open_gpui::{
    Context, Entity, EventEmitter, Refineable, Rgba, Role, StyleRefinement, Subscription, Window,
    div, prelude::*,
};

use crate::{Button, NumericEvent, NumericRange, Slider};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorMode {
    #[default]
    Rgba,
    Hsva,
}

/// HSV with hue in degrees and saturation, value, and alpha in the 0–1 range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsva {
    pub hue: f64,
    pub saturation: f64,
    pub value: f64,
    pub alpha: f64,
}

impl Hsva {
    pub fn from_rgba(color: Rgba) -> Self {
        let color = sanitize_rgba(color);
        let [red, green, blue] = [color.r, color.g, color.b].map(f64::from);
        let value = red.max(green).max(blue);
        let chroma = value - red.min(green).min(blue);
        let hue = if chroma == 0. {
            0.
        } else if value == red {
            ((green - blue) / chroma).rem_euclid(6.)
        } else if value == green {
            (blue - red) / chroma + 2.
        } else {
            (red - green) / chroma + 4.
        };
        Self {
            hue: hue * 60.,
            saturation: if value == 0. { 0. } else { chroma / value },
            value,
            alpha: f64::from(color.a),
        }
    }

    pub fn to_rgba(self) -> Rgba {
        let hue = if self.hue.is_finite() {
            self.hue.rem_euclid(360.) / 60.
        } else {
            0.
        };
        let value = unit(self.value);
        let chroma = value * unit(self.saturation);
        let second = chroma * (1. - ((hue % 2.) - 1.).abs());
        let offset = value - chroma;
        let [red, green, blue] = match hue as u8 {
            0 => [chroma, second, 0.],
            1 => [second, chroma, 0.],
            2 => [0., chroma, second],
            3 => [0., second, chroma],
            4 => [second, 0., chroma],
            _ => [chroma, 0., second],
        };
        Rgba {
            r: (red + offset) as f32,
            g: (green + offset) as f32,
            b: (blue + offset) as f32,
            a: unit(self.alpha) as f32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorPickerEvent {
    Change(Rgba),
    Commit(Rgba),
    Cancel(Rgba),
}

/// RGBA or HSVA channel controls with a live preview and shared alpha channel.
pub struct ColorPicker {
    color: Rgba,
    hsv: Hsva,
    mode: ColorMode,
    sliders: [Entity<Slider>; 7],
    _subscriptions: [Subscription; 7],
    style: StyleRefinement,
}

impl ColorPicker {
    pub fn new(color: Rgba, cx: &mut Context<Self>) -> Self {
        let color = sanitize_rgba(color);
        let hsv = Hsva::from_rgba(color);
        let sliders = Channel::ALL.map(|channel| {
            cx.new(|cx| {
                let mut slider = Slider::new(channel.value(color, hsv), channel.range(), cx);
                slider.set_label(channel.label(), cx);
                slider
            })
        });
        let subscriptions = Channel::ALL.map(|channel| {
            cx.subscribe(&sliders[channel as usize], move |picker, _, event, cx| {
                picker.on_channel(channel, *event, cx)
            })
        });
        Self {
            color,
            hsv,
            mode: ColorMode::Rgba,
            sliders,
            _subscriptions: subscriptions,
            style: StyleRefinement::default(),
        }
    }

    pub fn value(&self) -> Rgba {
        self.color
    }
    pub fn hsva(&self) -> Hsva {
        self.hsv
    }
    pub fn mode(&self) -> ColorMode {
        self.mode
    }

    pub fn set_value(&mut self, color: Rgba, cx: &mut Context<Self>) {
        let color = sanitize_rgba(color);
        if self.color != color {
            self.set_rgba(color);
            self.sync_sliders(None, cx);
            cx.notify();
        }
    }

    pub fn set_mode(&mut self, mode: ColorMode, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            cx.notify();
        }
    }

    pub fn set_style(&mut self, style: StyleRefinement, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    fn set_rgba(&mut self, rgba: Rgba) {
        self.color = rgba;
        let mut hsv = Hsva::from_rgba(rgba);
        if hsv.saturation == 0. {
            hsv.hue = self.hsv.hue;
        }
        self.hsv = hsv;
    }

    fn sync_sliders(&self, source: Option<Channel>, cx: &mut Context<Self>) {
        for channel in Channel::ALL {
            if Some(channel) != source {
                let value = channel.value(self.color, self.hsv);
                self.sliders[channel as usize].update(cx, |slider, cx| slider.set_value(value, cx));
            }
        }
    }

    fn on_channel(&mut self, channel: Channel, event: NumericEvent, cx: &mut Context<Self>) {
        let value = match event {
            NumericEvent::Change(value)
            | NumericEvent::Commit(value)
            | NumericEvent::Cancel(value) => value,
        };
        let before = self.color;
        let mut rgba = self.color;
        match channel {
            Channel::Red => {
                rgba.r = value as f32;
                self.set_rgba(rgba);
            }
            Channel::Green => {
                rgba.g = value as f32;
                self.set_rgba(rgba);
            }
            Channel::Blue => {
                rgba.b = value as f32;
                self.set_rgba(rgba);
            }
            Channel::Alpha => {
                self.color.a = value as f32;
                self.hsv.alpha = value;
            }
            Channel::Hue => {
                self.hsv.hue = value;
                self.color = self.hsv.to_rgba();
            }
            Channel::Saturation => {
                self.hsv.saturation = value;
                self.color = self.hsv.to_rgba();
            }
            Channel::Value => {
                self.hsv.value = value;
                self.color = self.hsv.to_rgba();
            }
        }
        self.sync_sliders(Some(channel), cx);
        cx.notify();
        match event {
            NumericEvent::Change(_) if before != self.color => {
                cx.emit(ColorPickerEvent::Change(self.color))
            }
            NumericEvent::Commit(_) => cx.emit(ColorPickerEvent::Commit(self.color)),
            NumericEvent::Cancel(_) => cx.emit(ColorPickerEvent::Cancel(self.color)),
            _ => {}
        }
    }
}

impl EventEmitter<ColorPickerEvent> for ColorPicker {}

impl Render for ColorPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mode = self.mode;
        let other = if mode == ColorMode::Rgba {
            ColorMode::Hsva
        } else {
            ColorMode::Rgba
        };
        let mode_label = if mode == ColorMode::Rgba {
            "RGBA"
        } else {
            "HSVA"
        };
        let channels = if mode == ColorMode::Rgba {
            [Channel::Red, Channel::Green, Channel::Blue, Channel::Alpha]
        } else {
            [
                Channel::Hue,
                Channel::Saturation,
                Channel::Value,
                Channel::Alpha,
            ]
        };
        let mut root = div().flex().flex_col().min_w_0().gap_1();
        root.style().refine(&self.style);
        root.id("color-picker")
            .role(Role::Group)
            .aria_label("Color")
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().size_4().bg(self.color))
                    .child(
                        Button::new("mode", mode_label).on_click(
                            cx.listener(move |picker, _, _, cx| picker.set_mode(other, cx)),
                        ),
                    ),
            )
            .children(channels.map(|channel| {
                let value = channel.value(self.color, self.hsv);
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w_20().child(channel.label()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(self.sliders[channel as usize].clone()),
                    )
                    .child(format!("{value:.2}"))
            }))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Channel {
    Red,
    Green,
    Blue,
    Hue,
    Saturation,
    Value,
    Alpha,
}

impl Channel {
    const ALL: [Self; 7] = [
        Self::Red,
        Self::Green,
        Self::Blue,
        Self::Hue,
        Self::Saturation,
        Self::Value,
        Self::Alpha,
    ];

    fn range(self) -> NumericRange {
        let max = if self == Self::Hue { 360. } else { 1. };
        NumericRange {
            min: 0.,
            max,
            step: None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Hue => "Hue",
            Self::Saturation => "Saturation",
            Self::Value => "Value",
            Self::Alpha => "Alpha",
        }
    }

    fn value(self, color: Rgba, hsv: Hsva) -> f64 {
        match self {
            Self::Red => f64::from(color.r),
            Self::Green => f64::from(color.g),
            Self::Blue => f64::from(color.b),
            Self::Hue => hsv.hue,
            Self::Saturation => hsv.saturation,
            Self::Value => hsv.value,
            Self::Alpha => hsv.alpha,
        }
    }
}

fn unit(value: f64) -> f64 {
    if value.is_nan() {
        0.
    } else {
        value.clamp(0., 1.)
    }
}

fn sanitize_rgba(color: Rgba) -> Rgba {
    let [r, g, b, a] =
        [color.r, color.g, color.b, color.a].map(|value| unit(f64::from(value)) as f32);
    Rgba { r, g, b, a }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsva_round_trips_color_and_alpha_across_all_hue_sectors() {
        for hue in (0..360).step_by(15) {
            let original = Hsva {
                hue: f64::from(hue),
                saturation: 0.7,
                value: 0.9,
                alpha: 0.35,
            };
            let restored = Hsva::from_rgba(original.to_rgba());
            assert!((restored.hue - original.hue).abs() < 0.001);
            assert!((restored.saturation - original.saturation).abs() < 0.001);
            assert!((restored.value - original.value).abs() < 0.001);
            assert!((restored.alpha - original.alpha).abs() < 0.001);
        }
    }

    #[test]
    fn hsva_wraps_hue_and_clamps_other_channels() {
        assert_eq!(
            Hsva {
                hue: -120.,
                saturation: 5.,
                value: 3.,
                alpha: f64::NAN
            }
            .to_rgba(),
            Rgba {
                r: 0.,
                g: 0.,
                b: 1.,
                a: 0.
            }
        );
    }
}
