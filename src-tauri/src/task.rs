//! Task model plus the markdown <-> struct conversion.
//!
//! A task is one markdown file: YAML-ish frontmatter for the structured fields,
//! the body for the description. The parser is deliberately forgiving -- a file
//! with no frontmatter at all is still a valid task (an agent can drop in a
//! plain `.md` and it shows up), and missing fields fall back to defaults.

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Urgent,
    High,
    Medium,
    Low,
    None,
}

impl Priority {
    pub fn parse(s: &str) -> Self {
        match s.trim().trim_matches('!').to_ascii_lowercase().as_str() {
            "urgent" | "critical" | "p0" | "p1" | "1" => Priority::Urgent,
            "high" | "p2" | "2" => Priority::High,
            "medium" | "med" | "normal" | "p3" | "3" => Priority::Medium,
            "low" | "p4" | "4" => Priority::Low,
            _ => Priority::None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Priority::Urgent => "urgent",
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
            Priority::None => "none",
        }
    }
}

impl Default for Priority {
    fn default() -> Self {
        Priority::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Todo,
    Doing,
    Done,
}

impl Status {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "done" | "complete" | "completed" | "x" | "closed" => Status::Done,
            "doing" | "in-progress" | "in_progress" | "wip" | "started" => Status::Doing,
            _ => Status::Todo,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Todo => "todo",
            Status::Doing => "doing",
            Status::Done => "done",
        }
    }
}

impl Default for Status {
    fn default() -> Self {
        Status::Todo
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub priority: Priority,
    /// Plain `YYYY-MM-DD`. Deliberately a date, not a timestamp: deadlines are
    /// day-granular and timestamps would drag timezones into the file format.
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub completed: Option<String>,
    #[serde(default)]
    pub description: String,
    /// Path relative to the vault root. Empty for a task not yet written.
    #[serde(default)]
    pub path: String,
    /// True when the file had no `id:` in frontmatter, i.e. it was hand- or
    /// agent-written. The UI marks these so the user knows they'll be
    /// normalised on first save.
    #[serde(default)]
    pub adopted: bool,
    /// True when the file sits under `archive/`. Derived from the path and
    /// never stored in the file -- moving the file is what archives it.
    #[serde(default)]
    pub archived: bool,
}

impl Task {
    pub fn new(title: String) -> Self {
        Task {
            id: new_id(),
            title,
            status: Status::Todo,
            priority: Priority::None,
            due: None,
            tags: Vec::new(),
            created: Local::now().format("%Y-%m-%d").to_string(),
            completed: None,
            description: String::new(),
            path: String::new(),
            adopted: false,
            archived: false,
        }
    }
}

/// Short, sortable, URL-safe id: base36 millis + 4 random base36 chars.
pub fn new_id() -> String {
    let millis = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let mut h = RandomState::new().build_hasher();
    h.write_u64(millis);
    let rand = h.finish();
    format!("{}{}", to_base36(millis), &to_base36(rand)[..4])
}

fn to_base36(mut n: u64) -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0000".into();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(ALPHABET[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    // Guarantee the caller can always slice 4 chars off the front.
    while out.len() < 4 {
        out.push(b'0');
    }
    String::from_utf8(out).unwrap()
}

/// Filesystem-safe, human-readable filename stem derived from a title.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if ch.is_alphanumeric() {
            // Keep non-ASCII letters (accents, CJK) -- they're valid in filenames.
            out.extend(ch.to_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let s = out.trim_matches('-').to_string();
    let s: String = s.chars().take(60).collect();
    if s.is_empty() {
        "task".into()
    } else {
        s.trim_matches('-').to_string()
    }
}

// ---------------------------------------------------------------------------
// Frontmatter
// ---------------------------------------------------------------------------

enum FmValue {
    Scalar(String),
    List(Vec<String>),
}

