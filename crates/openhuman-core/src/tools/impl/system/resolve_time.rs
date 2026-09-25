//! Tool: resolve_time — convert a relative or absolute time expression into an
//! exact timestamp in every format a downstream tool might want.
//!
//! Motivation: LLMs are unreliable at Unix-epoch arithmetic. A real incident
//! had the integrations agent compute "24 hours ago" as `1752189120`
//! (2025-07-10) instead of the intended 2026-06-09 — ~10 months off — then
//! fetch Slack history ascending from that wrong floor and never reach the
//! latest messages. `current_time` only returns *now*, so the agent still has
//! to subtract by hand; this tool does the resolution deterministically and
//! returns the value ready to paste into a tool argument
//! (`oldest`/`latest`/`since`/`after`, cron times, …).
//!
//! Read-only, no side effects. Accepts durations, conversational calendar
//! phrases, and ISO dates/timestamps. Bare durations look backward; `in` and
//! `next` look forward. Civil dates and clock times use the requested timezone.
//!
//! Returns every common representation so the caller can pick the one the
//! target tool's schema wants:
//!   - `unix_s`     — Unix seconds (integer)
//!   - `unix_ms`    — Unix milliseconds (integer)
//!   - `slack_ts`   — Slack `conversations.history` style `"<secs>.000000"`
//!   - `rfc3339`    — `"2026-06-09T19:12:00+00:00"`
//!   - `value`      — Unix seconds as a string for copy-paste; legacy callers
//!                    may still select another representation with `format`.

use async_trait::async_trait;
use chrono::{
    DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, SecondsFormat, Utc,
    Weekday,
};
use chrono_tz::Tz;
use serde_json::json;
use tinytools::{PermissionLevel, Tool, ToolCallOptions, ToolResult};

pub struct ResolveTimeTool;

impl ResolveTimeTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ResolveTimeTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a relative-duration expression into a **signed** [`Duration`] offset
/// from now: negative = the past, positive = the future.
///
/// Direction comes from `ago`, `last`, `past`, or `-` for the past and `in`,
/// `next`, `from now`, or `+` for the future. A bare duration looks backward.
///
/// Getting the sign right matters: `scheduler_agent` passes future phrasing
/// like `"in 10 minutes"`, which must resolve forward, not backward.
fn parse_relative_duration(raw: &str) -> Option<Duration> {
    let mut s = raw.trim().to_ascii_lowercase();
    let mut future = false;

    // Suffix direction markers.
    if let Some(rest) = s.strip_suffix(" ago") {
        s = rest.trim().to_string();
    } else if let Some(rest) = s.strip_suffix(" from now") {
        future = true;
        s = rest.trim().to_string();
    }
    // Prefix direction markers (first match wins).
    for (prefix, is_future) in [
        ("in ", true),
        ("next ", true),
        ("last ", false),
        ("past ", false),
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            future = is_future;
            s = rest.trim().to_string();
            break;
        }
    }
    if let Some(rest) = s.strip_prefix('+') {
        future = true;
        s = rest.trim().to_string();
    } else if let Some(rest) = s.strip_prefix('-') {
        // Leading '-' is the past — which is already the default — just strip it.
        s = rest.trim().to_string();
    }

    // Split both "2 hours 30 minutes" and "2h30m" into number/unit pairs.
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut was_digit = None;
    for c in s.chars() {
        if c.is_ascii_digit() || c.is_ascii_alphabetic() {
            let is_digit = c.is_ascii_digit();
            if was_digit.is_some_and(|previous| previous != is_digit) && !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            current.push(c);
            was_digit = Some(is_digit);
        } else if c.is_whitespace() || c == ',' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            was_digit = None;
        } else {
            return None;
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    let mut parts = tokens.into_iter().filter(|token| token != "and");
    let mut seconds = 0_i64;
    let mut any = false;
    while let Some(number) = parts.next() {
        let n: i64 = number.parse().ok()?;
        let unit = parts.next()?;
        let secs_per = match unit.as_str() {
            "s" | "sec" | "secs" | "second" | "seconds" => 1,
            "m" | "min" | "mins" | "minute" | "minutes" => 60,
            "h" | "hr" | "hrs" | "hour" | "hours" => 3_600,
            "d" | "day" | "days" => 86_400,
            "w" | "wk" | "wks" | "week" | "weeks" => 604_800,
            _ => return None,
        };
        seconds = seconds.checked_add(n.checked_mul(secs_per)?)?;
        any = true;
    }
    if !any {
        return None;
    }
    let magnitude = Duration::try_seconds(seconds)?;
    Some(if future { magnitude } else { -magnitude })
}

