---
name: yatta
description: >
  Reads and manages the user's yatta todo list, a folder of markdown files (one
  task per file) that the yatta app watches live. Lists, adds, updates,
  completes, archives and restores tasks through a bundled Python script that
  writes exactly what the app would. Use it whenever the user wants something
  captured as a task, asks what is on their plate, or wants a task changed.
  WHEN: add a task, remind me to, put that on my list, capture this as a todo,
  what's due today, what's overdue, show my tasks, what am I working on, mark
  that done, I finished X, move the deadline, reprioritise, tag these, archive
  completed tasks, turn these notes into tasks, yatta.
---

# yatta

yatta keeps every task as one markdown file with YAML frontmatter in a folder
the user owns: the *vault*. There is no database, index or lock file. The
folder is the app's whole state, and the app picks up every change as soon as it
lands on disk. Its name is lowercase; it is the Japanese やった ("did it!").

If the yatta MCP server's tools (`list_tasks`, `add_task`, ...) are available
in this session, prefer them; they do the same thing. This skill is for agents
that have a shell instead.

## The script

Run every operation through `scripts/yatta.py` in this skill's directory. It
needs only python3. It resolves the vault, generates ids, picks filenames,
quotes YAML, writes atomically and stamps `completed:`, all following the
app's own rules. Every command prints JSON.

```bash
Y="<this skill's directory>/scripts/yatta.py"

python3 "$Y" vault                        # where the vault is, counts per status
python3 "$Y" list                         # open tasks, soonest deadline first
python3 "$Y" list --due overdue           # also: today, week, upcoming, none
python3 "$Y" list --tag work --status doing
python3 "$Y" list --search invoice --status any --include-archived
python3 "$Y" tags                         # tags in use, most used first
python3 "$Y" get <ref>                    # one task, with its description

python3 "$Y" add --title "Renew the passport" --due 2026-12-01 \
    --priority high --tags admin,personal --description "Photos first."
python3 "$Y" update <ref> --due tomorrow --add-tags waiting
python3 "$Y" update <ref> --due none --remove-tags waiting
python3 "$Y" update <ref> --status doing
python3 "$Y" complete <ref>
python3 "$Y" archive <ref>                # reversible: restore <ref>
```

`<ref>` is a task's `id`, or its vault-relative `path` with or without `.md`.
Every result includes both. For a long or multi-line description, use
`--description-file -` and a heredoc rather than fighting shell quoting:

```bash
python3 "$Y" add --title "Plan the offsite" --description-file - <<'BODY'
Venue shortlist from Ana.

- [ ] book rooms
- [ ] send the agenda
BODY
```

`add` and `update` take `--dry-run` to print the file instead of writing it. Use
it when the user asked to see a task before it is saved.

The vault is found in this order: `--vault DIR` (placed before the command),
`$YATTA_VAULT`, the app's settings file (including the snap's), and finally
`~/Documents/yatta`. If `vault` reports `"exists": false`, say so and ask
where the tasks live. Don't create a vault in a guessed location.

## Turning a request into fields

| Field | How to fill it |
|---|---|
| `--title` | Short imperative phrase. Strip "remind me to", "I need to", "add a task to". Otherwise keep the user's wording. |
| `--description` | Everything that doesn't fit the title: context, links, names, steps. Omit it if the title says it all. Paste URLs bare. |
| `--due` | Only when the user gave a deadline. Resolve "friday", "next week", "in 3 days" against today's date to `YYYY-MM-DD` (`today` and `tomorrow` are accepted as-is). Dates are day-granular: keep a time of day in the description. |
| `--priority` | Only when signalled: urgent/asap/blocking → `urgent`; important → `high`; "when I get a chance" → `low`. Otherwise leave it out (`none`). |
| `--tags` | Run `tags` first and reuse an existing tag when one fits. A person's name is a fine tag for "waiting on / discuss with". |
| `--status` | Leave it out (`todo`). `doing` if the user has started; `done` to log something already finished. |

## Ground rules

- **Act, don't confirm.** If the user asked for a task, create it. Ask only when
  it is unclear *what* the task is, never to confirm a due date or priority you
  could simply leave out.
- **Omit what you weren't told.** A missing `due`, `priority` or tag is normal.
  A guessed one is noise the user has to clean up.
- **One task per file.** Split a request with several action items into several
  `add` calls; put them in one shell command, one line each.
- **Check before adding from a pasted list.** Run `list --status any` and match
  titles loosely (punctuation, curly quotes). If an item already exists, either
  skip it or `update` it with the new detail; don't add it twice. Report what
  you created, updated and skipped.
- **Don't do the work instead.** "Add a task to investigate X" means write the
  task, not investigate X.
- **Archive, don't delete.** Archiving moves the file into `archive/` and can
  be undone. Only delete a task file (with `rm`) when the user explicitly asks
  for deletion.
- **Never rename or move a task file yourself.** The filename is chosen once
  and stays put, because paths and links depend on it. Retitling changes the
  `title:` field, which `update --title` does.
- **Report briefly.** Give the title, due date and what changed, not the file
  contents.
- A result with `"conflicted": true` has git conflict markers in its
  description, left by a sync between devices. Point it out; don't resolve it
  unasked.

## The file format

What the script writes, and what to expect when you read a file directly:

```markdown
---
id: mtnb6ema1olk
title: Ship the v1 beta
status: todo
priority: high
due: 2026-09-15
tags: [work, release]
created: 2026-09-10
---

Free-form markdown description.
```

`status` is `todo`, `doing` or `done`; `priority` is `urgent`, `high`,
`medium`, `low` or `none`; dates are `YYYY-MM-DD`; `completed` is set when a
task becomes `done` and cleared when it reopens. Every field is optional. A
file holding only `# Some title` is already a valid task. The top-level
`README.md` is not a task, and `archive/` holds archived ones.

Reading files directly with `grep`, `cat` and so on is fine. To change a task,
go through the script. It rewrites the frontmatter in the order the app uses,
and like the app it drops frontmatter keys it doesn't know. Edit a file by
hand only when the script can't run, and keep the key order shown above.