/// Split a document into (frontmatter lines, body). Returns no frontmatter if
/// the file doesn't open with a `---` fence.
fn split_frontmatter(content: &str) -> (Vec<String>, String) {
    let normalized = content.replace("\r\n", "\n");
    let normalized = normalized.trim_start_matches('\u{feff}');
    let mut lines = normalized.split('\n');

    match lines.next() {
        Some(first) if first.trim() == "---" => {}
        _ => return (Vec::new(), normalized.to_string()),
    }

    let mut fm = Vec::new();
    let mut closed = false;
    for line in lines.by_ref() {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        fm.push(line.to_string());
    }

    if !closed {
        // Unterminated fence: treat the whole file as body rather than
        // silently swallowing the user's content.
        return (Vec::new(), normalized.to_string());
    }

    let body = lines.collect::<Vec<_>>().join("\n");
    (fm, body.trim_start_matches('\n').to_string())
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 {
        let b = s.as_bytes();
        if (b[0] == b'"' && b[s.len() - 1] == b'"') || (b[0] == b'\'' && b[s.len() - 1] == b'\'') {
            return s[1..s.len() - 1].replace("\\\"", "\"").replace("\\'", "'");
        }
    }
    s.to_string()
}

fn parse_frontmatter(lines: &[String]) -> Vec<(String, FmValue)> {
    let mut out: Vec<(String, FmValue)> = Vec::new();
    let mut pending_list_key: Option<String> = None;

    for raw in lines {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Continuation of a block list: "  - value"
        if let Some(item) = trimmed.strip_prefix("- ") {
            if let Some(key) = pending_list_key.clone() {
                let value = unquote(item);
                match out.iter_mut().find(|entry| entry.0 == key) {
                    // The key was recorded as an empty scalar when its header
                    // line was read; the first "- " promotes it to a list.
                    Some(entry) => match &mut entry.1 {
                        FmValue::List(items) => items.push(value),
                        slot => *slot = FmValue::List(vec![value]),
                    },
                    None => out.push((key, FmValue::List(vec![value]))),
                }
                continue;
            }
        }

        let Some(colon) = line.find(':') else { continue };
        let key = line[..colon].trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        let value = line[colon + 1..].trim();

        if value.is_empty() {
            // Either an empty scalar or the header of a block list; the next
            // "- " line decides.
            pending_list_key = Some(key.clone());
            out.push((key, FmValue::Scalar(String::new())));
            continue;
        }

        pending_list_key = None;

        if value.starts_with('[') && value.ends_with(']') {
            let inner = &value[1..value.len() - 1];
            let list = inner
                .split(',')
                .map(unquote)
                .filter(|s| !s.is_empty())
                .collect();
            out.push((key, FmValue::List(list)));
        } else {
            out.push((key, FmValue::Scalar(unquote(value))));
        }
    }
    out
}

fn scalar<'a>(fm: &'a [(String, FmValue)], key: &str) -> Option<&'a str> {
    fm.iter().find_map(|(k, v)| match v {
        FmValue::Scalar(s) if k.as_str() == key && !s.is_empty() => Some(s.as_str()),
        _ => None,
    })
}

fn list(fm: &[(String, FmValue)], key: &str) -> Vec<String> {
    for (k, v) in fm {
        if k.as_str() != key {
            continue;
        }
        match v {
            FmValue::List(items) => return items.clone(),
            // Tolerate `tags: work, home` written as a bare scalar.
            FmValue::Scalar(s) if !s.is_empty() => {
                return s
                    .split(',')
                    .map(|t| t.trim().trim_start_matches('#').to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            }
            _ => {}
        }
    }
    Vec::new()
}

/// Normalise a due value written by a human or an agent into `YYYY-MM-DD`.
/// Anything unparseable is dropped rather than shown as a bogus deadline.
fn normalize_date(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    // Already a date, possibly with a time suffix we can discard.
    let head = s.split(['T', ' ']).next().unwrap_or(s);
    if NaiveDate::parse_from_str(head, "%Y-%m-%d").is_ok() {
        return Some(head.to_string());
    }
    for fmt in ["%d/%m/%Y", "%m/%d/%Y", "%Y/%m/%d", "%d-%m-%Y", "%b %d %Y", "%d %b %Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(head, fmt) {
            return Some(d.format("%Y-%m-%d").to_string());
        }
    }
    None
}

