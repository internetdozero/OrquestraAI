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
    // `2026-10-19-99` still yields a real day if the tail is dropped.
    if date_parts.next().is_some() {
        return None;
    }

    let (time, offset) = split_offset(rest)?;
    let mut time_parts = time.split(':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    // Seconds may be absent (`12:30Z`). A present field that is not an integer
    // used to become 0, so `12:30:45abc` was stored as `12:30:00`.
    let seconds: i64 = match time_parts.next() {
        None => 0,
        Some(field) => whole_seconds(field)?,
    };
    if time_parts.next().is_some() {
        return None;
    }

    // Hinnant's conversion normalizes month 13 and 31 February into a later
    // real day. An age purge would then treat that invented instant as known.
    let max_day = days_in_month(year, month)?;
    if !(1..=max_day).contains(&day)
        || !(0..24).contains(&hour)
        || !(0..60).contains(&minute)
        || !(0..60).contains(&seconds)
    {
        return None;
    }

    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + seconds - offset)
}

/// Integer seconds plus an optional all-digit fraction. The fraction is not
/// rounded; it only has to be numeric so `45.12a` cannot pass as `45`.
fn whole_seconds(field: &str) -> Option<i64> {
    let mut parts = field.split('.');
    let whole = parts.next().unwrap_or("");
    if let Some(fraction) = parts.next() {
        if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
    }
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    whole.parse().ok()
}

fn days_in_month(year: i64, month: i64) -> Option<i64> {
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => return None,
    };
    Some(days)
}

fn is_leap(year: i64) -> bool {
    let year = year.rem_euclid(400);
    year % 4 == 0 && (year % 100 != 0 || year == 0)
}

/// Split the timezone suffix off a time-of-day string.
fn split_offset(rest: &str) -> Option<(&str, i64)> {
    if let Some(stripped) = rest.strip_suffix(['Z', 'z']) {
        return Some((stripped, 0));
    }

    for (index, ch) in rest.char_indices() {
        if (ch == '+' || ch == '-') && index > 0 {
            let (time, zone) = rest.split_at(index);
            let magnitude = offset_secs(&zone[1..])?;
            // West of UTC must be added back. Dropping the sign made
            // `-03:00` look like `+03:00` and shifted the session by twice
            // the offset.
            let signed = if ch == '-' { -magnitude } else { magnitude };
            return Some((time, signed));
        }
    }

    Some((rest, 0))
}

/// `±HHMM` / `±HH:MM`. Hours above 14 or minutes above 59 are not a zone.
fn offset_secs(zone: &str) -> Option<i64> {
    let (hours, minutes) = if let Some((hours, minutes)) = zone.split_once(':') {
        if hours.len() != 2
            || minutes.len() != 2
            || !hours.chars().all(|ch| ch.is_ascii_digit())
            || !minutes.chars().all(|ch| ch.is_ascii_digit())
        {
            return None;
        }
        (hours.parse::<i64>().ok()?, minutes.parse::<i64>().ok()?)
    } else if zone.len() == 4 && zone.chars().all(|ch| ch.is_ascii_digit()) {
        (
            zone[..2].parse::<i64>().ok()?,
            zone[2..].parse::<i64>().ok()?,
        )
    } else {
        return None;
    };
    if !(0..=14).contains(&hours) || !(0..60).contains(&minutes) {
        return None;
    }
    Some(hours * 3_600 + minutes * 60)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impossible_calendar_fields_are_not_a_timestamp() {
        for raw in [
            "2026-13-01T00:00:00Z",
            "2026-00-01T00:00:00Z",
            "2026-02-31T00:00:00Z",
            "2026-04-31T00:00:00Z",
            "2025-02-29T00:00:00Z",
            "2026-10-19T24:00:00Z",
            "2026-10-19T23:60:00Z",
            "2026-10-19T23:59:60Z",
        ] {
            assert_eq!(timestamp_secs(raw), None, "{raw}");
            assert_eq!(normalized_timestamp(raw), raw);
        }
        assert!(timestamp_secs("2024-02-29T00:00:00Z").is_some());
        assert!(timestamp_secs("2026-10-19T23:59:59Z").is_some());
    }

    #[test]
    fn impossible_utc_offsets_are_not_a_timestamp() {
        for raw in [
            "2026-10-19T00:00:00+99:99",
            "2026-10-19T00:00:00-99:00",
            "2026-10-19T00:00:00+14:60",
            "2026-10-19T00:00:00+15:00",
        ] {
            assert_eq!(timestamp_secs(raw), None, "{raw}");
            assert_eq!(normalized_timestamp(raw), raw);
        }
        assert_eq!(
            timestamp_secs("2026-08-18T12:11:33-03:00"),
            timestamp_secs("2026-08-18T15:11:33Z")
        );
        assert!(timestamp_secs("2026-10-19T00:00:00+14:00").is_some());
        assert_eq!(
            timestamp_secs("2026-10-19T00:00:00+1400"),
            timestamp_secs("2026-10-19T00:00:00+14:00")
        );
    }

    #[test]
    fn offset_letters_are_not_digits() {
        assert_eq!(timestamp_secs("2026-10-19T00:00:00+0a3:00"), None);
        assert_eq!(normalized_timestamp("2026-10-19T00:00:00+0a3:00"), "2026-10-19T00:00:00+0a3:00");
    }

    #[test]
    fn trailing_garbage_is_not_a_timestamp() {
        for raw in [
            "2026-10-19T12:30:45abc",
            "2026-10-19T12:30:45abcZ",
            "2026-10-19T00:00:00Zjunk",
            "2026-10-19T00:00:00.12aZ",
            "2026-10-19-99T00:00:00Z",
            "2026-10-19T12:30:45:01Z",
        ] {
            assert_eq!(timestamp_secs(raw), None, "{raw}");
            assert_eq!(normalized_timestamp(raw), raw);
        }
        assert_eq!(
            timestamp_secs("2026-10-19T12:30Z"),
            timestamp_secs("2026-10-19T12:30:00Z")
        );
        assert_eq!(
            timestamp_secs("2026-10-19T00:00:00.123Z"),
            timestamp_secs("2026-10-19T00:00:00Z")
        );
    }
}
