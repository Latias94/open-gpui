use open_gpui::PowerMode;

pub(super) fn power_mode(profile: &str) -> PowerMode {
    match profile {
        "power-saver" => PowerMode::LowPower,
        "balanced" => PowerMode::Balanced,
        "performance" => PowerMode::HighPower,
        _ => PowerMode::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_profiles_without_guessing_unknown_values() {
        for (profile, expected) in [
            ("power-saver", PowerMode::LowPower),
            ("balanced", PowerMode::Balanced),
            ("performance", PowerMode::HighPower),
            ("custom", PowerMode::Unknown),
            ("", PowerMode::Unknown),
            ("Balanced", PowerMode::Unknown),
            ("balanced ", PowerMode::Unknown),
            ("battery", PowerMode::Unknown),
            ("ac", PowerMode::Unknown),
        ] {
            assert_eq!(power_mode(profile), expected);
        }
    }
}