/// Resolve `expr` to an absolute UTC instant. `zone` interprets civil
/// inputs (`today`, `yesterday`, bare dates without an explicit offset).
fn parse_weekday(name: &str) -> Option<Weekday> {
    match name {
        "monday" | "mon" => Some(Weekday::Mon),
        "tuesday" | "tue" | "tues" => Some(Weekday::Tue),
        "wednesday" | "wed" => Some(Weekday::Wed),
        "thursday" | "thu" | "thur" | "thurs" => Some(Weekday::Thu),
        "friday" | "fri" => Some(Weekday::Fri),
        "saturday" | "sat" => Some(Weekday::Sat),
        "sunday" | "sun" => Some(Weekday::Sun),
        _ => None,
    }
}

fn parse_clock_time(raw: &str) -> Option<NaiveTime> {
    let compact = raw
        .trim()
        .trim_start_matches("at ")
        .replace([' ', '.'], "")
        .to_ascii_uppercase();
    match compact.as_str() {
        "NOON" => return NaiveTime::from_hms_opt(12, 0, 0),
        "MIDNIGHT" => return NaiveTime::from_hms_opt(0, 0, 0),
        _ => {}
    }
    let (clock, meridiem) = if let Some(clock) = compact.strip_suffix("AM") {
        (clock, Some(false))
    } else if let Some(clock) = compact.strip_suffix("PM") {
        (clock, Some(true))
    } else {
        (compact.as_str(), None)
    };
    let parts: Vec<&str> = clock.split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts.get(1).map_or(Some(0), |s| s.parse().ok())?;
    let second: u32 = parts.get(2).map_or(Some(0), |s| s.parse().ok())?;
    if let Some(pm) = meridiem {
        if !(1..=12).contains(&hour) {
            return None;
        }
        hour = hour % 12 + if pm { 12 } else { 0 };
    }
    NaiveTime::from_hms_opt(hour, minute, second)
}

/// A calendar phrase may be a day on its own or a day plus a clock time.
/// Unqualified weekdays refer to the most recent occurrence, which makes
/// "since Monday" useful for history queries; "next" is always in the future.
fn resolve_calendar_phrase(
    lower: &str,
    zone: &ResolveZone,
    now: DateTime<Utc>,
) -> Option<Result<DateTime<Utc>, String>> {
    let words: Vec<&str> = lower.split_whitespace().collect();
    let first = *words.first()?;
    let anchor_len = if matches!(first, "last" | "next" | "this" | "since")
        && words
            .get(1)
            .is_some_and(|word| parse_weekday(word).is_some())
    {
        2
    } else if matches!(first, "today" | "tomorrow" | "yesterday" | "tonight")
        || parse_weekday(first).is_some()
    {
        1
    } else {
        return None;
    };
    let anchor = words[..anchor_len].join(" ");
    let time_words = words[anchor_len..].join(" ");
    let time = if time_words.is_empty() {
        // "Tonight" is a period, not an exact instant. Require a clock time
        // rather than silently inventing one for a reminder.
        if anchor == "tonight" {
            return None;
        }
        NaiveTime::from_hms_opt(0, 0, 0)?
    } else {
        parse_clock_time(&time_words)?
    };
    let today = zone.civil_date(now);
    let date = match anchor.as_str() {
        "today" => today,
        "tonight" if time == NaiveTime::from_hms_opt(0, 0, 0)? => today + Duration::days(1),
        "tonight" => today,
        "yesterday" => today - Duration::days(1),
        "tomorrow" => today + Duration::days(1),
        _ => {
            let (direction, weekday_name) = anchor
                .split_once(' ')
                .map_or(("since", anchor.as_str()), |(a, b)| (a, b));
            let weekday = parse_weekday(weekday_name)?;
            let current = today.weekday().num_days_from_monday() as i64;
            let target = weekday.num_days_from_monday() as i64;
            let days = match direction {
                "next" => {
                    let ahead = (target - current).rem_euclid(7);
                    if ahead == 0 {
                        7
                    } else {
                        ahead
                    }
                }
                "this" => target - current,
                "last" => {
                    let behind = (current - target).rem_euclid(7);
                    if behind == 0 {
                        -7
                    } else {
                        -behind
                    }
                }
                _ => -(current - target).rem_euclid(7),
            };
            today + Duration::days(days)
        }
    };
    Some(zone.naive_to_utc(date.and_time(time)))
}

