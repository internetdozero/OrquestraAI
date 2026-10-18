//! Timestamp normalization for every harness dialect.
//!
//! Harnesses disagree about time: Codex, Grok and Hermes store epoch seconds,
//! Claude's history stores epoch millis, Antigravity stores ISO text with a
//! nanosecond fraction and an offset, Copilot stores ISO millis. The wire
//! format is always ISO 8601 UTC, so everything funnels through
//! [`timestamp_secs`] and [`iso8601_from_secs`].
//!
//! The civil-date arithmetic is Howard Hinnant's `days_from_civil` and its
//! inverse, which is why a date crate is not needed.

/// Seconds since the Unix epoch.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// Render epoch seconds as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn iso8601_from_secs(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);

    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        (rem % 3_600) / 60,
        rem % 60
    )
}

/// Parse the timestamp dialects found in harness stores into epoch seconds.
///
/// Accepts bare numbers (seconds, millis or micros — picked by magnitude) and
/// ISO 8601 text with either a `T` or a space separator, optional fractional
/// seconds and an optional `Z` / `±HH:MM` suffix.
pub fn timestamp_secs(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    let numeric = raw
        .chars()
        .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+');
    if numeric {
        return raw.parse::<f64>().ok().map(scale_epoch);
    }

    parse_iso8601(raw)
}

/// Normalize any harness timestamp for the wire, passing unknown text through
/// untouched rather than inventing a time.
pub fn normalized_timestamp(raw: &str) -> String {
    match timestamp_secs(raw) {
        Some(secs) => iso8601_from_secs(secs),
        None => raw.trim().to_string(),
    }
}

/// 1e11 seconds is the year 5138, so a larger magnitude can only be a finer
/// unit. OpenCode and Claude's history store millis.
///
/// Shared with the JSON helpers, which meet the same numbers as strings.
pub(super) fn scale_epoch(value: f64) -> i64 {
    if value.abs() >= 1e14 {
        (value / 1e6) as i64
    } else if value.abs() >= 1e11 {
        (value / 1e3) as i64
    } else {
        value as i64
    }
}

fn parse_iso8601(raw: &str) -> Option<i64> {
    let (date, rest) = raw.split_once(['T', ' '])?;

    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: i64 = date_parts.next()?.parse().ok()?;
    let day: i64 = date_parts.next()?.parse().ok()?;

    let (time, offset) = split_offset(rest);
    let mut time_parts = time.split(':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    let seconds: i64 = time_parts
        .next()
        .and_then(|field| field.split('.').next())
        .and_then(|field| field.parse::<i64>().ok())
        .unwrap_or(0);

    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + seconds - offset)
}

/// Split the timezone suffix off a time-of-day string.
fn split_offset(rest: &str) -> (&str, i64) {
    if let Some(stripped) = rest.strip_suffix(['Z', 'z']) {
        return (stripped, 0);
    }

    for (index, ch) in rest.char_indices() {
        if (ch == '+' || ch == '-') && index > 0 {
            let (time, zone) = rest.split_at(index);
            return (time, offset_secs(&zone[1..]));
        }
    }

    (rest, 0)
}

fn offset_secs(zone: &str) -> i64 {
    let digits: String = zone.chars().filter(char::is_ascii_digit).collect();
    if digits.len() < 4 {
        return 0;
    }

    let hours: i64 = digits[..2].parse().unwrap_or(0);
    let minutes: i64 = digits[2..4].parse().unwrap_or(0);
    hours * 3_600 + minutes * 60
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_index = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    era * 146_097 + day_of_era - 719_468
}

/// Inverse of [`days_from_civil`].
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };

    (if month <= 2 { year + 1 } else { year }, month, day)
}
