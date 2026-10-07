//! The user's time zone: set in Settings → Account, else the device's.
//!
//! Everything that reads a local date for the user (what "yesterday" means,
//! which calendar day a memory fell on) goes through [`Config::time_zone`],
//! so the user's choice wins over where the machine thinks it is.

use super::Config;

/// The IANA name `zone` normalises to (`Asia/Kolkata`), or `None` when it is
/// blank or not an IANA zone. Case and surrounding space are forgiven; an
/// abbreviation (`IST`) or an offset (`+05:30`) is not a zone.
pub fn normalize_time_zone(zone: &str) -> Option<String> {
    let trimmed = zone.trim();
    if trimmed.is_empty() {
        return None;
    }
    let tz = trimmed.parse::<chrono_tz::Tz>().ok().or_else(|| {
        chrono_tz::TZ_VARIANTS
            .iter()
            .copied()
            .find(|tz| tz.name().eq_ignore_ascii_case(trimmed))
    })?;
    Some(tz.name().to_string())
}

/// The device's IANA zone, when the host can resolve one.
pub fn device_time_zone() -> Option<String> {
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|zone| normalize_time_zone(&zone))
}

impl Config {
    /// The user's time zone: [`Config::user_timezone`] when it is a valid
    /// IANA zone, else the device's, else `UTC`.
    pub fn time_zone(&self) -> String {
        self.user_timezone
            .as_deref()
            .and_then(normalize_time_zone)
            .or_else(device_time_zone)
            .unwrap_or_else(|| "UTC".to_string())
    }
}

#[cfg(test)]
#[path = "time_zone_tests.rs"]
mod tests;
