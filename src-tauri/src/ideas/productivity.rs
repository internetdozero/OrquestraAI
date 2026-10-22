//! Developer velocity from Idea Vault deliveries.
//!
//! One IPC call returns everything the heatmap sub-panel paints: a trailing
//! window of daily activity, the current streak, week-over-week velocity and
//! the average delivery cycle. Everything comes from the SQLite `ideas`
//! table (`completed_at`); no process is ever spawned, so the panel can
//! never wedge on a slow checkout.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde::Serialize;

const DEFAULT_WEEKS: u32 = 16;
const MAX_WEEKS: u32 = 52;

/// One heatmap cell: vault deliveries for a calendar day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayActivity {
    pub date: String,
    pub ideas_completed: usize,
    pub intensity: u8,
}

/// Ideas completed in the trailing 7 days versus the 7 before them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyVelocity {
    pub this_week: usize,
    pub last_week: usize,
}

/// Everything the productivity sub-panel renders in one round trip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityMetrics {
    pub daily_activity: Vec<DayActivity>,
    pub current_streak: usize,
    pub weekly_velocity: WeeklyVelocity,
    pub avg_cycle_hours: Option<f64>,
    pub project: Option<String>,
}

fn today_days() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|age| (age.as_secs() / 86_400) as i64)
        .unwrap_or(0)
}

/// Days since the Unix epoch to a civil `YYYY-MM-DD` (Hinnant's algorithm,
/// so no date crate is needed for a trailing window).
fn day_string(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = (if month <= 2 { year + 1 } else { year }) as u32;
    format!("{year:04}-{month:02}-{day:02}")
}

/// Heatmap level straight from the delivery count: 0 none, 1 quiet, 2
/// steady, 3 busy, 4+ peak.
fn intensity(ideas: usize) -> u8 {
    match ideas {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        _ => 4,
    }
}

fn count_on_or_after(
    connection: &Connection,
    project: Option<&str>,
    since: &str,
) -> Result<usize, String> {
    let (sql, param) = match project {
        Some(name) => (
            "SELECT COUNT(*) FROM ideas WHERE completed_at IS NOT NULL \
             AND date(completed_at) >= date(?1) AND project = ?2",
            Some(name),
        ),
        None => (
            "SELECT COUNT(*) FROM ideas WHERE completed_at IS NOT NULL \
             AND date(completed_at) >= date(?1)",
            None,
        ),
    };
    let mut statement = connection.prepare(sql).map_err(super::store::db_error)?;
    let count: i64 = if let Some(name) = param {
        statement.query_row([since, name], |row| row.get(0))
    } else {
        statement.query_row([since], |row| row.get(0))
    }
    .map_err(super::store::db_error)?;
    Ok(count.max(0) as usize)
}

/// Trailing-window velocity plus the vault daily series behind it.
pub fn metrics(
    connection: &Connection,
    project: Option<&str>,
    weeks: Option<u32>,
) -> Result<ProductivityMetrics, String> {
    let weeks = weeks.unwrap_or(DEFAULT_WEEKS).clamp(1, MAX_WEEKS);
    let days = weeks as i64 * 7;
    let today = today_days();
    let start = day_string(today - days + 1);

    let mut ideas_by_day: BTreeMap<String, usize> = BTreeMap::new();
    let (list_sql, project_param) = match project {
        Some(name) => (
            "SELECT date(completed_at) AS day, COUNT(*) FROM ideas \
             WHERE completed_at IS NOT NULL AND date(completed_at) >= date(?1) \
             AND project = ?2 GROUP BY day",
            Some(name),
        ),
        None => (
            "SELECT date(completed_at) AS day, COUNT(*) FROM ideas \
             WHERE completed_at IS NOT NULL AND date(completed_at) >= date(?1) \
             GROUP BY day",
            None,
        ),
    };
    {
        let mut statement = connection
            .prepare(list_sql)
            .map_err(super::store::db_error)?;
        let params: Vec<&str> = match project_param {
            Some(name) => vec![start.as_str(), name],
            None => vec![start.as_str()],
        };
        let rows = statement
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(super::store::db_error)?;
        for row in rows {
            let (day, count) = row.map_err(super::store::db_error)?;
            ideas_by_day.insert(day, count.max(0) as usize);
        }
    }

    let (cycle_sql, cycle_param) = match project {
        Some(name) => (
            "SELECT AVG((julianday(completed_at) - julianday(created_at)) * 24.0) \
             FROM ideas WHERE completed_at IS NOT NULL \
             AND date(completed_at) >= date(?1) AND project = ?2",
            Some(name),
        ),
        None => (
            "SELECT AVG((julianday(completed_at) - julianday(created_at)) * 24.0) \
             FROM ideas WHERE completed_at IS NOT NULL \
             AND date(completed_at) >= date(?1)",
            None,
        ),
    };
    let avg_cycle_hours: Option<f64> = {
        let mut statement = connection
            .prepare(cycle_sql)
            .map_err(super::store::db_error)?;
        if let Some(name) = cycle_param {
            statement
                .query_row([start.as_str(), name], |row| row.get(0))
                .map_err(super::store::db_error)?
        } else {
            statement
                .query_row([start.as_str()], |row| row.get(0))
                .map_err(super::store::db_error)?
        }
    };

    let mut daily_activity = Vec::with_capacity(days as usize);
    for offset in (0..days).rev() {
        let date = day_string(today - offset);
        let ideas = ideas_by_day.get(&date).copied().unwrap_or(0);
        daily_activity.push(DayActivity {
            date,
            ideas_completed: ideas,
            intensity: intensity(ideas),
        });
    }

    // Streak counts backwards from today; an empty today starts the count at
    // yesterday (GitHub convention) instead of reporting a dead streak.
    let mut cursor = daily_activity.len();
    if cursor > 0 {
        let last = &daily_activity[cursor - 1];
        if last.ideas_completed == 0 {
            cursor -= 1;
        }
    }
    let mut streak = 0;
    while cursor > 0 {
        let day = &daily_activity[cursor - 1];
        if day.ideas_completed == 0 {
            break;
        }
        streak += 1;
        cursor -= 1;
    }

    let week_edge = day_string(today - 6);
    let fortnight_edge = day_string(today - 13);
    let this_week = count_on_or_after(connection, project, &week_edge)?;
    let fortnight = count_on_or_after(connection, project, &fortnight_edge)?;
    let weekly_velocity = WeeklyVelocity {
        this_week,
        last_week: fortnight.saturating_sub(this_week),
    };

    Ok(ProductivityMetrics {
        daily_activity,
        current_streak: streak,
        weekly_velocity,
        avg_cycle_hours,
        project: project.map(str::to_string),
    })
}
