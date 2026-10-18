//! Purge coordination: what *would* be deleted, and then the deleting.
//!
//! Selection is one shared pass over the merged catalog, so the estimate and
//! the execution cannot disagree about which sessions they looked at. Two
//! guards survive that pass: a pin, which the operator set deliberately, and a
//! session a running agent still holds open. Neither is ever deleted by
//! accident, and both are counted so the operator can see what was left.

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

use super::adapters;
use super::adapters::time::{now_secs, timestamp_secs};
use super::policy;
use super::types::{PinnedSession, PurgeEstimate, PurgeFilter, PurgeReport, UnifiedSession};
use crate::paths::OrquestraPaths;

#[cfg(test)]
mod tests;

/// Where a purge reads and writes.
///
/// Both roots are explicit so a test can point a purge at a fixture home
/// instead of the operator's real stores.
#[derive(Debug, Clone)]
pub struct Store {
    pub home: PathBuf,
    /// The `~/.orquestra` root holding `sessions_policy.json`.
    pub root: PathBuf,
}

impl Store {
    pub fn system() -> Result<Self, String> {
        Ok(Self {
            home: adapters::home()?,
            root: OrquestraPaths::resolve()?.root,
        })
    }
}

/// One session that passed every guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub harness: String,
    pub session_id: String,
    pub size_bytes: u64,
}

/// What a filter selected, and why everything else stayed.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub candidates: Vec<Candidate>,
    pub skipped_pinned: usize,
    pub skipped_active: usize,
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Dry run: what this filter would delete, in sessions and bytes.
pub fn estimate(store: &Store, filter: &PurgeFilter) -> Result<PurgeEstimate, String> {
    let selection = select(store, filter)?;

    Ok(PurgeEstimate {
        sessions_count: selection.candidates.len(),
        reclaimed_bytes: selection.candidates.iter().map(|c| c.size_bytes).sum(),
        affected_harnesses: selection
            .candidates
            .iter()
            .map(|candidate| candidate.harness.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        skipped_pinned_count: selection.skipped_pinned,
        skipped_active_count: selection.skipped_active,
    })
}

/// Delete everything the filter matches, then reclaim what the deletions left
/// in each affected database.
pub fn execute(store: &Store, filter: &PurgeFilter) -> Result<PurgeReport, String> {
    execute_selection(store, select(store, filter)?)
}

/// Candidates for per-harness retention rules, in a single listing pass.
///
/// The scheduler needs one TTL per harness, which a single [`PurgeFilter`]
/// cannot express; running nine filtered purges instead would rescan every
/// store nine times.
pub fn select_by_ttl(store: &Store, ttl_days: &dyn Fn(&str) -> u32) -> Result<Selection, String> {
    let now = now_secs();
    let cutoff = move |harness: &str| now - (ttl_days(harness) as i64) * 86_400;

    select_with(
        store,
        &Rules {
            cutoff_for: Some(&cutoff),
            // A scheduled purge is the last place a pin may be ignored: the
            // operator is not watching it happen.
            ignore_pinned: true,
            ..Rules::default()
        },
    )
}

/// Delete a selection, then record the run in the policy sidecar so the
/// scheduler can compute when it is next due.
pub fn execute_selection(store: &Store, selection: Selection) -> Result<PurgeReport, String> {
    let mut adapters = adapters::adapters_by_id(&store.home);
    let mut report = PurgeReport::default();
    let mut touched: BTreeSet<String> = BTreeSet::new();

    for candidate in selection.candidates {
        let Some(adapter) = adapters.get_mut(candidate.harness.as_str()) else {
            report.failed_count += 1;
            report
                .errors
                .push(format!("{}: no adapter registered", candidate.harness));
            continue;
        };

        match adapter.delete_session(&candidate.session_id) {
            Ok(bytes) => {
                report.deleted_count += 1;
                report.reclaimed_bytes += bytes;
                touched.insert(candidate.harness);
            }
            Err(error) => {
                report.failed_count += 1;
                report.errors.push(format!(
                    "{}/{}: {error}",
                    candidate.harness, candidate.session_id
                ));
            }
        }
    }

    // Reclaiming space runs once per harness rather than once per session, and
    // a database that is busy right now costs a line in `errors` — never a
    // failed deletion, because the rows are already gone.
    for harness in touched {
        let Some(adapter) = adapters.get(harness.as_str()) else {
            continue;
        };

        if let Err(error) = adapter.optimize_storage() {
            report.errors.push(format!("{harness}: {error}"));
        }
    }

    policy::mark_last_run_at(&store.root, policy::now_iso8601())?;

    Ok(report)
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

/// The decision rules a selection applies, in the order they matter.
#[derive(Default)]
struct Rules<'a> {
    harness: Option<&'a str>,
    session_ids: Option<&'a [String]>,
    /// When set, a session must be at least this old. The cutoff is per
    /// harness so the scheduler's overrides work in one pass.
    cutoff_for: Option<&'a dyn Fn(&str) -> i64>,
    ignore_pinned: bool,
}

fn select(store: &Store, filter: &PurgeFilter) -> Result<Selection, String> {
    let age_limit: Option<Box<dyn Fn(&str) -> i64>> = filter.older_than_days.map(|days| {
        let now = now_secs();
        Box::new(move |_harness: &str| now - (days as i64) * 86_400) as Box<dyn Fn(&str) -> i64>
    });

    select_with(
        store,
        &Rules {
            harness: filter.harness.as_deref(),
            session_ids: filter.session_ids.as_deref(),
            cutoff_for: age_limit.as_deref(),
            ignore_pinned: filter.ignore_pinned,
        },
    )
}

fn select_with(store: &Store, rules: &Rules<'_>) -> Result<Selection, String> {
    let (sessions, warnings) = adapters::collect(&store.home);
    for warning in warnings {
        crate::log_warn!("sessions: skipped {warning}");
    }

    let pins: HashSet<(String, String)> = policy::load_from(&store.root)?
        .pinned_sessions
        .iter()
        .map(pin_key)
        .collect();
    let adapters = adapters::adapters_by_id(&store.home);
    let mut selection = Selection::default();

    for session in sessions {
        if !matches_identity(&session, rules) {
            continue;
        }
        if !matches_age(&session, rules.cutoff_for) {
            continue;
        }

        if pins.contains(&(session.harness.clone(), session.id.clone())) && rules.ignore_pinned {
            selection.skipped_pinned += 1;
            continue;
        }

        if adapters
            .get(session.harness.as_str())
            .is_some_and(|adapter| adapter.is_session_active(&session.id))
        {
            selection.skipped_active += 1;
            continue;
        }

        selection.candidates.push(Candidate {
            harness: session.harness,
            session_id: session.id,
            size_bytes: session.size_bytes,
        });
    }

    Ok(selection)
}

fn matches_identity(session: &UnifiedSession, rules: &Rules<'_>) -> bool {
    if rules
        .harness
        .is_some_and(|expected| expected != session.harness)
    {
        return false;
    }

    match rules.session_ids {
        Some(ids) => ids.contains(&session.id),
        None => true,
    }
}

fn matches_age(session: &UnifiedSession, cutoff_for: Option<&dyn Fn(&str) -> i64>) -> bool {
    let Some(cutoff_for) = cutoff_for else {
        return true;
    };

    // A session with no readable timestamp has no known age, and guessing
    // "old enough" is how an age-based purge deletes the wrong conversation.
    timestamp_secs(&session.updated_at).is_some_and(|at| at <= cutoff_for(&session.harness))
}

fn pin_key(pin: &PinnedSession) -> (String, String) {
    (pin.harness.clone(), pin.session_id.clone())
}
