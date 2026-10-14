//! yatta as an MCP server: `yatta --mcp [--vault <dir>]`.
//!
//! The Model Context Protocol is JSON-RPC 2.0, one message per line over
//! stdin and stdout, plus a short list of methods. That is small enough to
//! serve by hand, which keeps the server inside the binary that ships: no
//! Node, no second copy of the file format, and the same `task.rs` and
//! `vault.rs` the window uses. An agent that adds a task through here and
//! the user who ticks it in the app are writing the same files the same way.
//!
//! The vault is found like the app finds it -- the settings file -- unless
//! `--vault` or `YATTA_VAULT` says otherwise. Nothing is logged to stdout
//! except protocol messages; everything human-facing goes to stderr.

use crate::task::{normalize_date, Priority, Status, Task};
use crate::vault;
use chrono::{Days, Local, NaiveDate};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

const PROTOCOL: &str = "2025-06-18";
const SUPPORTED: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];
const IDENTIFIER: &str = "com.yatta.app";

const INSTRUCTIONS: &str = "yatta is a todo app whose database is a folder of markdown files, one task \
per file. These tools read and write that folder directly; the app picks changes up live. Tasks \
are addressed by `id` or by vault-relative `path` -- both appear in every result. Statuses: todo, \
doing, done. Priorities: urgent, high, medium, low, none. Dates are YYYY-MM-DD. Prefer \
archive_task over delete_task: archiving is a reversible file move.";

// ---------------------------------------------------------------------------
// Where things are
// ---------------------------------------------------------------------------

/// The app's settings file, without an app handle: the same place Tauri's
/// `app_config_dir` resolves to on each platform.
pub fn settings_file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join(IDENTIFIER).join("settings.json"))
}

/// Mirror of `settings::default_vault`, for when no settings file exists.
fn default_vault() -> PathBuf {
    if let Some(home) = std::env::var_os("SNAP_REAL_HOME").filter(|h| !h.is_empty()) {
        let documents = PathBuf::from(&home).join("Documents");
        return if documents.is_dir() {
            documents.join("yatta")
        } else {
            PathBuf::from(home).join("yatta")
        };
    }
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("yatta")
}

/// An explicit override wins, then the app's own settings, then the folder
/// the app would suggest on first run.
pub fn resolve_vault(explicit: Option<String>) -> PathBuf {
    let explicit = explicit.or_else(|| std::env::var("YATTA_VAULT").ok());
    if let Some(dir) = explicit.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        return PathBuf::from(dir);
    }
    let configured = settings_file()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("vault_path")?.as_str().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty());
    configured.map(PathBuf::from).unwrap_or_else(default_vault)
}

