# yatta vault

Every task in this folder is one markdown file. There is no database, no index
and no lock file — this folder *is* the app's state. Edit these files by hand,
sync them with git or Nextcloud, or point an AI agent at them. The app watches
the folder and picks up changes as soon as they land on disk.

## Format

```markdown
---
id: m1k2j3h4abcd
title: Ship the v1 beta
status: todo
priority: high
due: 2026-09-15
tags: [work, release]
created: 2026-08-31
---

Cut the release branch, run the snap build, then post in #announce.

Anything below the frontmatter is the description. It's ordinary markdown,
so lists, links and code blocks all work.
```

## Fields

| Field       | Required | Values                                          |
|-------------|----------|-------------------------------------------------|
| `id`        | no       | Stable identifier. Generated if you omit it.     |
| `title`     | no       | Falls back to a `# Heading`, then the filename.  |
| `status`    | no       | `todo`, `doing`, `done`. Default `todo`.         |
| `priority`  | no       | `urgent`, `high`, `medium`, `low`, `none`.       |
| `due`       | no       | `YYYY-MM-DD`. Omit for no deadline.              |
| `tags`      | no       | `[a, b]` or a `-` list. Used for grouping.       |
| `created`   | no       | `YYYY-MM-DD`. Set on creation.                   |
| `completed` | no       | `YYYY-MM-DD`. Set when status becomes `done`.    |

## Writing a task as an agent

The minimum viable task is a file with a title in it:

```bash
echo '# Renew the TLS certificate' > ~/Documents/yatta/renew-tls-cert.md
```

That is a real task the moment it hits the disk. The app fills in the missing
fields the first time you edit it in the UI. To create a fully specified one,
write the frontmatter yourself — every field is optional, so include only what
you know.

To complete a task, set `status: done`. To reschedule one, change `due`.

## Conventions worth knowing

- **Filenames are stable.** The app names a file once, from the title, and then
  never renames it. Retitling a task edits the frontmatter, not the filename,
  so any path you're holding stays valid.
- **`archive/` is just a folder.** Archived tasks are moved there, never
  deleted. They're still ordinary task files.
- **`README.md` at the top level is skipped**, so this file isn't a task.
  A `README.md` in a subfolder *is* read as one.
- **Unknown frontmatter keys are preserved by you, not by the app.** If you add
  your own fields by hand, note that saving that task from the UI rewrites the
  frontmatter and drops keys it doesn't know about.
