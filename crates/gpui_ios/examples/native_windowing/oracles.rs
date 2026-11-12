#[derive(Clone, Copy, Debug)]
pub(super) enum ExpectedPixel {
    Rgb([u8; 3]),
    Gray { minimum: u8, maximum: u8 },
}

impl ExpectedPixel {
    fn matches(self, actual: [u8; 4]) -> bool {
        if actual[3] != 255 {
            return false;
        }
        match self {
            Self::Rgb(expected) => actual[..3]
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= 3),
            Self::Gray { minimum, maximum } => {
                actual[..3]
                    .iter()
                    .all(|value| (minimum..=maximum).contains(value))
                    && actual[..3].iter().max().unwrap() - actual[..3].iter().min().unwrap() <= 2
            }
        }
    }
}

pub(super) struct PixelSample {
    pub(super) position: (f32, f32),
    pub(super) expected: ExpectedPixel,
}

pub(super) struct RenderingCase {
    pub(super) name: &'static str,
    pub(super) samples: &'static [PixelSample],
}

const fn rgb(horizontal: f32, vertical: f32, color: [u8; 3]) -> PixelSample {
    PixelSample {
        position: (horizontal, vertical),
        expected: ExpectedPixel::Rgb(color),
    }
}

pub(super) const CASES: &[RenderingCase] = &[
    RenderingCase {
        name: "fractional rounded clipping",
        samples: &[
            rgb(24., 24., [255, 0, 0]),
            rgb(88., 24., [255, 255, 255]),
            rgb(5., 5., [255, 255, 255]),
        ],
    },
    RenderingCase {
        name: "background and border",
        samples: &[
            rgb(124., 24., [0, 0, 255]),
            rgb(104.5, 24., [0, 255, 0]),
            rgb(188., 24., [255, 255, 255]),
        ],
    },
    RenderingCase {
        name: "analytic quadratic path",
        samples: &[
            rgb(24., 124., [0, 255, 0]),
            rgb(40., 96., [0, 255, 0]),
            rgb(64., 92., [255, 255, 255]),
        ],
    },
    RenderingCase {
        name: "layered overlap",
        samples: &[
            rgb(108., 88., [255, 0, 0]),
            rgb(124., 104., [0, 255, 255]),
            rgb(188., 104., [255, 255, 255]),
        ],
    },
    RenderingCase {
        name: "translucent over opaque order",
        samples: &[
            rgb(24., 184., [128, 0, 128]),
            rgb(24., 224., [255, 255, 255]),
        ],
    },
    RenderingCase {
        name: "rounded shadow",
        samples: &[
            rgb(124., 184., [255, 255, 0]),
            PixelSample {
                position: (105., 165.),
                expected: ExpectedPixel::Gray {
                    minimum: 200,
                    maximum: 255,
                },
            },
            PixelSample {
                position: (144., 226.),
                expected: ExpectedPixel::Gray {
                    minimum: 200,
                    maximum: 252,
                },
            },
            rgb(204., 248., [255, 255, 255]),
        ],
    },
];

pub(super) fn check_case(
    case: &RenderingCase,
    mut pixel_at: impl FnMut((f32, f32)) -> [u8; 4],
) -> Result<(), String> {
    for sample in case.samples {
        let actual = pixel_at(sample.position);
        if !sample.expected.matches(actual) {
            return Err(format!(
                "{} at {:?}: expected {:?}, got RGBA {actual:?}",
                case.name, sample.position, sample.expected
            ));
        }
    }
    Ok(())
}

pub(super) fn resize_was_submitted(
    baseline_generation: u64,
    baseline_serial: u64,
    generation: Option<u64>,
    serial: u64,
    extent: (usize, usize),
    expected_extent: (usize, usize),
) -> bool {
    generation.is_some_and(|generation| generation > baseline_generation)
        && serial > baseline_serial
        && extent == expected_extent
        && extent.0 > 0
        && extent.1 > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rendering_case_rejects_solid_black_and_white_images() {
        for case in CASES {
            for pixel in [[0, 0, 0, 255], [255; 4]] {
                assert!(
                    check_case(case, |_| pixel).is_err(),
                    "{} accepted {pixel:?}",
                    case.name
                );
            }
        }
    }

    #[test]
    fn individual_samples_reject_missing_clipping_border_path_order_and_shadow() {
        for (case, sample, wrong) in [
            (0, 1, [255, 0, 0, 255]),
            (0, 2, [255, 0, 0, 255]),
            (1, 1, [0, 0, 255, 255]),
            (2, 1, [255; 4]),
            (2, 2, [0, 255, 0, 255]),
            (3, 1, [255, 0, 0, 255]),
            (4, 0, [255, 0, 0, 255]),
            (4, 0, [0, 0, 255, 255]),
            (5, 1, [255, 255, 0, 255]),
            (5, 2, [255; 4]),
        ] {
            assert!(!CASES[case].samples[sample].expected.matches(wrong));
        }
    }

    #[test]
    fn color_checks_accept_the_expected_blend_and_shadow_without_accepting_transparency() {
        assert!(ExpectedPixel::Rgb([128, 0, 128]).matches([127, 0, 128, 255]));
        assert!(!ExpectedPixel::Rgb([128, 0, 128]).matches([128, 0, 128, 128]));
        let shadow = ExpectedPixel::Gray {
            minimum: 200,
            maximum: 252,
        };
        assert!(shadow.matches([241, 241, 241, 255]));
        assert!(!shadow.matches([241, 220, 241, 255]));
    }

    #[test]
    fn resize_requires_both_new_submission_facts_and_the_actual_target_extent() {
        assert!(!resize_was_submitted(
            4,
            8,
            Some(4),
            9,
            (300, 200),
            (300, 200)
        ));
        assert!(!resize_was_submitted(
            4,
            8,
            Some(5),
            8,
            (300, 200),
            (300, 200)
        ));
        assert!(!resize_was_submitted(
            4,
            8,
            Some(5),
            9,
            (320, 200),
            (300, 200)
        ));
        assert!(!resize_was_submitted(4, 8, None, 9, (300, 200), (300, 200)));
        assert!(!resize_was_submitted(4, 8, Some(5), 9, (0, 200), (0, 200)));
        assert!(resize_was_submitted(
            4,
            8,
            Some(5),
            9,
            (300, 200),
            (300, 200)
        ));
    }
}
