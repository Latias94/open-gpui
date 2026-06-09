pub(crate) fn cleaned_shares(child_count: usize, fractions: &[f32]) -> Vec<f32> {
    let mut shares: Vec<f32> = (0..child_count)
        .map(|index| fractions.get(index).copied().unwrap_or(1.0))
        .collect();
    normalize_shares(&mut shares);
    shares
}

pub(crate) fn normalize_shares(shares: &mut Vec<f32>) {
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
    fn cleaned_shares_fills_missing_fractions() {
        let shares = cleaned_shares(3, &[0.25]);

        assert_close(shares.iter().sum(), 1.0);
        assert_close(shares[0], 0.1111);
        assert_close(shares[1], 0.4444);
        assert_close(shares[2], 0.4444);
    }

    #[test]
    fn normalize_shares_repairs_non_finite_and_negative_values() {
        let mut shares = vec![f32::NAN, -1.0, 3.0];

        normalize_shares(&mut shares);

        assert_close(shares.iter().sum(), 1.0);
        assert_close(shares[0], 0.0);
        assert_close(shares[1], 0.0);
        assert_close(shares[2], 1.0);
    }

    #[test]
    fn normalize_shares_falls_back_to_equal_shares_when_sum_is_empty() {
        let mut shares = vec![0.0, f32::NAN, -1.0];

        normalize_shares(&mut shares);

        assert_eq!(shares, vec![1.0 / 3.0; 3]);
    }
}
