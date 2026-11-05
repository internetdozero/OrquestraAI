//! SQL queries for Freebuff Desktop threads and cloud sync metadata.

use std::path::Path;

use rusqlite::Connection;

use super::super::utils;

pub struct ThreadRow {
    pub id: String,
    pub title: Option<String>,
    pub project_path: Option<String>,
    pub created_ms: Option<i64>,
    pub updated_ms: Option<i64>,
    pub turns: usize,
    pub message_bytes: u64,
    pub execution_target: Option<String>,
    #[allow(dead_code)]
    pub cloud_account_id: Option<String>,
}

/// One grouped query per database; discovered columns keep schema variations compatible.
pub fn query_threads(connection: &Connection, database: &Path) -> Result<Vec<ThreadRow>, String> {
    if !utils::table_exists(connection, "threads")? {
        return Ok(Vec::new());
    }
    let columns = utils::table_columns(connection, "threads");
    if !columns.contains(&"id".to_string()) {
        return Err(format!("{} has no `threads.id` column", database.display()));
    }

    let pick = |name: &str| {
        if columns.contains(&name.to_string()) {
            format!("t.{name}")
        } else {
            "NULL".to_string()
        }
    };

    let message_columns = if utils::table_exists(connection, "messages")? {
        utils::table_columns(connection, "messages")
    } else {
        Vec::new()
    };
    let joined = message_columns.contains(&"thread_id".to_string());
    let count_expr = if joined && message_columns.contains(&"seq".to_string()) {
        "COUNT(m.seq)".to_string()
    } else if joined {
        "COUNT(m.thread_id)".to_string()
    } else {
        "0".to_string()
    };
    let mut size_terms = Vec::new();
    for column in ["parts_json", "attachments_json"] {
        if joined && message_columns.contains(&column.to_string()) {
            size_terms.push(format!("COALESCE(SUM(LENGTH(m.{column})), 0)"));
        }
    }
    let size_expr = if size_terms.is_empty() {
        "0".to_string()
    } else {
        size_terms.join(" + ")
    };

    let mut activity_terms = Vec::new();
    if joined {
        if let Some(column) = ["ts", "timestamp"]
            .into_iter()
            .find(|column| message_columns.contains(&column.to_string()))
        {
            activity_terms.push(format!("MAX(m.{column})"));
        }
    }
    for column in [
        "last_turn_finished_at",
        "last_prompt_at",
        "created_at",
        "updated_at",
    ] {
        if columns.contains(&column.to_string()) {
            activity_terms.push(format!("t.{column}"));
        }
    }
    let activity_expr = match activity_terms.as_slice() {
        [] => "NULL".to_string(),
        [only] => only.clone(),
        _ => format!("COALESCE({})", activity_terms.join(", ")),
    };

    // Without a `messages` table, threads still list with zero turns.
    let join = if joined {
        "LEFT JOIN messages m ON m.thread_id = t.id"
    } else {
        ""
    };
    // Activity column 5 drives both the displayed session time and ordering.
    // Columns: 0:id, 1:title, 2:project_path, 3:created_at, 4:activity, 5:turns, 6:bytes, 7:execution_target, 8:cloud_account_id
    let sql = format!(
        "SELECT t.id, {}, {}, {}, {activity_expr}, {count_expr}, {size_expr}, {}, {} \
         FROM threads t {join} \
         GROUP BY t.id ORDER BY 5 DESC",
        pick("title"),
        pick("project_path"),
        pick("created_at"),
        pick("execution_target"),
        pick("cloud_account_id"),
    );

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("{}: {error}", database.display()))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                utils::text_cell(row, 0),
                utils::text_cell(row, 1),
                utils::text_cell(row, 2),
                utils::i64_cell(row, 3),
                utils::i64_cell(row, 4),
                utils::i64_cell(row, 5),
                utils::i64_cell(row, 6),
                utils::text_cell(row, 7),
                utils::text_cell(row, 8),
            ))
        })
        .map_err(|error| format!("{}: {error}", database.display()))?;

    let mut threads = Vec::new();
    for row in rows.flatten() {
        let (
            Some(id),
            title,
            project_path,
            created_ms,
            updated_ms,
            turns,
            message_bytes,
            execution_target,
            cloud_account_id,
        ) = row
        else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        threads.push(ThreadRow {
            id,
            title,
            project_path,
            created_ms,
            updated_ms,
            turns: turns.unwrap_or(0).max(0) as usize,
            message_bytes: message_bytes.unwrap_or(0).max(0) as u64,
            execution_target,
            cloud_account_id,
        });
    }

    Ok(threads)
}

/// Look up the execution_target of a thread directly when querying transcript.
pub fn thread_execution_target(connection: &Connection, thread_id: &str) -> Option<String> {
    if !utils::table_exists(connection, "threads").unwrap_or(false) {
        return None;
    }
    let columns = utils::table_columns(connection, "threads");
    if !columns.iter().any(|c| c == "execution_target") {
        return None;
    }
    let mut stmt = connection
        .prepare("SELECT execution_target FROM threads WHERE id = ?1")
        .ok()?;
    stmt.query_row([thread_id], |row| Ok(utils::text_cell(row, 0)))
        .ok()
        .flatten()
}
