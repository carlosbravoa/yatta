//! Merging two versions of one task.
//!
//! Sync is per file, and a file is one task, so a conflict is never "the list
//! disagrees" -- it is always "this task disagrees", which is small enough to
//! resolve by rule rather than by asking.
//!
//! Every field is merged three ways: if only one device changed it since the
//! common ancestor, that change wins outright and nothing is asked of anyone.
//! Only when *both* devices changed the same field differently does a policy
//! apply, and each one is chosen so that the failure mode is visible work
//! rather than silent loss:
//!
//! | field       | both changed it                                        |
//! |-------------|--------------------------------------------------------|
//! | status      | the furthest along wins (done > doing > todo)           |
//! | priority    | the more urgent wins                                    |
//! | due         | the earlier date wins; a date beats no date             |
//! | tags        | set merge: an addition sticks, so does a removal        |
//! | created     | the earlier date wins                                   |
//! | title       | this device's wins; the other stays in git history      |
//! | description | line-level three-way merge, conflict markers if it fails|
//!
//! A ranked winner can always be undone in one click -- unticking a task takes
//! a second -- whereas a dropped deadline is only noticed when it passes. That
//! asymmetry is the whole argument for these rules.

use crate::task::{parse_task, render_task, Priority, Status, Task};

/// Line-level three-way merge of two texts: `(ours, base, theirs)` in, merged
/// text and "did it have to leave markers" out. Injected rather than called
/// directly so the rules above can be tested without a git binary.
pub type TextMerge<'a> = &'a dyn Fn(&str, &str, &str) -> (String, bool);

pub struct Resolved {
    pub content: String,
    /// The body came back with conflict markers in it; a human has to look.
    pub conflicted: bool,
    /// Both sides renamed the task and this device's name was kept.
    pub renamed: bool,
}

/// Pick a field's value. `base` is the common ancestor, absent when the two
/// devices created the file independently.
fn pick<T: PartialEq + Clone>(
    base: Option<&T>,
    ours: &T,
    theirs: &T,
    tie: impl Fn(&T, &T) -> T,
) -> T {
    if ours == theirs {
        return ours.clone();
    }
    if let Some(base) = base {
        if base == ours {
            return theirs.clone();
        }
        if base == theirs {
            return ours.clone();
        }
    }
    tie(ours, theirs)
}

fn status_rank(s: Status) -> u8 {
    match s {
        Status::Todo => 0,
        Status::Doing => 1,
        Status::Done => 2,
    }
}

fn priority_rank(p: Priority) -> u8 {
    match p {
        Priority::Urgent => 0,
        Priority::High => 1,
        Priority::Medium => 2,
        Priority::Low => 3,
        Priority::None => 4,
    }
}

/// Set merge. A tag added on either side is kept; a tag deleted on either side
/// stays deleted. Without an ancestor there is nothing to have deleted, so it
/// degrades to a union.
fn merge_tags(base: Option<&Vec<String>>, ours: &[String], theirs: &[String]) -> Vec<String> {
    let has = |list: &[String], tag: &String| list.iter().any(|t| t == tag);
    let empty = Vec::new();
    let base = base.unwrap_or(&empty);

    let mut out: Vec<String> = Vec::new();
    for tag in ours.iter().chain(theirs.iter()) {
        if out.iter().any(|t| t == tag) {
            continue;
        }
        let in_ours = has(ours, tag);
        let in_theirs = has(theirs, tag);
        let in_base = has(base, tag);
        // Kept when both still have it, or when one side has newly added it.
        let keep = (in_ours && in_theirs) || (in_ours && !in_base) || (in_theirs && !in_base);
        if keep {
            out.push(tag.clone());
        }
    }
    out
}

/// The earlier of two `YYYY-MM-DD` deadlines, treating "no deadline" as later
/// than any deadline: a date one device bothered to set survives the other
/// device clearing it.
fn earlier(a: &Option<String>, b: &Option<String>) -> Option<String> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if a <= b { a.clone() } else { b.clone() }),
        (Some(a), None) => Some(a.clone()),
        (None, Some(b)) => Some(b.clone()),
        (None, None) => None,
    }
}

