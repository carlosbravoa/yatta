#!/usr/bin/env python3
"""Read and write a yatta vault from the command line.

The skill's counterpart to `yatta --mcp`: the same operations, for agents that
have a shell but no MCP connection. Python standard library only, so it runs
wherever python3 does.

It mirrors the app's own rules (src-tauri/src/task.rs and vault.rs) so a file
written here is exactly what the app would have written: base36 id, slugified
filename chosen once and never changed, frontmatter key order, YAML quoting,
atomic writes, `completed:` stamped and cleared as the status changes.

Every command prints JSON on stdout. Errors go to stderr with exit status 1.

  yatta.py vault
  yatta.py list     [--status open|todo|doing|done|any] [--tag T]
                    [--due overdue|today|week|upcoming|none|any]
                    [--search TEXT] [--include-archived] [--limit N]
  yatta.py tags
  yatta.py get      REF
  yatta.py add      --title T [--description D | --description-file F|-]
                    [--status S] [--priority P] [--due D] [--tags a,b] [--dry-run]
  yatta.py update   REF [--title T] [--description D | --description-file F|-]
                    [--status S] [--priority P] [--due D|none]
                    [--tags a,b] [--add-tags a,b] [--remove-tags a,b] [--dry-run]
  yatta.py complete REF
  yatta.py archive  REF
  yatta.py restore  REF

REF is a task id, or a vault-relative path with or without `.md`.
--vault DIR (before the command) or $YATTA_VAULT overrides the vault location.
"""

import argparse
import json
import os
import platform
import random
import re
import sys
import time
from datetime import date, datetime, timedelta
from pathlib import Path

ALPHABET = "0123456789abcdefghijklmnopqrstuvwxyz"
STATUSES = ("todo", "doing", "done")
PRIORITIES = ("urgent", "high", "medium", "low", "none")
PRIORITY_RANK = {p: i for i, p in enumerate(PRIORITIES)}
ARCHIVE_DIR = "archive"
SKIP_DIRS = {".git", "node_modules", ".obsidian", ".trash"}
IDENTIFIER = "com.yatta.app"


def fail(message: str) -> None:
    print(f"yatta: {message}", file=sys.stderr)
    sys.exit(1)


def today() -> str:
    return date.today().isoformat()


# ---------------------------------------------------------------------------
# Where the vault is
# ---------------------------------------------------------------------------


def config_dir() -> Path:
    home = Path.home()
    system = platform.system()
    if system == "Darwin":
        return home / "Library" / "Application Support"
    if system == "Windows":
        return Path(os.environ.get("APPDATA", home / "AppData" / "Roaming"))
    return Path(os.environ.get("XDG_CONFIG_HOME") or home / ".config")


def settings_files() -> list:
    """The app's settings, then the snap's: a confined app keeps its own HOME."""
    files = [config_dir() / IDENTIFIER / "settings.json"]
    files.append(Path.home() / "snap" / "yatta" / "current" / ".config" / IDENTIFIER / "settings.json")
    return files


def resolve_vault(explicit) -> Path:
    """Same order as `yatta --mcp`: flag, $YATTA_VAULT, settings, default."""
    chosen = (explicit or os.environ.get("YATTA_VAULT") or "").strip()
    if chosen:
        return Path(chosen).expanduser()
    for f in settings_files():
        try:
            configured = (json.loads(f.read_text()).get("vault_path") or "").strip()
        except (OSError, ValueError, AttributeError):
            continue
        if configured:
            return Path(configured).expanduser()
    documents = Path.home() / "Documents"
    return (documents if documents.is_dir() else Path.home()) / "yatta"


# ---------------------------------------------------------------------------
# Task format -- a port of task.rs
# ---------------------------------------------------------------------------


def to_base36(n: int) -> str:
    if n == 0:
        return "0000"
    out = []
    while n > 0:
        out.append(ALPHABET[n % 36])
        n //= 36
    out.reverse()
    while len(out) < 4:
        out.append("0")
    return "".join(out)


def new_id() -> str:
    """Short, sortable, URL-safe: base36 millis + 4 random base36 chars."""
    millis = max(0, int(time.time() * 1000))
    return to_base36(millis) + "".join(random.choice(ALPHABET) for _ in range(4))


def slugify(title: str) -> str:
    out = []
    prev_dash = False
    for ch in title:
        if ch.isalnum():
            out.append(ch.lower())
            prev_dash = False
        elif not prev_dash and out:
            out.append("-")
            prev_dash = True
    s = "".join(out).strip("-")[:60].strip("-")
    return s or "task"


