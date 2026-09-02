# yatta

**Yet Another Text-based TODO App.**
Also 「やった」: *"did it!"* — which is the part that matters.

A fast, colourful desktop todo list whose entire database is a folder of
markdown files you own.

There is no sync service, no account and no binary store. Each task is one
`.md` file with YAML frontmatter, which means your text editor, your git
history and any AI agent can all read and write your tasks without going
through an API.

## Why markdown files

The point of the format is that *nothing* needs yatta to be running. An agent
can add a task with a single `echo`. You can grep your backlog. You can commit
it, sync it with Nextcloud, or open it on a machine that has never heard of
this app.

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
```

Every field is optional. A file containing nothing but `# Renew the passport`
is a valid task. The full format is documented in the `README.md` that yatta
writes into your vault on first run.

## Stack

| Layer     | Choice                  | Why                                                        |
|-----------|-------------------------|------------------------------------------------------------|
| Shell     | Tauri 2 (Rust)          | ~10 MB bundles, native webview, one codebase for Linux/macOS/Windows |
| UI        | Svelte 5 + TypeScript   | No virtual DOM, tiny runtime, compiles away                 |
| Storage   | Markdown files          | Yours, greppable, agent-writable                            |

Dependencies are deliberately thin: the frontmatter parser, the icon set and
the date parser are all in-tree rather than pulled from a package, because each
is small, fixed in scope, and cheaper to own than to track.

## Building

### Linux (Ubuntu 24.04+)

```bash
sudo apt install -y build-essential pkg-config curl wget file libssl-dev \
  libxdo-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  cargo rustc npm

npm install
npm test             # parser unit tests
npm run app          # dev, with hot reload
npm run app:build    # .deb + AppImage into src-tauri/target/release/bundle/
```

### Portable build

`libayatana-appindicator3` is the only Linux-specific native dependency, and it
exists solely for the tray icon. Building without it drops the tray and the
global hotkey and needs nothing but webkit:

```bash
npm run app:build:portable    # tauri build --no-default-features
```

The settings panel hides the tray options automatically in that build.

### Snap

```bash
snapcraft pack
sudo snap install --dangerous ./yatta_0.1.0_amd64.snap
```

Strictly confined, core24, using the `gnome` extension. Only `git` is staged —
the GNOME platform snap already provides WebKitGTK, GTK3, librsvg and
libayatana-appindicator. See `SNAP_PACKAGING.md` for interface connections and
troubleshooting.

### macOS and Windows

No source changes are needed. Install Rust and Node, then `npm install &&
npm run app:build`. The tray and hotkey work on both without the appindicator
dependency, and git auto-commit shells out to whatever `git` is on `PATH`.

## Features

- **Quick add that parses what you type.** `Send the report tomorrow !high #work`
  becomes a task with a deadline, a priority and a tag. `@`-prefixed dates
  (`@friday`, `@15 sep`, `@2026-09-15`) work anywhere in the sentence; bare date
  words are only read at the end, so "Plan the friday standup" keeps its title.
- **New tasks announce themselves.** A task that has just arrived glows for a
  few seconds, and a message names it with a link straight into its details —
  quick add captures a title in a hurry, and that is the moment you still
  remember what else you meant to write down. Completing a task offers an undo.
  All of it works however the task got there: the quick-add box, the tray
  popup, the importer, or a file an agent wrote into the vault.
- **Right-click a task** for open, complete, archive, delete, and reaching the
  markdown file itself. Archiving and deleting are both undoable — deleting
  snapshots the task first, so undo puts the file back exactly where it was.
- **Live external-edit sync.** The vault is watched. Edit a file in vim or have
  an agent write one and the list updates as it lands.
- **Tags, grouping and smart views.** All tasks (the default), Today, Upcoming,
  No deadline, Done, Archive, plus a view per tag. Group by tag, priority or
  deadline. Tag colours are derived from the tag name, so they're stable
  everywhere without being stored.
- **List or kanban.** The same tasks, either as a list or as a To do / In
  progress / Done board. Drag a card between columns to change its status, or
  focus a card and use the left/right arrow keys. The board always shows a Done
  column regardless of the "show completed inline" setting.
- **Bulk import, one task per line.** Paste a list, a markdown checklist or a
  numbered agenda. Every line runs through the same parser as quick add, so
  `!high`, `#tag` and dates work per line. `- [x]` imports as already done,
  `# Headings` become tags, and indented lines attach as the description. A
  multi-line paste into the quick-add box opens the importer automatically.
- **Runs in the tray, and starts with your session.** Both optional. Closing the
  window can keep yatta running and reachable from the tray, and it can add
  itself to your login items — written to the right place whether it is a snap
  or not.