pub fn parse_task(content: &str, rel_path: &str) -> Task {
    let (fm_lines, body) = split_frontmatter(content);
    let fm = parse_frontmatter(&fm_lines);

    let had_id = scalar(&fm, "id").is_some();

    let mut description = body.trim().to_string();

    // Title resolution, most to least authoritative: frontmatter, a leading
    // `# Heading` in the body, then the filename.
    let title = match scalar(&fm, "title") {
        Some(t) => t.to_string(),
        None => {
            let heading = description
                .lines()
                .next()
                .and_then(|l| l.trim().strip_prefix("# ").map(|h| h.trim().to_string()))
                .filter(|h| !h.is_empty());
            match heading {
                Some(h) => {
                    // The heading became the title; don't repeat it in the body.
                    description = description
                        .splitn(2, '\n')
                        .nth(1)
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    h
                }
                None => rel_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(rel_path)
                    .trim_end_matches(".md")
                    .replace(['-', '_'], " ")
                    .trim()
                    .to_string(),
            }
        }
    };

    let tags = {
        let mut t: Vec<String> = list(&fm, "tags")
            .into_iter()
            .map(|s| s.trim().trim_start_matches('#').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        t.dedup();
        t
    };

    let status = scalar(&fm, "status").map(Status::parse).unwrap_or_default();

    Task {
        id: scalar(&fm, "id").map(str::to_string).unwrap_or_else(new_id),
        title,
        status,
        priority: scalar(&fm, "priority")
            .map(Priority::parse)
            .unwrap_or_default(),
        due: scalar(&fm, "due")
            .or_else(|| scalar(&fm, "deadline"))
            .or_else(|| scalar(&fm, "date"))
            .and_then(normalize_date),
        tags,
        created: scalar(&fm, "created")
            .and_then(normalize_date)
            .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string()),
        completed: scalar(&fm, "completed").and_then(normalize_date),
        description,
        path: rel_path.to_string(),
        adopted: !had_id,
        archived: rel_path.starts_with("archive/"),
    }
}

/// Emit a YAML scalar, quoting only when the value would otherwise be
/// misparsed. Keeps the common case clean for human eyes.
fn yaml_scalar(v: &str) -> String {
    let needs_quotes = v.is_empty()
        || v.trim() != v
        || v.contains(": ")
        || v.contains(" #")
        || v.ends_with(':')
        || v.starts_with(['&', '*', '!', '|', '>', '%', '@', '`', '[', ']', '{', '}', ',', '"', '\'', '-', '?'])
        || matches!(
            v.to_ascii_lowercase().as_str(),
            "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "~"
        )
        || v.parse::<f64>().is_ok();

    if needs_quotes {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        v.to_string()
    }
}