def single_line(v: str) -> str:
    return " ".join(v.split())


def yaml_scalar(v: str) -> str:
    needs_quotes = (
        not v
        or v.strip() != v
        or ": " in v
        or " #" in v
        or v.endswith(":")
        or v[0] in "&*!|>%@`[]{},\"'-?"
        or v.lower() in ("true", "false", "null", "yes", "no", "on", "off", "~")
    )
    if not needs_quotes:
        try:
            float(v)
            needs_quotes = True
        except ValueError:
            pass
    if needs_quotes:
        return '"' + v.replace("\\", "\\\\").replace('"', '\\"') + '"'
    return v


def unquote(s: str) -> str:
    s = s.strip()
    if len(s) >= 2 and s[0] == s[-1] and s[0] in "\"'":
        return s[1:-1].replace('\\"', '"').replace("\\'", "'")
    return s


def split_frontmatter(content: str):
    text = content.replace("\r\n", "\n").lstrip("﻿")
    lines = text.split("\n")
    if not lines or lines[0].strip() != "---":
        return [], text
    for i in range(1, len(lines)):
        if lines[i].strip() == "---":
            return lines[1:i], "\n".join(lines[i + 1 :]).lstrip("\n")
    # Unterminated fence: the whole file is body, never swallowed.
    return [], text


def parse_frontmatter(lines):
    fm = []  # [key, value] where value is str or list, in file order
    pending = None
    for raw in lines:
        line = raw.rstrip()
        trimmed = line.strip()
        if not trimmed or trimmed.startswith("#"):
            continue
        if trimmed.startswith("- ") and pending is not None:
            value = unquote(trimmed[2:])
            for entry in fm:
                if entry[0] == pending:
                    if isinstance(entry[1], list):
                        entry[1].append(value)
                    else:
                        entry[1] = [value]
                    break
            continue
        if ":" not in line:
            continue
        key, _, value = line.partition(":")
        key = key.strip().lower()
        if not key:
            continue
        value = value.strip()
        if not value:
            pending = key
            fm.append([key, ""])
            continue
        pending = None
        if value.startswith("[") and value.endswith("]"):
            fm.append([key, [x for x in (unquote(p) for p in value[1:-1].split(",")) if x]])
        else:
            fm.append([key, unquote(value)])
    return fm


def fm_scalar(fm, key):
    for k, v in fm:
        if k == key and isinstance(v, str) and v:
            return v
    return None


def fm_list(fm, key):
    for k, v in fm:
        if k != key:
            continue
        if isinstance(v, list):
            return v
        if v:
            return [t.strip().lstrip("#") for t in v.split(",") if t.strip().lstrip("#")]
    return []


def normalize_date(raw):
    s = (raw or "").strip()
    if not s:
        return None
    head = re.split(r"[T ]", s, maxsplit=1)[0]
    try:
        return datetime.strptime(head, "%Y-%m-%d").date().isoformat()
    except ValueError:
        pass
    for fmt in ("%d/%m/%Y", "%m/%d/%Y", "%Y/%m/%d", "%d-%m-%Y"):
        try:
            return datetime.strptime(head, fmt).date().isoformat()
        except ValueError:
            pass
    return None


def parse_status(s: str) -> str:
    s = s.strip().lower()
    if s in ("done", "complete", "completed", "x", "closed"):
        return "done"
    if s in ("doing", "in-progress", "in_progress", "wip", "started"):
        return "doing"
    return "todo"


def parse_priority(s: str) -> str:
    s = s.strip().strip("!").lower()
    if s in ("urgent", "critical", "p0", "p1", "1"):
        return "urgent"
    if s in ("high", "p2", "2"):
        return "high"
    if s in ("medium", "med", "normal", "p3", "3"):
        return "medium"
    if s in ("low", "p4", "4"):
        return "low"
    return "none"