fn resolve_expr(expr: &str, zone: ResolveZone) -> Result<DateTime<Utc>, String> {
    resolve_expr_at(expr, zone, Utc::now())
}

fn resolve_expr_at(
    expr: &str,
    zone: ResolveZone,
    now: DateTime<Utc>,
) -> Result<DateTime<Utc>, String> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return Err(
            "`expr` is required (e.g. \"24h ago\", \"2026-06-09T19:12:00Z\", \"now\").".into(),
        );
    }
    let lower = trimmed.to_ascii_lowercase();

    if lower == "now" {
        return Ok(now);
    }

    // Relative duration → signed offset from now (sign encodes past/future).
    if let Some(dur) = parse_relative_duration(trimmed) {
        return now
            .checked_add_signed(dur)
            .ok_or_else(|| "relative time is outside the supported date range".to_string());
    }

    if let Some(result) = resolve_calendar_phrase(&lower, &zone, now) {
        return result;
    }

    // RFC-3339 / ISO-8601 with explicit offset (e.g. ...Z, +05:30).
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(dt.with_timezone(&Utc));
    }

    // "YYYY-MM-DD HH:MM:SS" or "YYYY-MM-DDTHH:MM:SS" (no offset) → resolve in zone.
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M"] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, fmt) {
            return zone.naive_to_utc(naive);
        }
    }

    // Bare date "YYYY-MM-DD" → civil midnight in zone.
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return zone.civil_midnight_to_utc(date);
    }

    // Date with a conversational clock time, e.g. "2026-06-09 at 9am".
    if let Some((date_text, clock_text)) = trimmed.split_once(' ') {
        if let (Ok(date), Some(time)) = (
            NaiveDate::parse_from_str(date_text, "%Y-%m-%d"),
            parse_clock_time(clock_text),
        ) {
            return zone.naive_to_utc(date.and_time(time));
        }
    }

    // Accept time-first forms too: "11pm tonight", "9am next Friday".
    let words: Vec<&str> = lower.split_whitespace().collect();
    for anchor_len in [2, 1] {
        if words.len() > anchor_len {
            let split = words.len() - anchor_len;
            let clock = words[..split].join(" ");
            if parse_clock_time(&clock).is_some() {
                let reordered = format!("{} {clock}", words[split..].join(" "));
                if let Some(result) = resolve_calendar_phrase(&reordered, &zone, now) {
                    return result;
                }
            }
        }
    }

    Err(format!(
        "could not parse {trimmed:?}; try 'last 24 hours', 'tomorrow at 9am', \
         'since Monday', or an ISO date/time."
    ))
}

/// Zone used to interpret civil (offset-less) inputs.
enum ResolveZone {
    /// Machine-local timezone (the default, matching `current_time`).
    Local,
    /// An explicit IANA zone supplied by the caller.
    Iana(Tz),
}

impl ResolveZone {
    fn civil_date(&self, now: DateTime<Utc>) -> NaiveDate {
        match self {
            ResolveZone::Local => now.with_timezone(&Local).date_naive(),
            ResolveZone::Iana(tz) => now.with_timezone(tz).date_naive(),
        }
    }