pub fn render_task(task: &Task) -> String {
    let mut s = String::from("---\n");
    s.push_str(&format!("id: {}\n", yaml_scalar(&task.id)));
    s.push_str(&format!("title: {}\n", yaml_scalar(&task.title)));
    s.push_str(&format!("status: {}\n", task.status.as_str()));
    s.push_str(&format!("priority: {}\n", task.priority.as_str()));
    if let Some(due) = task.due.as_deref().filter(|d| !d.is_empty()) {
        s.push_str(&format!("due: {}\n", due));
    }
    if !task.tags.is_empty() {
        s.push_str(&format!("tags: [{}]\n", task.tags.join(", ")));
    }
    if !task.created.is_empty() {
        s.push_str(&format!("created: {}\n", task.created));
    }
    if let Some(done) = task.completed.as_deref().filter(|d| !d.is_empty()) {
        s.push_str(&format!("completed: {}\n", done));
    }
    s.push_str("---\n\n");
    s.push_str(task.description.trim());
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_fully_specified_task() {
        let md = "---\nid: abc123\ntitle: Ship the v1 beta\nstatus: doing\npriority: high\ndue: 2026-09-15\ntags: [work, release]\ncreated: 2026-08-01\n---\n\nCut the release branch.\n";
        let t = parse_task(md, "ship.md");
        assert_eq!(t.id, "abc123");
        assert_eq!(t.title, "Ship the v1 beta");
        assert_eq!(t.status, Status::Doing);
        assert_eq!(t.priority, Priority::High);
        assert_eq!(t.due.as_deref(), Some("2026-09-15"));
        assert_eq!(t.tags, vec!["work", "release"]);
        assert_eq!(t.description, "Cut the release branch.");
        assert!(!t.adopted);
    }

    #[test]
    fn round_trips_through_render() {
        let mut original = Task::new("Renew the passport".into());
        original.priority = Priority::Urgent;
        original.due = Some("2026-10-01".into());
        original.tags = vec!["personal".into(), "admin".into()];
        original.description = "Book an appointment first.\n\n- [ ] photos".into();

        let parsed = parse_task(&render_task(&original), "renew.md");
        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.title, original.title);
        assert_eq!(parsed.priority, original.priority);
        assert_eq!(parsed.due, original.due);
        assert_eq!(parsed.tags, original.tags);
        assert_eq!(parsed.description, original.description);
    }

    #[test]
    fn adopts_a_bare_file_using_its_heading() {
        // The minimum an agent has to write for a task to exist.
        let t = parse_task("# Renew the TLS certificate\n\nBefore it expires.\n", "tls.md");
        assert_eq!(t.title, "Renew the TLS certificate");
        assert_eq!(t.description, "Before it expires.");
        assert_eq!(t.status, Status::Todo);
        assert!(t.adopted, "a file with no id: is adopted");
        assert!(!t.id.is_empty(), "an id is generated for it");
    }

    #[test]
    fn falls_back_to_the_filename_for_a_title() {
        let t = parse_task("just some notes\n", "renew-tls-cert.md");
        assert_eq!(t.title, "renew tls cert");
        assert_eq!(t.description, "just some notes");
    }

    #[test]
    fn accepts_both_yaml_list_styles_and_a_bare_scalar() {
        let block = parse_task("---\ntitle: T\ntags:\n  - work\n  - home\n---\n", "a.md");
        assert_eq!(block.tags, vec!["work", "home"]);

        let flow = parse_task("---\ntitle: T\ntags: [work, home]\n---\n", "a.md");
        assert_eq!(flow.tags, vec!["work", "home"]);

        // Hand-written by someone who didn't read the docs.
        let bare = parse_task("---\ntitle: T\ntags: work, #home\n---\n", "a.md");
        assert_eq!(bare.tags, vec!["work", "home"]);
    }

    #[test]
    fn tolerates_sloppy_frontmatter() {
        let t = parse_task(
            "---\nTitle: \"Pay: the rent\"\nPriority: P1\nStatus: Completed\nDeadline: 01/09/2026\n---\nbody\n",
            "x.md",
        );
        assert_eq!(t.title, "Pay: the rent", "keys are case-insensitive, values unquoted");
        assert_eq!(t.priority, Priority::Urgent);
        assert_eq!(t.status, Status::Done);
        assert_eq!(t.due.as_deref(), Some("2026-09-01"), "deadline: is an alias for due:");
    }

    #[test]
    fn an_unterminated_fence_is_never_swallowed() {
        // Losing the user's text to a typo'd fence would be unforgivable.
        let md = "---\ntitle: oops\n\nthe rest of my notes\n";
        let t = parse_task(md, "x.md");
        assert!(t.description.contains("the rest of my notes"));
        assert!(t.description.contains("title: oops"));
    }

    #[test]
    fn drops_unparseable_dates_rather_than_inventing_one() {
        let t = parse_task("---\ntitle: T\ndue: sometime next quarter\n---\n", "x.md");
        assert_eq!(t.due, None);
    }

    #[test]
    fn quotes_only_titles_that_need_it() {
        let mut t = Task::new("Plain title".into());
        assert!(render_task(&t).contains("title: Plain title\n"));

        t.title = "Pay: the rent".into();
        assert!(render_task(&t).contains("title: \"Pay: the rent\"\n"));

        t.title = "true".into();
        assert!(render_task(&t).contains("title: \"true\"\n"), "a bare `true` would parse as a bool");

        t.title = "2026".into();
        assert!(render_task(&t).contains("title: \"2026\"\n"), "a bare number would parse as one");
    }

    #[test]
    fn omits_empty_optional_fields() {
        let rendered = render_task(&Task::new("No metadata".into()));
        assert!(!rendered.contains("due:"));
        assert!(!rendered.contains("tags:"));
        assert!(!rendered.contains("completed:"));
    }

    #[test]
    fn slugs_are_filesystem_safe() {
        assert_eq!(slugify("Ship the v1 beta!"), "ship-the-v1-beta");
        assert_eq!(slugify("  Pay: the rent / now  "), "pay-the-rent-now");
        assert_eq!(slugify("***"), "task");
        assert_eq!(slugify("Café münchen"), "café-münchen");
        assert!(slugify(&"x".repeat(200)).len() <= 60);
    }

    #[test]
    fn ids_are_unique_and_sliceable() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
        assert!(a.len() >= 5);
    }
}
