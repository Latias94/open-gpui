use open_gpui::Pixels;
#[cfg(test)]
use open_gpui::px;

pub(crate) fn resize_adjacent_fractions(
    fractions: &[f32],
    child_count: usize,
    handle_index: usize,
    split_extent: Pixels,
    delta: Pixels,
    min_pane_size: Pixels,
) -> Option<Vec<f32>> {
    if child_count < 2 || handle_index + 1 >= child_count {
        return None;
    }

    let extent = f32::from(split_extent);
    if !extent.is_finite() || extent <= f32::EPSILON {
        return None;
    }

    let mut shares = cleaned_shares(child_count, fractions);
    let pair_total = shares[handle_index] + shares[handle_index + 1];
    if !pair_total.is_finite() || pair_total <= f32::EPSILON {
        return None;
    }

    let min_fraction = (f32::from(min_pane_size).max(0.0) / extent).clamp(0.0, pair_total / 2.0);
    let delta_fraction = f32::from(delta) / extent;
    let next_first =
        (shares[handle_index] + delta_fraction).clamp(min_fraction, pair_total - min_fraction);

    shares[handle_index] = next_first;
    shares[handle_index + 1] = pair_total - next_first;
    normalize_shares(&mut shares);
    Some(shares)
}

pub(crate) fn cleaned_shares(child_count: usize, fractions: &[f32]) -> Vec<f32> {
    let mut shares: Vec<f32> = (0..child_count)
        .map(|index| fractions.get(index).copied().unwrap_or(1.0))
        .collect();
    normalize_shares(&mut shares);
    shares
}

fn normalize_shares(shares: &mut Vec<f32>) {
    for share in shares.iter_mut() {
        if !share.is_finite() || *share < 0.0 {
            *share = 0.0;
        }
    }

    let sum: f32 = shares.iter().sum();
    if !sum.is_finite() || sum <= f32::EPSILON {
        let len = shares.len().max(1);
        *shares = vec![1.0 / len as f32; len];
        return;
    }

    for share in shares.iter_mut() {
        *share /= sum;
    }

    if !shares.is_empty() {
        let rest: f32 = shares.iter().take(shares.len().saturating_sub(1)).sum();
        let last = shares.len().saturating_sub(1);
        shares[last] = (1.0 - rest).clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.001,
            "expected {actual} to be close to {expected}"
        );
    }

    #[test]
    fn positive_delta_grows_first_adjacent_pane() {
        let next = resize_adjacent_fractions(&[0.25, 0.75], 2, 0, px(400.0), px(40.0), px(48.0))
            .expect("resize should be valid");

        assert_close(next[0], 0.35);
        assert_close(next[1], 0.65);
    }

    #[test]
    fn negative_delta_shrinks_first_adjacent_pane() {
        let next = resize_adjacent_fractions(&[0.5, 0.5], 2, 0, px(400.0), px(-80.0), px(48.0))
            .expect("resize should be valid");

        assert_close(next[0], 0.3);
        assert_close(next[1], 0.7);
    }

    #[test]
    fn resize_clamps_at_minimum_pane_size() {
        let next = resize_adjacent_fractions(&[0.5, 0.5], 2, 0, px(400.0), px(-300.0), px(100.0))
            .expect("resize should be valid");

        assert_close(next[0], 0.25);
        assert_close(next[1], 0.75);
    }

    #[test]
    fn impossible_minimum_splits_adjacent_pair_evenly() {
        let next = resize_adjacent_fractions(&[0.5, 0.5], 2, 0, px(120.0), px(100.0), px(80.0))
            .expect("resize should be valid");

        assert_close(next[0], 0.5);
        assert_close(next[1], 0.5);
    }

    #[test]
    fn mismatched_and_non_finite_input_is_repaired() {
        let next = resize_adjacent_fractions(&[f32::NAN], 3, 1, px(300.0), px(30.0), px(24.0))
            .expect("resize should be valid");

        assert_close(next.iter().sum(), 1.0);
        assert_close(next[0], 0.0);
        assert_close(next[1], 0.6);
        assert_close(next[2], 0.4);
    }

    #[test]
    fn invalid_handle_index_returns_none() {
        assert!(
            resize_adjacent_fractions(&[0.5, 0.5], 2, 1, px(400.0), px(10.0), px(48.0)).is_none()
        );
    }
}
