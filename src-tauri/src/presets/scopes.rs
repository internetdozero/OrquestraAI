//! Scope-map helpers for presets: normalize and plan per-harness maps.
//!
//! Both `harness_skills` and `harness_mcps` share the same shape
//! (`harness_id -> ids`), so they share validation: drop unknown harness
//! scopes, sort and dedupe each list, and drop empties so stored JSON stays
//! compact. Planning filters each list against installed ids and warns once
//! per unknown id.

use std::collections::{BTreeSet, HashMap};

/// Drop unknown harness scopes, sort/dedupe every list, drop empties.
pub(super) fn normalize_scope_map(map: &HashMap<String, Vec<String>>) -> HashMap<String, Vec<String>> {
    let mut cleaned = HashMap::new();
    for (harness, ids) in map {
        if crate::harness::find(harness).is_none() {
            continue;
        }
        let mut sorted: Vec<String> = ids.iter().cloned().collect();
        sorted.sort();
        sorted.dedup();
        // Skills/MCP ids with path syntax can never resolve; drop them here
        // so a hand-edited preset cannot smuggle traversal into the engine.
        sorted.retain(|id| !id.is_empty() && !id.contains('/') && !id.contains('\\'));
        sorted.retain(|id| !id.contains('\0') && !id.chars().any(char::is_whitespace));
        if !sorted.is_empty() {
            cleaned.insert(harness.clone(), sorted);
        }
    }
    cleaned
}

/// Filter every scope list against `known` ids, warning once per gap.
pub(super) fn plan_scope_map(
    map: &HashMap<String, Vec<String>>,
    known: &[String],
    kind: &str,
) -> (HashMap<String, BTreeSet<String>>, Vec<String>) {
    let installed: BTreeSet<&str> = known.iter().map(String::as_str).collect();
    let mut planned: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut warnings = Vec::new();
    // Sort scopes so warnings are deterministic across runs.
    let mut scopes: Vec<&String> = map.keys().collect();
    scopes.sort();
    for harness in scopes {
        let mut wanted = BTreeSet::new();
        for id in &map[harness] {
            if installed.contains(id.as_str()) {
                wanted.insert(id.clone());
            } else if kind == "skill" {
                warnings.push(format!("skill “{id}” is not in the vault"));
            } else {
                warnings.push(format!("MCP server “{id}” is not in the catalog"));
            }
        }
        if !wanted.is_empty() {
            planned.insert(harness.clone(), wanted);
        }
    }
    (planned, warnings)
}

/// Union of the global set plus every per-harness set, for report counts.
pub(super) fn union_unique(
    global: &BTreeSet<String>,
    per_harness: &HashMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    let mut all = global.clone();
    for set in per_harness.values() {
        all.extend(set.iter().cloned());
    }
    all
}
