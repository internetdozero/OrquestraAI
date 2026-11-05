//! Human-readable formatters for ideas and blackboard notes.

use crate::ideas::productivity::ProductivityMetrics;
use crate::ideas::types::{BlackboardNote, IdeaItem};

pub(super) fn render_ideas_table(ideas: &[IdeaItem]) -> String {
    if ideas.is_empty() {
        return "No matching ideas found.".to_string();
    }
    let mut lines = Vec::with_capacity(ideas.len() + 1);
    lines.push(format!(
        "{:<6} {:<10} {:<12} {:<16} {}",
        "ID", "PRIORITY", "STATUS", "PROJECT", "TITLE"
    ));
    for idea in ideas {
        lines.push(format!(
            "#{:<5} {:<10} {:<12} {:<16} {}",
            idea.id, idea.priority, idea.status, idea.project, idea.title
        ));
    }
    lines.join("\n")
}

pub(super) fn render_idea_detail(idea: &IdeaItem) -> String {
    let mut out = format!(
        "ID:          #{}\n\
         Project:     {}\n\
         Title:       {}\n\
         Category:    {}\n\
         Priority:    {}\n\
         Status:      {}\n\
         Created:     {}\n\
         Updated:     {}",
        idea.id,
        idea.project,
        idea.title,
        idea.category,
        idea.priority,
        idea.status,
        idea.created_at,
        idea.updated_at
    );
    if let Some(completed) = &idea.completed_at {
        out.push_str(&format!("\nCompleted:   {completed}"));
    }
    if let Some(tags) = &idea.tags {
        out.push_str(&format!("\nTags:        {tags}"));
    }
    if let Some(desc) = &idea.description {
        out.push_str(&format!("\nDescription:\n  {desc}"));
    }
    if let Some(prompt) = &idea.prompt {
        out.push_str(&format!("\nBuilder Prompt:\n{prompt}"));
    }
    out
}

pub(super) fn render_notes_table(notes: &[BlackboardNote]) -> String {
    if notes.is_empty() {
        return "No blackboard notes found.".to_string();
    }
    let mut lines = Vec::with_capacity(notes.len() + 1);
    lines.push(format!(
        "{:<6} {:<12} {:<14} {:<10} {:<6} {}",
        "ID", "PROJECT", "TOPIC", "SENDER", "READ", "NOTE"
    ));
    for note in notes {
        let read_str = if note.is_read { "yes" } else { "no" };
        let snippet = if note.note.len() > 50 {
            format!("{}...", &note.note[..47])
        } else {
            note.note.clone()
        };
        lines.push(format!(
            "#{:<5} {:<12} {:<14} {:<10} {:<6} {}",
            note.id, note.project, note.topic, note.sender, read_str, snippet
        ));
    }
    lines.join("\n")
}

pub(super) fn render_note_detail(note: &BlackboardNote) -> String {
    format!(
        "Note #{}:\n\
         Project:  {}\n\
         Topic:    {}\n\
         Sender:   {}\n\
         Target:   {}\n\
         Is Read:  {}\n\
         Created:  {}\n\
         Payload:\n  {}",
        note.id,
        note.project,
        note.topic,
        note.sender,
        note.target,
        if note.is_read { "yes" } else { "no" },
        note.created_at,
        note.note
    )
}

pub(super) fn render_metrics(metrics: &ProductivityMetrics) -> String {
    let project_str = metrics.project.as_deref().unwrap_or("all projects");
    let total_in_window: usize = metrics
        .daily_activity
        .iter()
        .map(|d| d.ideas_completed)
        .sum();
    let avg_cycle_str = match metrics.avg_cycle_hours {
        Some(hours) => format!("{hours:.1}h"),
        None => "n/a".to_string(),
    };
    format!(
        "Productivity Metrics ({project_str}):\n  \
         Completed in Window: {}\n  \
         Current Streak:      {} days\n  \
         Weekly Velocity:     {} (this week) vs {} (last week)\n  \
         Avg Cycle Time:      {}",
        total_in_window,
        metrics.current_streak,
        metrics.weekly_velocity.this_week,
        metrics.weekly_velocity.last_week,
        avg_cycle_str
    )
}