pub fn merge_task(
    path: &str,
    base: Option<&str>,
    ours: &str,
    theirs: &str,
    text_merge: TextMerge,
) -> Resolved {
    let base_task: Option<Task> = base.map(|c| parse_task(c, path));
    let ours_task = parse_task(ours, path);
    let theirs_task = parse_task(theirs, path);
    let b = base_task.as_ref();

    let status = pick(
        b.map(|t| &t.status),
        &ours_task.status,
        &theirs_task.status,
        |a, b| if status_rank(*a) >= status_rank(*b) { *a } else { *b },
    );

    let priority = pick(
        b.map(|t| &t.priority),
        &ours_task.priority,
        &theirs_task.priority,
        |a, b| if priority_rank(*a) <= priority_rank(*b) { *a } else { *b },
    );

    let due = pick(b.map(|t| &t.due), &ours_task.due, &theirs_task.due, earlier);
    let created = pick(
        b.map(|t| &t.created),
        &ours_task.created,
        &theirs_task.created,
        |a, b| if a <= b { a.clone() } else { b.clone() },
    );
    let title = pick(
        b.map(|t| &t.title),
        &ours_task.title,
        &theirs_task.title,
        |a, _| a.clone(),
    );
    let renamed = title != theirs_task.title && ours_task.title != theirs_task.title;

    let tags = merge_tags(b.map(|t| &t.tags), &ours_task.tags, &theirs_task.tags);

    // The completion date belongs to the status it records. Anything else
    // leaves a task that is "todo, completed 2026-09-18".
    let completed = match status {
        Status::Done => earlier(&ours_task.completed, &theirs_task.completed)
            .or_else(|| b.and_then(|t| t.completed.clone())),
        _ => None,
    };

    let (description, conflicted) = if ours_task.description == theirs_task.description {
        (ours_task.description.clone(), false)
    } else {
        match b.map(|t| &t.description) {
            Some(base_text) if *base_text == ours_task.description => {
                (theirs_task.description.clone(), false)
            }
            Some(base_text) if *base_text == theirs_task.description => {
                (ours_task.description.clone(), false)
            }
            other => {
                // Trailing newlines matter to a line-based merge: without one,
                // the last line of each side looks changed to every other.
                let line = |s: &str| {
                    if s.is_empty() || s.ends_with('\n') {
                        s.to_string()
                    } else {
                        format!("{s}\n")
                    }
                };
                let (merged, conflicted) = text_merge(
                    &line(&ours_task.description),
                    &line(other.map(String::as_str).unwrap_or("")),
                    &line(&theirs_task.description),
                );
                (merged.trim_end().to_string(), conflicted)
            }
        }
    };

    let merged = Task {
        // The identity is the ancestor's, so a merge never renames a task out
        // from under a link or an agent holding its id.
        id: b.map(|t| t.id.clone()).unwrap_or_else(|| ours_task.id.clone()),
        title,
        status,
        priority,
        due,
        tags,
        created,
        completed,
        description,
        path: path.to_string(),
        adopted: false,
        archived: ours_task.archived,
        conflicted,
    };

    Resolved {
        content: render_task(&merged),
        conflicted,
        renamed,
    }
}

// ---------------------------------------------------------------------------
// Conflict markers
// ---------------------------------------------------------------------------

const OURS: &str = "<<<<<<<";
const BASE: &str = "|||||||";
const SPLIT: &str = "=======";
const THEIRS: &str = ">>>>>>>";

/// Whether a task body still carries an unresolved conflict. Derived from the
/// text rather than stored in frontmatter, so it stays true across restarts,
/// across devices, and for a conflict resolved by hand in a text editor.
pub fn has_markers(body: &str) -> bool {
    let mut opened = false;
    for line in body.lines() {
        if line.starts_with(OURS) {
            opened = true;
        } else if opened && line.starts_with(THEIRS) {
            return true;
        }
    }
    false
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Ours,
    Theirs,
    Both,
}

impl Side {
    pub fn parse(s: &str) -> Side {
        match s.trim().to_ascii_lowercase().as_str() {
            "theirs" | "other" | "remote" => Side::Theirs,
            "both" => Side::Both,
            _ => Side::Ours,
        }
    }
}