def parse_task(content: str, rel_path: str) -> dict:
    fm_lines, body = split_frontmatter(content)
    fm = parse_frontmatter(fm_lines)
    description = body.strip()

    title = fm_scalar(fm, "title")
    if title is None:
        first = description.split("\n", 1)[0].strip()
        if first.startswith("# ") and first[2:].strip():
            title = first[2:].strip()
            description = description.split("\n", 1)[1].strip() if "\n" in description else ""
        else:
            name = rel_path.rsplit("/", 1)[-1]
            title = (name[:-3] if name.endswith(".md") else name).replace("-", " ").replace("_", " ").strip()

    tags = []
    for t in fm_list(fm, "tags"):
        t = t.strip().lstrip("#")
        if t and (not tags or tags[-1] != t):
            tags.append(t)

    due = fm_scalar(fm, "due") or fm_scalar(fm, "deadline") or fm_scalar(fm, "date")
    status = fm_scalar(fm, "status")
    priority = fm_scalar(fm, "priority")
    task = {
        "id": fm_scalar(fm, "id") or new_id(),
        "title": title,
        "status": parse_status(status) if status else "todo",
        "priority": parse_priority(priority) if priority else "none",
        "due": normalize_date(due),
        "tags": tags,
        "created": normalize_date(fm_scalar(fm, "created")) or today(),
        "completed": normalize_date(fm_scalar(fm, "completed")),
        "path": rel_path,
        "description": description,
    }
    if rel_path.startswith(ARCHIVE_DIR + "/"):
        task["archived"] = True
    if has_markers(description):
        task["conflicted"] = True
    return task


def has_markers(body: str) -> bool:
    """Git conflict markers left by a sync that could not merge on its own."""
    opened = False
    for line in body.splitlines():
        if line.startswith("<<<<<<<"):
            opened = True
        elif opened and line.startswith(">>>>>>>"):
            return True
    return False


def render_task(task: dict) -> str:
    s = "---\n"
    s += f"id: {yaml_scalar(task['id'])}\n"
    s += f"title: {yaml_scalar(single_line(task['title']))}\n"
    s += f"status: {task['status']}\n"
    s += f"priority: {task['priority']}\n"
    if task.get("due"):
        s += f"due: {task['due']}\n"
    if task.get("tags"):
        s += f"tags: [{', '.join(task['tags'])}]\n"
    if task.get("created"):
        s += f"created: {task['created']}\n"
    if task.get("completed"):
        s += f"completed: {task['completed']}\n"
    s += "---\n\n"
    s += (task.get("description") or "").strip()
    s += "\n"
    return s


# ---------------------------------------------------------------------------
# Vault -- a port of vault.rs
# ---------------------------------------------------------------------------


def collect(root: Path):
    out = []

    def walk(d: Path, depth: int):
        if depth > 6:
            return
        try:
            entries = sorted(d.iterdir())
        except OSError:
            return
        for p in entries:
            if p.is_dir():
                if p.name.startswith(".") or p.name in SKIP_DIRS:
                    continue
                walk(p, depth + 1)
            elif p.suffix == ".md":
                if d == root and p.name.lower() == "readme.md":
                    continue
                out.append(p)

    walk(root, 0)
    return out


def rel(root: Path, path: Path) -> str:
    return path.relative_to(root).as_posix()


def load_all(root: Path):
    tasks = []
    seen = set()
    for p in collect(root):
        try:
            content = p.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        t = parse_task(content, rel(root, p))
        if t["id"] in seen:
            # A copied file would make an id address two tasks; the path is unique.
            t["id"] = t["path"]
        seen.add(t["id"])
        tasks.append(t)
    return tasks


def find_task(root: Path, ref: str) -> dict:
    wanted = ref.strip()
    while wanted.startswith("./"):
        wanted = wanted[2:]
    wanted = wanted.lstrip("/")
    if not wanted:
        fail("no task reference given")
    tasks = load_all(root)
    for t in tasks:
        if t["id"] == wanted:
            return t
    for t in tasks:
        if t["path"] in (wanted, wanted + ".md"):
            return t
    fail(f'no task with id or path "{ref}". Run `list --status any --include-archived` to see what exists.')


def unique_path(directory: Path, stem: str) -> Path:
    candidate = directory / f"{stem}.md"
    if not candidate.exists():
        return candidate
    for n in range(2, 1000):
        candidate = directory / f"{stem}-{n}.md"
        if not candidate.exists():
            return candidate
    return directory / f"{stem}-{new_id()}.md"


def inside(root: Path, path: Path) -> bool:
    try:
        path.resolve().relative_to(root.resolve())
        return True
    except ValueError:
        return False


def save_task(root: Path, task: dict) -> None:
    """Atomic write. A new task gets a filename once; an existing one keeps it."""
    if task.get("path"):
        path = root / task["path"]
    else:
        path = unique_path(root, slugify(task["title"]))
        task["path"] = rel(root, path)
    if not inside(root, path):
        fail("refusing to write outside the vault")
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(render_task(task), encoding="utf-8")
    os.replace(tmp, path)


