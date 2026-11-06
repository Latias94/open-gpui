use std::{
    fmt::Debug,
    hash::{Hash, Hasher},
    time::Duration,
};

use anyhow::Context as _;
use uuid::Uuid;
use wayland_backend::client::ObjectId;

use open_gpui::{Bounds, DisplayId, Pixels, PlatformDisplay, refresh_interval_from_hz};

#[derive(Debug, Clone)]
pub(crate) struct WaylandDisplay {
    /// The ID of the wl_output object
    pub id: ObjectId,
    pub name: Option<String>,
    pub bounds: Bounds<Pixels>,
    pub refresh_interval: Option<Duration>,
}

impl Hash for WaylandDisplay {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PlatformDisplay for WaylandDisplay {
    fn id(&self) -> DisplayId {
        DisplayId::new(self.id.protocol_id() as u64)
    }

    fn uuid(&self) -> anyhow::Result<Uuid> {
        let name = self
            .name
            .as_ref()
            .context("Wayland display does not have a name")?;
        Ok(Uuid::new_v5(&Uuid::NAMESPACE_DNS, name.as_bytes()))
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    fn refresh_interval(&self) -> Option<Duration> {
        self.refresh_interval
    }
}

pub(crate) fn refresh_interval_from_millihertz(millihertz: i32) -> Option<Duration> {
    refresh_interval_from_hz(f64::from(millihertz) / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_intervals_preserve_fractional_rates_and_unknown_values() {
        for (millihertz, microseconds) in [(60_000, 16_666), (59_940, 16_683), (143_998, 6_944)] {
            assert_eq!(
                refresh_interval_from_millihertz(millihertz).map(|interval| interval.as_micros()),
                Some(microseconds)
            );
        }
        for millihertz in [0, -1, i32::MIN] {
            assert_eq!(refresh_interval_from_millihertz(millihertz), None);
        }
        assert!(refresh_interval_from_millihertz(i32::MAX).is_some());
    }
}