/// The executable an agent should launch, as the user would type it: the
/// AppImage file rather than its mount point, the snap's `/snap/bin` wrapper
/// rather than the versioned path inside the snap.
pub fn command() -> String {
    if let Some(appimage) = std::env::var_os("APPIMAGE").filter(|v| !v.is_empty()) {
        return appimage.to_string_lossy().to_string();
    }
    if std::env::var_os("SNAP").is_some() {
        if let Some(name) = std::env::var_os("SNAP_NAME").filter(|v| !v.is_empty()) {
            return format!("/snap/bin/{}", name.to_string_lossy());
        }
    }
    // On Linux this reads /proc/self/exe, which the kernel suffixes with
    // " (deleted)" once the running binary has been replaced on disk -- a
    // rebuild under `tauri dev`, or a package upgrade while the app is open.
    // The path that remains is the new binary, which is the one to show.
    std::env::current_exe()
        .map(|p| p.to_string_lossy().trim_end_matches(" (deleted)").to_string())
        .unwrap_or_else(|_| "yatta".into())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Serve until stdin closes. Returns the process exit code.
pub fn serve(vault: PathBuf) -> i32 {
    let server = Server::new(vault.clone());
    eprintln!(
        "yatta mcp: serving {}{}",
        vault.display(),
        if vault.is_dir() { "" } else { " (folder does not exist yet)" }
    );

    let stdin = io::stdin();
    let stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => server.handle(&msg),
            Err(e) => Some(error_reply(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        if let Some(reply) = reply {
            let mut out = stdout.lock();
            if writeln!(out, "{reply}").and_then(|_| out.flush()).is_err() {
                break;
            }
        }
    }
    0
}

fn error_reply(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn result_reply(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// What a tool hands back: a text block, flagged when it is an error message
/// the model should read rather than data it should use.
fn tool_text(text: String, is_error: bool) -> Value {
    let mut v = json!({ "content": [{ "type": "text", "text": text }] });
    if is_error {
        v["isError"] = json!(true);
    }
    v
}

fn tool_ok(payload: Value) -> Value {
    tool_text(serde_json::to_string_pretty(&payload).unwrap_or_default(), false)
}

fn tool_fail(message: impl Into<String>) -> Value {
    tool_text(message.into(), true)
}

// ---------------------------------------------------------------------------
// The server
// ---------------------------------------------------------------------------

pub struct Server {
    vault: PathBuf,
}

impl Server {
    pub fn new(vault: PathBuf) -> Self {
        Server { vault }
    }

    /// Answer one message. `None` for notifications, which get no reply.
    pub fn handle(&self, msg: &Value) -> Option<Value> {
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let id = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        // A notification: no id, nothing to say back.
        let Some(id) = id.filter(|i| !i.is_null()) else {
            return None;
        };

        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_list() })),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                self.call_tool(name, &args)
                    .map_err(|e| (-32602, e))
            }
            "resources/list" => Ok(json!({ "resources": resource_list() })),
            "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
            "resources/read" => {
                let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
                self.read_resource(uri).map_err(|e| (-32002, e))
            }
            _ => Err((-32601, format!("method not found: {method}"))),
        };

        Some(match result {
            Ok(value) => result_reply(id, value),
            Err((code, message)) => error_reply(id, code, &message),
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let asked = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or(PROTOCOL);
        let version = if SUPPORTED.contains(&asked) { asked } else { PROTOCOL };
        json!({
            "protocolVersion": version,
            "capabilities": {
                "tools": { "listChanged": false },
                "resources": { "subscribe": false, "listChanged": false },
            },
            "serverInfo": { "name": "yatta", "version": env!("CARGO_PKG_VERSION") },
            "instructions": INSTRUCTIONS,
        })
    }

    // -- tools ---------------------------------------------------------------

    /// `Err` is a protocol-level problem (unknown tool). A problem with the
    /// request itself comes back as `Ok` with `isError`, so the model sees it.
    fn call_tool(&self, name: &str, args: &Value) -> Result<Value, String> {
        Ok(match name {
            "list_tasks" => self.list_tasks(args),
            "get_task" => self.with_task(args, |root, t| tool_ok(full(root, &t))),
            "add_task" => self.add_task(args),
            "update_task" => self.update_task(args),
            "complete_task" => self.with_task(args, |root, mut t| {
                if t.status != Status::Done {
                    let _guard = vault::lock();
                    vault::apply_status(&mut t, Status::Done);
                    if let Err(e) = vault::save_task(root, &t) {
                        return tool_fail(e);
                    }
                }
                tool_ok(full(root, &t))
            }),
            "archive_task" => self.with_task(args, |root, mut t| {
                let _guard = vault::lock();
                match vault::archive_task(root, &t.path) {
                    Ok(p) => {
                        t.path = p;
                        t.archived = true;
                        tool_ok(summary(&t))
                    }
                    Err(e) => tool_fail(e),
                }
            }),
            "restore_task" => self.with_task(args, |root, mut t| {
                if !t.archived {
                    return tool_ok(summary(&t));
                }
                let _guard = vault::lock();
                match vault::restore_task(root, &t.path) {
                    Ok(p) => {
                        t.path = p;
                        t.archived = false;
                        tool_ok(summary(&t))
                    }
                    Err(e) => tool_fail(e),
                }
            }),
            "delete_task" => self.with_task(args, |root, t| {
                let _guard = vault::lock();
                match vault::delete_task(root, &t.path) {
                    Ok(()) => tool_ok(json!({ "deleted": summary(&t) })),
                    Err(e) => tool_fail(e),
                }
            }),
            _ => return Err(format!("unknown tool: {name}")),
        })
    }

    fn missing_vault(&self) -> Option<Value> {
        if self.vault.is_dir() {
            return None;
        }
        Some(tool_fail(format!(
            "The vault folder does not exist: {}\nPick a vault in the yatta app, or point this \
             server at one with --vault <dir> or $YATTA_VAULT.",
            self.vault.display()
        )))
    }

    /// Resolve the `ref` argument to a task and hand it to `f`.
    fn with_task(&self, args: &Value, f: impl FnOnce(&Path, Task) -> Value) -> Value {
        if let Some(err) = self.missing_vault() {
            return err;
        }
        let reference = str_arg(args, "ref").unwrap_or_default();
        match find_task(&self.vault, &reference) {
            Some(task) => f(&self.vault, task),
            None => tool_fail(format!(
                "No task with id or path \"{reference}\". Use list_tasks to see what exists."
            )),
        }
    }

    fn list_tasks(&self, args: &Value) -> Value {
        if let Some(err) = self.missing_vault() {
            return err;
        }
        let status = str_arg(args, "status").unwrap_or_else(|| "open".into());
        let tag = str_arg(args, "tag").map(|t| t.trim_start_matches('#').to_string());
        let due = str_arg(args, "due").unwrap_or_else(|| "any".into());
        let search = str_arg(args, "search").map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
        let include_archived = args.get("include_archived").and_then(Value::as_bool).unwrap_or(false);
        let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(200) as usize;

        let now = today();
        let week_end = add_days(&now, 7);

        let mut tasks: Vec<Task> = vault::list_tasks(&self.vault)
            .into_iter()
            .filter(|t| include_archived || !t.archived)
            .filter(|t| match status.as_str() {
                "open" => t.status != Status::Done,
                "any" => true,
                s => t.status.as_str() == s,
            })
            .filter(|t| tag.as_ref().map_or(true, |tag| t.tags.iter().any(|x| x == tag)))
            .filter(|t| match due.as_str() {
                "overdue" => t.due.as_deref().is_some_and(|d| d < now.as_str()) && t.status != Status::Done,
                "today" => t.due.as_deref() == Some(now.as_str()),
                "week" => t.due.as_deref().is_some_and(|d| d >= now.as_str() && d <= week_end.as_str()),
                "upcoming" => t.due.as_deref().is_some_and(|d| d >= now.as_str()),
                "none" => t.due.is_none(),
                _ => true,
            })
            .filter(|t| {
                search.as_ref().map_or(true, |needle| {
                    t.title.to_lowercase().contains(needle) || t.description.to_lowercase().contains(needle)
                })
            })
            .collect();

        // Soonest deadline first, undated last; then by priority, then age.
        tasks.sort_by(|a, b| {
            match (&a.due, &b.due) {
                (Some(x), Some(y)) if x != y => return x.cmp(y),
                (Some(_), None) => return std::cmp::Ordering::Less,
                (None, Some(_)) => return std::cmp::Ordering::Greater,
                _ => {}
            }
            priority_rank(a.priority)
                .cmp(&priority_rank(b.priority))
                .then_with(|| a.created.cmp(&b.created))
        });

        tool_ok(json!({
            "vault": self.vault.display().to_string(),
            "today": now,
            "count": tasks.len(),
            "tasks": tasks.iter().take(limit).map(summary).collect::<Vec<_>>(),
        }))
    }

    fn add_task(&self, args: &Value) -> Value {
        if let Some(err) = self.missing_vault() {
            return err;
        }
        let Some(title) = str_arg(args, "title").filter(|t| !t.trim().is_empty()) else {
            return tool_fail("add_task needs a title.");
        };
        let mut task = Task::new(title);
        task.description = str_arg(args, "description").unwrap_or_default();

        if let Some(p) = str_arg(args, "priority") {
            match parse_priority(&p) {
                Some(p) => task.priority = p,
                None => return tool_fail(bad_priority(&p)),
            }
        }
        if let Some(raw) = str_arg(args, "due") {
            match parse_due(&raw) {
                Ok(due) => task.due = due,
                Err(e) => return tool_fail(e),
            }
        }
        task.tags = clean_tags(args.get("tags"));
        if let Some(s) = str_arg(args, "status") {
            match parse_status(&s) {
                Some(s) => vault::apply_status(&mut task, s),
                None => return tool_fail(bad_status(&s)),
            }
        }

        let _guard = vault::lock();
        match vault::save_task(&self.vault, &task) {
            Ok(path) => {
                task.path = path;
                tool_ok(full(&self.vault, &task))
            }
            Err(e) => tool_fail(e),
        }
    }

    fn update_task(&self, args: &Value) -> Value {
        self.with_task(args, |root, mut task| {
            if let Some(raw) = str_arg(args, "due") {
                match parse_due(&raw) {
                    Ok(due) => task.due = due,
                    Err(e) => return tool_fail(e),
                }
            }
            if let Some(t) = str_arg(args, "title").filter(|t| !t.trim().is_empty()) {
                task.title = t;
            }
            if let Some(d) = str_arg(args, "description") {
                task.description = d;
            }
            if let Some(p) = str_arg(args, "priority") {
                match parse_priority(&p) {
                    Some(p) => task.priority = p,
                    None => return tool_fail(bad_priority(&p)),
                }
            }
            if args.get("tags").is_some_and(|t| t.is_array()) {
                task.tags = clean_tags(args.get("tags"));
            }
            for t in clean_tags(args.get("add_tags")) {
                if !task.tags.contains(&t) {
                    task.tags.push(t);
                }
            }
            let drop = clean_tags(args.get("remove_tags"));
            if !drop.is_empty() {
                task.tags.retain(|t| !drop.contains(t));
            }
            if let Some(s) = str_arg(args, "status") {
                match parse_status(&s) {
                    Some(s) if s != task.status => vault::apply_status(&mut task, s),
                    Some(_) => {}
                    None => return tool_fail(bad_status(&s)),
                }
            }

            let _guard = vault::lock();
            match vault::save_task(root, &task) {
                Ok(_) => tool_ok(full(root, &task)),
                Err(e) => tool_fail(e),
            }
        })
    }

    // -- resources -----------------------------------------------------------

    fn read_resource(&self, uri: &str) -> Result<Value, String> {
        let (mime, text) = match uri {
            "yatta://vault" => {
                let tasks = if self.vault.is_dir() { vault::list_tasks(&self.vault) } else { Vec::new() };
                let active: Vec<&Task> = tasks.iter().filter(|t| !t.archived).collect();
                let count = |s: Status| active.iter().filter(|t| t.status == s).count();
                let info = json!({
                    "path": self.vault.display().to_string(),
                    "exists": self.vault.is_dir(),
                    "today": today(),
                    "todo": count(Status::Todo),
                    "doing": count(Status::Doing),
                    "done": count(Status::Done),
                    "archived": tasks.len() - active.len(),
                });
                ("application/json", serde_json::to_string_pretty(&info).unwrap_or_default())
            }
            "yatta://format" => {
                // The vault's own README wins, in case the user annotated it;
                // the one the app seeds is the fallback.
                let text = std::fs::read_to_string(self.vault.join("README.md"))
                    .unwrap_or_else(|_| include_str!("../assets/vault_readme.md").to_string());
                ("text/markdown", text)
            }
            _ => return Err(format!("unknown resource: {uri}")),
        };
        Ok(json!({ "contents": [{ "uri": uri, "mimeType": mime, "text": text }] }))
    }
}

// ---------------------------------------------------------------------------
// Catalogue
// ---------------------------------------------------------------------------

fn ref_schema() -> Value {
    json!({ "type": "string", "description": "Task id, or its vault-relative path (with or without .md)." })
}

fn one_ref_input() -> Value {
    json!({ "type": "object", "properties": { "ref": ref_schema() }, "required": ["ref"] })
}

fn tool_list() -> Vec<Value> {
    let status = json!({ "type": "string", "enum": ["todo", "doing", "done"] });
    let priority = json!({ "type": "string", "enum": ["urgent", "high", "medium", "low", "none"] });
    let tags = json!({ "type": "array", "items": { "type": "string" }, "description": "Tag names without the leading #." });
    vec![
        json!({
            "name": "list_tasks",
            "title": "List tasks",
            "description": "List tasks in the vault with optional filters. By default returns open tasks (todo and doing) that are not archived. Returns a JSON array of task summaries without descriptions; use get_task for the body of one task.",
            "inputSchema": { "type": "object", "properties": {
                "status": { "type": "string", "enum": ["open", "todo", "doing", "done", "any"], "description": "Filter by status. `open` (default) is todo + doing." },
                "tag": { "type": "string", "description": "Only tasks carrying this tag (no leading #)." },
                "due": { "type": "string", "enum": ["overdue", "today", "week", "upcoming", "none", "any"], "description": "Filter by deadline: overdue (before today), today, week (today through 7 days out), upcoming (today or later), none (no deadline). Default any." },
                "search": { "type": "string", "description": "Case-insensitive substring match on title and description." },
                "include_archived": { "type": "boolean", "description": "Also include tasks under archive/. Default false." },
                "limit": { "type": "integer", "minimum": 1, "description": "Maximum number of tasks to return. Default 200." }
            } },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "get_task",
            "title": "Get a task",
            "description": "Fetch one task in full, including its markdown description and absolute file path.",
            "inputSchema": one_ref_input(),
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "add_task",
            "title": "Add a task",
            "description": "Create a new task file in the vault. Only the title is required. The description is markdown; `- [ ] step` lines render as a checklist in the app.",
            "inputSchema": { "type": "object", "properties": {
                "title": { "type": "string", "description": "One line. Whitespace is folded." },
                "description": { "type": "string", "description": "Markdown body. Put bare URLs on their own line." },
                "status": status, "priority": priority,
                "due": { "type": "string", "description": "YYYY-MM-DD, `today` or `tomorrow`. Omit for no deadline." },
                "tags": tags
            }, "required": ["title"] }
        }),
        json!({
            "name": "update_task",
            "title": "Update a task",
            "description": "Change any fields of an existing task. Fields left out are untouched. Setting status to done stamps today's completion date; moving it back clears it. `tags` replaces the whole list; use add_tags / remove_tags to adjust it instead.",
            "inputSchema": { "type": "object", "properties": {
                "ref": ref_schema(),
                "title": { "type": "string" },
                "description": { "type": "string", "description": "Replaces the whole markdown body." },
                "status": status, "priority": priority,
                "due": { "type": "string", "description": "YYYY-MM-DD, `today`, `tomorrow`, or `none` to clear." },
                "tags": { "type": "array", "items": { "type": "string" }, "description": "Replace the tag list." },
                "add_tags": { "type": "array", "items": { "type": "string" } },
                "remove_tags": { "type": "array", "items": { "type": "string" } }
            }, "required": ["ref"] }
        }),
        json!({
            "name": "complete_task",
            "title": "Complete a task",
            "description": "Mark a task done and record today's date as its completion date.",
            "inputSchema": one_ref_input(),
            "annotations": { "idempotentHint": true }
        }),
        json!({
            "name": "archive_task",
            "title": "Archive a task",
            "description": "Move a task's file into the vault's archive/ folder. Nothing is deleted and the task keeps its contents; archived tasks are hidden from the app's normal views. Reversible with restore_task.",
            "inputSchema": one_ref_input(),
            "annotations": { "idempotentHint": true }
        }),
        json!({
            "name": "restore_task",
            "title": "Restore an archived task",
            "description": "Move a task's file out of archive/ back into the vault root.",
            "inputSchema": one_ref_input(),
            "annotations": { "idempotentHint": true }
        }),
        json!({
            "name": "delete_task",
            "title": "Delete a task",
            "description": "Permanently delete a task's file. Not reversible from here -- prefer archive_task unless the user explicitly asked for deletion.",
            "inputSchema": one_ref_input(),
            "annotations": { "destructiveHint": true }
        }),
    ]
}

fn resource_list() -> Vec<Value> {
    vec![
        json!({ "uri": "yatta://vault", "name": "vault", "title": "Vault location and counts",
            "description": "Where the task folder is and how many tasks it holds, by status.",
            "mimeType": "application/json" }),
        json!({ "uri": "yatta://format", "name": "format", "title": "Task file format",
            "description": "The markdown + frontmatter format of a task file, for agents that want to write files directly.",
            "mimeType": "text/markdown" }),
    ]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn str_arg(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn add_days(ymd: &str, days: u64) -> String {
    NaiveDate::parse_from_str(ymd, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.checked_add_days(Days::new(days)))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| ymd.to_string())
}

/// Accept a date as the app would, or say so loudly rather than drop it.
fn parse_due(raw: &str) -> Result<Option<String>, String> {
    match raw.trim().to_lowercase().as_str() {
        "" | "none" | "null" => Ok(None),
        "today" => Ok(Some(today())),
        "tomorrow" => Ok(Some(add_days(&today(), 1))),
        _ => normalize_date(raw)
            .map(Some)
            .ok_or_else(|| format!("Could not read \"{raw}\" as a date. Use YYYY-MM-DD.")),
    }
}

/// Only the spellings the schema advertises. `Status::parse` would quietly
/// read "whatever" as todo, which is the wrong kind of forgiving for a tool.
fn parse_status(s: &str) -> Option<Status> {
    match s.trim() {
        "todo" => Some(Status::Todo),
        "doing" => Some(Status::Doing),
        "done" => Some(Status::Done),
        _ => None,
    }
}

fn parse_priority(s: &str) -> Option<Priority> {
    match s.trim() {
        "urgent" => Some(Priority::Urgent),
        "high" => Some(Priority::High),
        "medium" => Some(Priority::Medium),
        "low" => Some(Priority::Low),
        "none" => Some(Priority::None),
        _ => None,
    }
}

fn bad_status(s: &str) -> String {
    format!("Unknown status \"{s}\". Use todo, doing or done.")
}

fn bad_priority(s: &str) -> String {
    format!("Unknown priority \"{s}\". Use urgent, high, medium, low or none.")
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

/// Tags as the file format wants them: no `#`, no blanks, no repeats, and a
/// comma-separated string split the way a human would mean it.
fn clean_tags(value: Option<&Value>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let Some(Value::Array(items)) = value else { return out };
    for item in items.iter().filter_map(Value::as_str) {
        for piece in item.split(',') {
            let t = piece.trim().trim_start_matches('#').to_string();
            if !t.is_empty() && !out.contains(&t) {
                out.push(t);
            }
        }
    }
    out
}

/// Find one task by id, by vault-relative path, or by path without `.md`.
fn find_task(root: &Path, reference: &str) -> Option<Task> {
    let wanted = reference.trim().trim_start_matches("./").trim_start_matches('/');
    if wanted.is_empty() {
        return None;
    }
    let tasks = vault::list_tasks(root);
    let with_ext = format!("{wanted}.md");
    tasks
        .iter()
        .find(|t| t.id == wanted)
        .or_else(|| tasks.iter().find(|t| t.path == wanted || t.path == with_ext))
        .cloned()
}

/// What an agent needs to know about a task, without the body.
fn summary(t: &Task) -> Value {
    let mut v = json!({
        "id": t.id,
        "title": t.title,
        "status": t.status.as_str(),
        "priority": t.priority.as_str(),
        "due": t.due,
        "tags": t.tags,
        "created": t.created,
        "completed": t.completed,
        "path": t.path,
    });
    if t.archived {
        v["archived"] = json!(true);
    }
    if t.conflicted {
        v["conflicted"] = json!(true);
    }
    v
}

fn full(root: &Path, t: &Task) -> Value {
    let mut v = summary(t);
    v["description"] = json!(t.description);
    v["file"] = json!(root.join(&t.path).display().to_string());
    v
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_vault() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("yatta-mcp-{}", crate::task::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn request(method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
    }

    fn call(server: &Server, tool: &str, args: Value) -> (bool, Value) {
        let reply = server
            .handle(&request("tools/call", json!({ "name": tool, "arguments": args })))
            .unwrap();
        let result = &reply["result"];
        let text = result["content"][0]["text"].as_str().unwrap();
        let is_error = result["isError"].as_bool().unwrap_or(false);
        let value = if is_error { json!(text) } else { serde_json::from_str(text).unwrap() };
        (is_error, value)
    }

    #[test]
    fn initialize_answers_with_a_supported_version() {
        let s = Server::new(temp_vault());
        let reply = s.handle(&request("initialize", json!({ "protocolVersion": "2025-03-26" }))).unwrap();
        assert_eq!(reply["result"]["protocolVersion"], "2025-03-26");
        let reply = s.handle(&request("initialize", json!({ "protocolVersion": "1999-01-01" }))).unwrap();
        assert_eq!(reply["result"]["protocolVersion"], PROTOCOL);
        assert_eq!(reply["result"]["serverInfo"]["name"], "yatta");
    }

    #[test]
    fn notifications_get_no_reply_and_unknown_methods_an_error() {
        let s = Server::new(temp_vault());
        assert!(s.handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
        let reply = s.handle(&request("prompts/list", json!({}))).unwrap();
        assert_eq!(reply["error"]["code"], -32601);
    }

    #[test]
    fn the_catalogue_is_complete() {
        let s = Server::new(temp_vault());
        let tools = s.handle(&request("tools/list", json!({}))).unwrap();
        let names: Vec<&str> = tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["list_tasks", "get_task", "add_task", "update_task", "complete_task", "archive_task", "restore_task", "delete_task"]);
        let resources = s.handle(&request("resources/list", json!({}))).unwrap();
        assert_eq!(resources["result"]["resources"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_task_goes_round_the_whole_life_cycle() {
        let root = temp_vault();
        let s = Server::new(root.clone());

        let (err, added) = call(&s, "add_task", json!({
            "title": "Ship the v1 beta", "priority": "high", "due": "tomorrow",
            "tags": ["#work", "release"], "description": "- [ ] cut branch"
        }));
        assert!(!err);
        assert_eq!(added["path"], "ship-the-v1-beta.md");
        assert_eq!(added["tags"], json!(["work", "release"]));
        assert_eq!(added["due"], json!(add_days(&today(), 1)));
        let id = added["id"].as_str().unwrap().to_string();

        // The file on disk is one the app would have written.
        let on_disk = vault::list_tasks(&root);
        assert_eq!(on_disk.len(), 1);
        assert_eq!(on_disk[0].id, id);
        assert!(!on_disk[0].adopted, "written with an id, so not an adopted file");

        let (_, listed) = call(&s, "list_tasks", json!({ "tag": "work" }));
        assert_eq!(listed["count"], 1);
        let (_, listed) = call(&s, "list_tasks", json!({ "due": "overdue" }));
        assert_eq!(listed["count"], 0);

        let (err, updated) = call(&s, "update_task", json!({
            "ref": id, "add_tags": ["later"], "remove_tags": ["work"], "due": "none", "status": "doing"
        }));
        assert!(!err);
        assert_eq!(updated["tags"], json!(["release", "later"]));
        assert_eq!(updated["due"], Value::Null);
        assert_eq!(updated["status"], "doing");

        let (_, done) = call(&s, "complete_task", json!({ "ref": "ship-the-v1-beta" }));
        assert_eq!(done["status"], "done");
        assert_eq!(done["completed"], json!(today()));

        let (_, archived) = call(&s, "archive_task", json!({ "ref": id }));
        assert_eq!(archived["path"], "archive/ship-the-v1-beta.md");
        let (_, listed) = call(&s, "list_tasks", json!({ "status": "any" }));
        assert_eq!(listed["count"], 0, "archived tasks are hidden by default");
        let (_, listed) = call(&s, "list_tasks", json!({ "status": "any", "include_archived": true }));
        assert_eq!(listed["count"], 1);

        let (_, restored) = call(&s, "restore_task", json!({ "ref": id }));
        assert_eq!(restored["path"], "ship-the-v1-beta.md");

        let (_, deleted) = call(&s, "delete_task", json!({ "ref": id }));
        assert_eq!(deleted["deleted"]["id"], json!(id));
        assert!(vault::list_tasks(&root).is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn bad_input_is_reported_not_swallowed() {
        let root = temp_vault();
        let s = Server::new(root.clone());
        let (err, msg) = call(&s, "add_task", json!({ "title": "x", "due": "someday" }));
        assert!(err);
        assert!(msg.as_str().unwrap().contains("someday"));
        let (err, _) = call(&s, "add_task", json!({ "title": "x", "status": "whatever" }));
        assert!(err);
        let (err, _) = call(&s, "get_task", json!({ "ref": "missing" }));
        assert!(err);
        assert!(vault::list_tasks(&root).is_empty(), "nothing was written");

        let reply = s.handle(&request("tools/call", json!({ "name": "nope", "arguments": {} }))).unwrap();
        assert_eq!(reply["error"]["code"], -32602);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_vault_is_explained() {
        let s = Server::new(PathBuf::from("/nonexistent/yatta-vault"));
        let (err, msg) = call(&s, "list_tasks", json!({}));
        assert!(err);
        assert!(msg.as_str().unwrap().contains("--vault"));
    }

    #[test]
    fn resources_describe_the_vault_and_the_format() {
        let root = temp_vault();
        let s = Server::new(root.clone());
        call(&s, "add_task", json!({ "title": "one" }));
        let reply = s.handle(&request("resources/read", json!({ "uri": "yatta://vault" }))).unwrap();
        let info: Value = serde_json::from_str(reply["result"]["contents"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(info["todo"], 1);
        let reply = s.handle(&request("resources/read", json!({ "uri": "yatta://format" }))).unwrap();
        assert!(reply["result"]["contents"][0]["text"].as_str().unwrap().starts_with("# yatta vault"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