def move(root: Path, task: dict, directory: Path) -> None:
    src = root / task["path"]
    if not inside(root, src):
        fail("refusing to move a file outside the vault")
    directory.mkdir(parents=True, exist_ok=True)
    dest = unique_path(directory, src.stem)
    os.replace(src, dest)
    task["path"] = rel(root, dest)


def apply_status(task: dict, status: str) -> None:
    task["status"] = status
    task["completed"] = today() if status == "done" else None


# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------


def summary(t: dict) -> dict:
    keys = ("id", "title", "status", "priority", "due", "tags", "created", "completed", "path")
    out = {k: t.get(k) for k in keys}
    for flag in ("archived", "conflicted"):
        if t.get(flag):
            out[flag] = True
    return out


def full(root: Path, t: dict) -> dict:
    out = summary(t)
    out["description"] = t.get("description", "")
    out["file"] = str(root / t["path"])
    return out


def emit(value) -> None:
    print(json.dumps(value, indent=2, ensure_ascii=False))


# ---------------------------------------------------------------------------
# Argument helpers
# ---------------------------------------------------------------------------


def parse_due(raw: str):
    s = raw.strip().lower()
    if s in ("", "none", "null"):
        return None
    if s == "today":
        return today()
    if s == "tomorrow":
        return (date.today() + timedelta(days=1)).isoformat()
    d = normalize_date(raw)
    if d is None:
        fail(f'could not read "{raw}" as a date. Use YYYY-MM-DD.')
    return d


def clean_tags(raw) -> list:
    out = []
    for piece in (raw or "").split(","):
        t = piece.strip().lstrip("#")
        if t and t not in out:
            out.append(t)
    return out


def read_description(args):
    if args.description_file is not None:
        if args.description_file == "-":
            return sys.stdin.read()
        return Path(args.description_file).read_text(encoding="utf-8")
    return args.description


def require_vault(root: Path) -> None:
    if not root.is_dir():
        fail(
            f"the vault folder does not exist: {root}\n"
            "Pick a vault in the yatta app, or pass --vault DIR or set $YATTA_VAULT."
        )


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------


def cmd_vault(root, args):
    tasks = load_all(root) if root.is_dir() else []
    active = [t for t in tasks if not t.get("archived")]
    emit({
        "path": str(root),
        "exists": root.is_dir(),
        "today": today(),
        "todo": sum(t["status"] == "todo" for t in active),
        "doing": sum(t["status"] == "doing" for t in active),
        "done": sum(t["status"] == "done" for t in active),
        "archived": len(tasks) - len(active),
    })


def cmd_list(root, args):
    require_vault(root)
    now = today()
    week_end = (date.today() + timedelta(days=7)).isoformat()
    tag = args.tag.lstrip("#") if args.tag else None
    needle = (args.search or "").strip().lower()

    def keep(t):
        if t.get("archived") and not args.include_archived:
            return False
        if args.status == "open" and t["status"] == "done":
            return False
        if args.status not in ("open", "any") and t["status"] != args.status:
            return False
        if tag and tag not in t["tags"]:
            return False
        d = t["due"]
        if args.due == "overdue" and not (d and d < now and t["status"] != "done"):
            return False
        if args.due == "today" and d != now:
            return False
        if args.due == "week" and not (d and now <= d <= week_end):
            return False
        if args.due == "upcoming" and not (d and d >= now):
            return False
        if args.due == "none" and d:
            return False
        if needle and needle not in t["title"].lower() and needle not in t["description"].lower():
            return False
        return True

    tasks = [t for t in load_all(root) if keep(t)]
    # Soonest deadline first, undated last; then by priority, then age.
    tasks.sort(key=lambda t: (t["due"] is None, t["due"] or "", PRIORITY_RANK[t["priority"]], t["created"]))
    emit({
        "vault": str(root),
        "today": now,
        "count": len(tasks),
        "tasks": [summary(t) for t in tasks[: args.limit]],
    })


def cmd_tags(root, args):
    require_vault(root)
    counts = {}
    for t in load_all(root):
        if t.get("archived"):
            continue
        for tag in t["tags"]:
            counts[tag] = counts.get(tag, 0) + 1
    emit(dict(sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))))


def cmd_get(root, args):
    require_vault(root)
    emit(full(root, find_task(root, args.ref)))