/// Strip conflict markers, keeping one side -- or both, in order, for the
/// common case where the two edits are complementary rather than rival.
pub fn resolve_markers(body: &str, keep: Side) -> String {
    #[derive(PartialEq)]
    enum At {
        Text,
        Ours,
        Base,
        Theirs,
    }

    let mut at = At::Text;
    let mut out: Vec<&str> = Vec::new();

    for line in body.lines() {
        if line.starts_with(OURS) {
            at = At::Ours;
            continue;
        }
        if at != At::Text {
            if line.starts_with(BASE) {
                at = At::Base;
                continue;
            }
            if line.starts_with(SPLIT) {
                at = At::Theirs;
                continue;
            }
            if line.starts_with(THEIRS) {
                at = At::Text;
                continue;
            }
        }
        let keep_line = match at {
            At::Text => true,
            At::Ours => keep != Side::Theirs,
            At::Theirs => keep != Side::Ours,
            At::Base => false,
        };
        if keep_line {
            out.push(line);
        }
    }

    let mut text = out.join("\n");
    text.push('\n');
    text.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stands in for `git merge-file`: enough to exercise the rules without
    /// shelling out. Anything it cannot decide comes back marked.
    fn stub(ours: &str, base: &str, theirs: &str) -> (String, bool) {
        if ours == base {
            return (theirs.to_string(), false);
        }
        if theirs == base {
            return (ours.to_string(), false);
        }
        (
            format!("<<<<<<< here\n{ours}=======\n{theirs}>>>>>>> there\n"),
            true,
        )
    }

    fn task(fm: &str, body: &str) -> String {
        format!("---\nid: abc123\n{fm}---\n\n{body}\n")
    }

    #[test]
    fn a_change_on_one_side_only_is_taken_without_comment() {
        let base = task("title: Report\nstatus: todo\npriority: low\n", "Draft it.");
        let ours = task("title: Report\nstatus: todo\npriority: low\n", "Draft it.");
        let theirs = task("title: Report\nstatus: doing\npriority: low\n", "Draft it.");

        let r = merge_task("report.md", Some(&base), &ours, &theirs, &stub);
        assert!(!r.conflicted);
        let merged = parse_task(&r.content, "report.md");
        assert_eq!(merged.status, Status::Doing);
    }

    #[test]
    fn rival_frontmatter_edits_resolve_by_rank() {
        let base = task("title: Report\nstatus: todo\npriority: low\ndue: 2026-10-01\ntags: [work]\n", "Draft it.");
        let ours = task("title: Report\nstatus: done\ncompleted: 2026-09-17\npriority: low\ndue: 2026-10-05\ntags: [work, q3]\n", "Draft it.");
        let theirs = task("title: Report\nstatus: doing\npriority: urgent\ndue: 2026-09-28\ntags: [work]\n", "Draft it.");

        let r = merge_task("report.md", Some(&base), &ours, &theirs, &stub);
        let merged = parse_task(&r.content, "report.md");

        assert!(!r.conflicted, "frontmatter alone never needs a human");
        assert_eq!(merged.status, Status::Done, "furthest along wins");
        assert_eq!(merged.priority, Priority::Urgent, "most urgent wins");
        assert_eq!(merged.due.as_deref(), Some("2026-09-28"), "earliest deadline wins");
        assert_eq!(merged.tags, vec!["work", "q3"], "an added tag survives");
        assert_eq!(
            merged.completed.as_deref(),
            Some("2026-09-17"),
            "the completion date follows the status that won"
        );
    }

    #[test]
    fn a_completion_date_does_not_outlive_the_status_it_records() {
        // One device ticked the task off, the other pushed it back to doing
        // *and* moved the deadline, so neither side is simply the ancestor.
        let base = task("title: Report\nstatus: todo\n", "Draft it.");
        let ours = task("title: Report\nstatus: doing\ndue: 2026-10-01\n", "Draft it.");
        let theirs = task("title: Report\nstatus: doing\ncompleted: 2026-09-17\n", "Draft it.");

        let r = merge_task("report.md", Some(&base), &ours, &theirs, &stub);
        let merged = parse_task(&r.content, "report.md");
        assert_eq!(merged.status, Status::Doing);
        assert_eq!(merged.completed, None, "not done, so not completed");
    }

    #[test]
    fn a_tag_removed_on_one_side_stays_removed() {
        let base = task("title: Report\ntags: [work, urgent]\n", "Draft it.");
        let ours = task("title: Report\ntags: [work]\n", "Draft it.");
        let theirs = task("title: Report\ntags: [work, urgent, q3]\n", "Draft it.");

        let r = merge_task("report.md", Some(&base), &ours, &theirs, &stub);
        let merged = parse_task(&r.content, "report.md");
        assert_eq!(merged.tags, vec!["work", "q3"]);
    }

    #[test]
    fn rival_notes_come_back_marked() {
        let base = task("title: Report\n", "Draft it.");
        let ours = task("title: Report\n", "Draft it with Ana.");
        let theirs = task("title: Report\n", "Draft it after the numbers land.");

        let r = merge_task("report.md", Some(&base), &ours, &theirs, &stub);
        assert!(r.conflicted);
        let merged = parse_task(&r.content, "report.md");
        assert!(merged.conflicted, "the flag is derived from the file itself");
        assert!(merged.description.contains("Draft it with Ana."));
        assert!(merged.description.contains("Draft it after the numbers land."));
    }

    #[test]
    fn two_devices_that_never_shared_an_ancestor_still_merge() {
        let ours = task("title: Report\nstatus: done\n", "Draft it.");
        let theirs = task("title: Report\nstatus: todo\n", "Draft it.");
        let r = merge_task("report.md", None, &ours, &theirs, &stub);
        assert!(!r.conflicted);
        assert_eq!(parse_task(&r.content, "report.md").status, Status::Done);
    }

    #[test]
    fn keeping_one_side_strips_the_markers() {
        let body = "Shared line.\n<<<<<<< here\nMine.\n=======\nTheirs.\n>>>>>>> there\nTail.";
        assert_eq!(
            resolve_markers(body, Side::Ours),
            "Shared line.\nMine.\nTail."
        );
        assert_eq!(
            resolve_markers(body, Side::Theirs),
            "Shared line.\nTheirs.\nTail."
        );
        assert_eq!(
            resolve_markers(body, Side::Both),
            "Shared line.\nMine.\nTheirs.\nTail."
        );
        assert!(!has_markers(&resolve_markers(body, Side::Both)));
    }

    #[test]
    fn a_diff3_ancestor_block_is_dropped_with_the_markers() {
        let body = "<<<<<<< here\nMine.\n||||||| common ancestor\nOld.\n=======\nTheirs.\n>>>>>>> there";
        assert_eq!(resolve_markers(body, Side::Both), "Mine.\nTheirs.");
    }
}