    fn civil_midnight_to_utc(&self, date: NaiveDate) -> Result<DateTime<Utc>, String> {
        let naive = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| "invalid civil midnight".to_string())?;
        self.naive_to_utc(naive)
    }

    /// Interpret a naive (offset-less) datetime in this zone and convert to UTC.
    fn naive_to_utc(&self, naive: NaiveDateTime) -> Result<DateTime<Utc>, String> {
        use chrono::TimeZone;
        match self {
            ResolveZone::Local => Local
                .from_local_datetime(&naive)
                .single()
                .map(|dt| dt.with_timezone(&Utc))
                .ok_or_else(|| format!("ambiguous or invalid local time {naive} (DST boundary?)")),
            ResolveZone::Iana(tz) => tz
                .from_local_datetime(&naive)
                .single()
                .map(|dt| dt.with_timezone(&Utc))
                .ok_or_else(|| {
                    format!("ambiguous or invalid time {naive} in {tz:?} (DST boundary?)")
                }),
        }
    }
}

#[async_trait]
impl Tool for ResolveTimeTool {
    fn name(&self) -> &str {
        "resolve_time"
    }

    fn description(&self) -> &str {
        "Turn a time phrase into exact timestamps. Accepts 'last 24 hours', \
         'in 10 minutes', 'since Monday', 'tomorrow at 9am', or an ISO date/time. \
         Returns Unix seconds, milliseconds, Slack timestamp, and RFC-3339."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "expr": {
                    "type": "string",
                    "description": "Time phrase or ISO date/time, e.g. 'tomorrow at 9am'."
                },
                "timezone": {
                    "type": "string",
                    "description": "IANA timezone for dates without an offset; defaults to local."
                }
            },
            "required": ["expr"]
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn supports_markdown(&self) -> bool {
        true
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        self.execute_with_options(args, ToolCallOptions::default())
            .await
    }

    async fn execute_with_options(
        &self,
        args: serde_json::Value,
        options: ToolCallOptions,
    ) -> anyhow::Result<ToolResult> {
        tracing::debug!(args = %args, "[resolve_time] execute start");

        let expr = match args.get("expr").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return Ok(ToolResult::error(
                    "resolve_time: `expr` is required (e.g. \"24h ago\", \
                     \"2026-06-09T19:12:00Z\", \"now\").",
                ));
            }
        };

        // Resolve the interpretation zone for civil inputs.
        let zone = match args.get("timezone").and_then(|v| v.as_str()) {
            Some(tz_name) if !tz_name.trim().is_empty() => match tz_name.trim().parse::<Tz>() {
                Ok(tz) => ResolveZone::Iana(tz),
                Err(_) => {
                    return Ok(ToolResult::error(format!(
                        "resolve_time: unknown IANA timezone '{}' — use names like \
                         'America/Los_Angeles'.",
                        tz_name.trim()
                    )));
                }
            },
            _ => ResolveZone::Local,
        };

        let dt = match resolve_expr(expr, zone) {
            Ok(dt) => dt,
            Err(e) => {
                tracing::debug!(expr = expr, error = %e, "[resolve_time] parse failed");
                return Ok(ToolResult::error(format!("resolve_time: {e}")));
            }
        };

        let unix_s = dt.timestamp();
        let unix_ms = dt.timestamp_millis();
        let slack_ts = format!("{unix_s}.000000");
        let rfc3339 = dt.to_rfc3339_opts(SecondsFormat::Secs, true);

        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("unix_s");
        let value = match format {
            "unix_ms" => unix_ms.to_string(),
            "slack_ts" => slack_ts.clone(),
            "rfc3339" => rfc3339.clone(),
            _ => unix_s.to_string(),
        };

        let payload = json!({
            "interpreted": expr,
            "value": value,
            "unix_s": unix_s,
            "unix_ms": unix_ms,
            "slack_ts": slack_ts,
            "rfc3339": rfc3339,
        });

        tracing::debug!("[resolve_time] resolved {expr:?} -> {rfc3339} (unix_s={unix_s})");
        let mut result = ToolResult::success(serde_json::to_string_pretty(&payload)?);
        if options.prefer_markdown {
            result.markdown_formatted = Some(format!(
                "- **interpreted**: {expr}\n- **value** ({format}): {value}\n- **unix_s**: \
                 {unix_s}\n- **unix_ms**: {unix_ms}\n- **slack_ts**: {slack_ts}\n- **rfc3339**: \
                 {rfc3339}\n"
            ));
        }
        Ok(result)
    }
}

#[cfg(test)]
#[path = "resolve_time_tests.rs"]
mod tests;
