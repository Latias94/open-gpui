use open_gpui::{OperatingSystem, PlatformInfo, PlatformTarget};

pub(crate) fn from_navigator_report(
    user_agent: &str,
    platform: &str,
    max_touch_points: i32,
) -> PlatformInfo {
    let os = if user_agent.contains("Android") {
        OperatingSystem::Android
    } else if user_agent.contains("iPhone")
        || user_agent.contains("iPad")
        || user_agent.contains("iPod")
        || (platform.contains("Mac") && max_touch_points > 1)
    {
        OperatingSystem::Ios
    } else if platform.contains("Mac") || user_agent.contains("Mac") {
        OperatingSystem::Mac
    } else if platform.contains("Win") || user_agent.contains("Windows") {
        OperatingSystem::Windows
    } else if platform.contains("FreeBSD") || user_agent.contains("FreeBSD") {
        OperatingSystem::FreeBsd
    } else if platform.contains("Linux")
        || user_agent.contains("Linux")
        || user_agent.contains("CrOS")
    {
        OperatingSystem::Linux
    } else {
        OperatingSystem::Unknown
    };
    PlatformInfo {
        os,
        target: PlatformTarget::Web,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigator_report_detection_matrix_preserves_official_priority() {
        use OperatingSystem::*;

        let cases = [
            ("", "", 0, Unknown),
            ("unrecognized browser", "unrecognized platform", 0, Unknown),
            (
                "Mozilla/5.0 (Linux; Android 16; Pixel 9)",
                "Linux armv8l",
                5,
                Android,
            ),
            (
                "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)",
                "iPhone",
                5,
                Ios,
            ),
            (
                "Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X)",
                "iPad",
                5,
                Ios,
            ),
            (
                "Mozilla/5.0 (iPod touch; CPU iPhone OS 18_0 like Mac OS X)",
                "iPod",
                5,
                Ios,
            ),
            (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15)",
                "MacIntel",
                5,
                Ios,
            ),
            ("", "MacIntel", 2, Ios),
            ("", "MacIntel", 1, Mac),
            ("", "MacIntel", 0, Mac),
            ("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15)", "", 5, Mac),
            ("Mozilla/5.0 (Windows NT 10.0; Win64; x64)", "", 0, Windows),
            ("", "Win32", 5, Windows),
            ("Mozilla/5.0 (X11; FreeBSD amd64)", "", 0, FreeBsd),
            ("", "FreeBSD amd64", 0, FreeBsd),
            ("Mozilla/5.0 (X11; Linux x86_64)", "", 0, Linux),
            ("", "Linux x86_64", 0, Linux),
            ("Mozilla/5.0 (X11; CrOS x86_64 16093.68.0)", "", 0, Linux),
            ("", "X11", 0, Unknown),
            ("", "Android", 5, Unknown),
            ("", "iPhone", 5, Unknown),
            (
                "android iphone ipad ipod mac windows freebsd linux cros",
                "macintel",
                5,
                Unknown,
            ),
            (
                "Android iPhone Mac Windows FreeBSD Linux CrOS",
                "MacIntel",
                5,
                Android,
            ),
            ("iPhone Mac Windows FreeBSD Linux CrOS", "Win32", 0, Ios),
            ("iPad Windows FreeBSD Linux", "Win32", 0, Ios),
            ("iPod Windows FreeBSD Linux", "Win32", 0, Ios),
            ("Windows FreeBSD Linux", "MacIntel", 2, Ios),
            ("Mac Windows FreeBSD Linux CrOS", "Win32", 0, Mac),
            ("Windows FreeBSD Linux CrOS", "MacIntel", 1, Mac),
            ("Windows FreeBSD Linux CrOS", "FreeBSD", 0, Windows),
            ("FreeBSD Linux CrOS", "Win32", 0, Windows),
            ("FreeBSD Linux CrOS", "Linux x86_64", 0, FreeBsd),
            ("Linux CrOS", "FreeBSD", 0, FreeBsd),
        ];
        for (user_agent, platform, max_touch_points, os) in cases {
            let actual = from_navigator_report(user_agent, platform, max_touch_points);
            assert_eq!(
                actual,
                PlatformInfo {
                    os,
                    target: PlatformTarget::Web
                }
            );
            assert_eq!(actual.is_mac(), os == Mac);
        }
    }
}