def cmd_add(root, args):
    title = single_line(args.title or "")
    if not title:
        fail("add needs a non-empty --title")
    task = {
        "id": new_id(),
        "title": title,
        "status": "todo",
        "priority": args.priority or "none",
        "due": parse_due(args.due) if args.due else None,
        "tags": clean_tags(args.tags),
        "created": today(),
        "completed": None,
        "description": read_description(args) or "",
        "path": "",
    }
    if args.status:
        apply_status(task, args.status)
    if args.dry_run:
        print(f"# would write {unique_path(root, slugify(title))}\n")
        print(render_task(task), end="")
        return
    require_vault(root)
    save_task(root, task)
    emit(full(root, task))


def cmd_update(root, args):
    require_vault(root)
    task = find_task(root, args.ref)
    if args.title is not None and args.title.strip():
        task["title"] = single_line(args.title)
    description = read_description(args)
    if description is not None:
        task["description"] = description
    if args.priority:
        task["priority"] = args.priority
    if args.due is not None:
        task["due"] = parse_due(args.due)
    if args.tags is not None:
        task["tags"] = clean_tags(args.tags)
    for t in clean_tags(args.add_tags):
        if t not in task["tags"]:
            task["tags"].append(t)
    drop = clean_tags(args.remove_tags)
    task["tags"] = [t for t in task["tags"] if t not in drop]
    if args.status and args.status != task["status"]:
        apply_status(task, args.status)
    if args.dry_run:
        print(f"# would rewrite {root / task['path']}\n")
        print(render_task(task), end="")
        return
    save_task(root, task)
    emit(full(root, task))


def cmd_complete(root, args):
    require_vault(root)
    task = find_task(root, args.ref)
    if task["status"] != "done":
        apply_status(task, "done")
        save_task(root, task)
    emit(full(root, task))


def cmd_archive(root, args):
    require_vault(root)
    task = find_task(root, args.ref)
    if not task.get("archived"):
        move(root, task, root / ARCHIVE_DIR)
        task["archived"] = True
    emit(summary(task))


def cmd_restore(root, args):
    require_vault(root)
    task = find_task(root, args.ref)
    if task.get("archived"):
        move(root, task, root)
        task.pop("archived", None)
    emit(summary(task))


def main() -> None:
    p = argparse.ArgumentParser(prog="yatta.py", description="Read and write a yatta vault.")
    p.add_argument("--vault", help="vault directory (default: $YATTA_VAULT, then the app's settings)")
    sub = p.add_subparsers(dest="command", required=True)

    sub.add_parser("vault", help="where the vault is, and task counts")
    sub.add_parser("tags", help="tags in use, most used first")

    ls = sub.add_parser("list", help="list tasks (open, unarchived by default)")
    ls.add_argument("--status", default="open", choices=("open", "any") + STATUSES)
    ls.add_argument("--tag")
    ls.add_argument("--due", default="any", choices=("overdue", "today", "week", "upcoming", "none", "any"))
    ls.add_argument("--search")
    ls.add_argument("--include-archived", action="store_true")
    ls.add_argument("--limit", type=int, default=200)

    for name in ("get", "complete", "archive", "restore"):
        sub.add_parser(name).add_argument("ref", help="task id or vault-relative path")

    def fields(sp, creating):
        sp.add_argument("--title", required=creating)
        sp.add_argument("--description", default="" if creating else None)
        sp.add_argument("--description-file", help='read the description from a file, "-" for stdin')
        sp.add_argument("--status", choices=STATUSES)
        sp.add_argument("--priority", choices=PRIORITIES)
        sp.add_argument("--due", help="YYYY-MM-DD, today, tomorrow" + ("" if creating else ", or none to clear"))
        sp.add_argument("--tags", help="comma-separated" + ("" if creating else "; replaces the existing tags"))
        sp.add_argument("--dry-run", action="store_true", help="print the file instead of writing it")

    fields(sub.add_parser("add", help="create a task"), creating=True)
    up = sub.add_parser("update", help="change fields of a task")
    up.add_argument("ref", help="task id or vault-relative path")
    fields(up, creating=False)
    up.add_argument("--add-tags")
    up.add_argument("--remove-tags")

    args = p.parse_args()
    root = resolve_vault(args.vault)
    {
        "vault": cmd_vault,
        "tags": cmd_tags,
        "list": cmd_list,
        "get": cmd_get,
        "add": cmd_add,
        "update": cmd_update,
        "complete": cmd_complete,
        "archive": cmd_archive,
        "restore": cmd_restore,
    }[args.command](root, args)


if __name__ == "__main__":
    main()