- **Quick-add popup.** The tray item and the global hotkey open a small
  always-on-top window with one field, rather than trying to raise the main
  window — which Wayland does not permit a client to do for itself. Capture
  works whether the app is minimised, on another workspace, or closed.
- **Calendar.** A month grid answering two questions: what is due on a day
  ahead, and — the one that is hard to answer anywhere else — what you actually
  finished on a day behind. Every task file already records `completed:`, so the
  history is real rather than reconstructed. Archived tasks still count, because
  archiving is only a file move and tidying up should not erase what you did.
- **Share this view.** One click copies the list you are looking at as markdown,
  in the same `@date !priority #tag` syntax quick add accepts — so the person
  you send it to can paste it straight into their importer and get the tasks,
  not just a picture of them.
- **Interactive checklists.** `- [ ] step one` in a description renders as a
  real checkbox you can tick. Ticking rewrites that line in the file, so the
  markdown stays the source of truth and an agent reading it sees what you see.
  Rows show progress as `2/5`.
- **Deadline reminders.** A desktop notification listing what is overdue or due
  today, once or twice a day at times you choose. Nothing is sent when nothing
  is due, and a reminder more than two hours stale is dropped rather than
  delivered late.
- **Optional git auto-commit.** Coalesced a few seconds after you stop typing,
  via the `git` binary. Off unless the vault is a repo and you enable it.
- **Optional tray icon and global hotkey**, behind a compile-time feature.
- **Agent prompt.** The detail panel composes a ready-to-paste prompt containing
  the task, its metadata and its file path — the seam for the planned
  "hand this to an agent" action.

### Keyboard

| Key           | Action                          |
|---------------|---------------------------------|
| `N`           | New task                        |
| `/`           | Search                          |
| `J` / `K`     | Move between tasks (or `↑`/`↓`) |
| `X`           | Complete the selected task      |
| `E`           | Open the selected task          |
| `←` / `→`     | Move a card between columns (board) |
| `Esc`         | Close panel / clear search      |
| `Ctrl` `R`    | Reload from disk                |

## How archiving works

Archiving is a **file move**, never a delete. `Archive N completed` (in the
grouping menu) moves each completed task's `.md` file into an `archive/`
subfolder of your vault:

```
~/Documents/yatta/
├── ship-the-v1-beta.md          <- active
├── renew-the-passport.md        <- active
└── archive/
    ├── review-pr-482.md         <- archived, still a normal task file
    └── update-readme-badges.md
```

Nothing about the file's contents changes — a task is archived purely by virtue
of where it sits, so `archived` is derived from the path and never written into
the frontmatter. That keeps the property honest: move a file into `archive/` in
your file manager and it *is* archived; move it back out and it isn't.

Archived tasks are excluded from every view, tag count and search except the
Archive view, which appears in the sidebar once the folder is non-empty. Opening
one gives you a **Restore** button that moves the file back to the vault root.
Because it is only ever a move, `git log` shows an archive as a rename and
nothing is ever lost.

## Layout

```
src/                    Svelte frontend
  lib/quickadd.ts       natural-language parser
  lib/importer.ts       bulk "one task per line" parser
  lib/store.svelte.ts   app state (Svelte 5 runes)
  lib/components/       UI
src-tauri/src/
  task.rs               markdown <-> task, frontmatter parser
  vault.rs              scan, atomic save, archive
  watcher.rs            debounced filesystem watching
  git.rs                optional auto-commit
  tray.rs               optional tray + hotkey (feature-gated)
```

## Where your tasks live

On first run yatta asks, and creates nothing until you have chosen. The
suggested default is `~/Documents/yatta`; the picker lets you put it anywhere.

- An **empty or new folder** is seeded with a README documenting the file format
  and a couple of example tasks.
- A folder that **already contains markdown** is adopted untouched — no README,
  no examples. Point it at an existing notes folder and it just reads them.
- Emptying the vault by hand does not resurrect the examples.

You can move the vault later in Settings. Moving it does not move your files;
it changes which folder the app reads.

## Roadmap

Feature requests and known gaps live in [ROADMAP.md](ROADMAP.md).

## Known limitations

- **Global hotkeys need X11.** Under Wayland the compositor owns shortcuts;
  registration fails, the app logs it and carries on. Bind a shortcut to the
  `yatta` binary in your desktop's keyboard settings instead.
- **Saving a task rewrites its frontmatter.** Custom keys you added by hand are
  not preserved through an edit made in the UI.
- **Filenames are fixed at creation.** Retitling a task changes `title:`, not
  the filename, so paths that agents hold stay valid.

## License

MIT — see [LICENSE](LICENSE).

Use it, change it, ship it commercially. The one condition is that the
copyright notice and permission notice travel with the code, so credit stays
attached to it.
