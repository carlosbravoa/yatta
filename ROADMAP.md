# Roadmap

Where feature requests land so they aren't lost in a chat log. Newest requests go
under **Next up**; move things to **Done** with the date rather than deleting them.

---

## Next up

### Undo (Ctrl+Z) for accidental task changes

**Requested:** 2026-08-31

Clicking a checkbox by mistake currently means finding the task again, unticking
it, and hoping nothing re-sorted underneath you in the meantime. It should take
one keystroke to put things back.

**Scope, in priority order:**

1. **Completing a task** — the reported case, and the easiest to hit by accident
   because the row re-sorts or disappears from the view the instant you click.
2. Deleting a task.
3. Field edits made in the detail panel (title, deadline, priority, tags).
4. Archive / restore.

**Behaviour:**

- `Ctrl+Z` undoes the most recent change; repeated presses walk back through a
  stack. `Ctrl+Shift+Z` redoes.
- The toast that already appears after a destructive action grows an **Undo**
  button, so the affordance is discoverable without knowing the shortcut.
- The stack is per-session and in-memory. It is not persisted, and it does not
  try to undo changes made outside the app.

**Implementation notes:**

- The store gets an undo stack of inverse operations. Because every task is a
  file, "undo" is mostly "write the previous version back" — snapshot the task
  before mutating and the inverse is a `save_task` of that snapshot.
- Deletion is the exception: the file is gone, so the snapshot must carry the
  full task *including its body and path* to recreate it. Restoring to the same
  path matters — anything holding that path (an agent, a link) should stay valid.
- Watch the interaction with the file watcher: an undo is a self-write and must
  not bounce back through `vault-changed` as an external edit.
- An external edit landing between a change and its undo makes the inverse stale.
  Prefer detecting that and dropping the entry over silently clobbering the
  user's other edit.

### Do not retry: keeping the quick-add popup resident

**Tried and reverted, 2026-09-02.** Building the popup means building a webview,
which is most of the app's startup cost paid again for a box you type one line
into, so keeping it hidden between uses looks like free speed. It is not.

On Wayland a client cannot focus itself; only a newly mapped window is granted
focus by the compositor. A re-shown popup appears **without the caret**, so the
first thing typed goes to whatever had focus before. A capture box you have to
click first is worse than one that takes half a second to appear, so the trade
is not close.

This is the same restriction that forces the main window to be rebuilt rather
than hidden, and it is not specific to decorated windows -- that was the wrong
theory. Do not revisit without a way to request an activation token, which a
tray click or a global shortcut does not currently provide.

### Parked: scroll a newly-arrived task into view

**Attempted 2026-09-02, reverted.** The glow added that day works, but if the
new task is below the fold you never see it. Several attempts to scroll to it
all failed. Recording the dead ends so the next attempt starts further along.

What is already known to work: the store correctly identifies the new task, and
the effect fires and finds the right row in the DOM. Measured, not assumed --
`rows=15 overflow=232px rowTop=822 viewport=0..673 alreadyVisible=false`. So the
task is genuinely off screen in a scrollable container, and detection is fine.

What failed, in order:

1. `scrollIntoView({ block: "nearest" })` — no effect at all. WebKitGTK appears
   to ignore the options object, the same way it ignores `field-sizing`.
2. Computing the delta and calling `scroller.scrollTo({ top, behavior })` — also
   nothing. Suspicion: if the dictionary form is unsupported it degrades to
   `scrollTo(0, 0)`, i.e. scrolls to the top, which is indistinguishable from
   doing nothing when already there.
3. Assigning `scroller.scrollTop = before + delta` directly, with smoothness
   moved to CSS `scroll-behavior`. Reported `delta=260 before=0 after=0
   max=364` — the container had room and refused to move. Note the read-back is
   unreliable under smooth scrolling, so `after=0` may be an artefact rather
   than the failure itself.
4. `input.focus({ preventScroll: true })` in QuickAdd, on the theory that
   refocusing the box (which sits at the top) was yanking the viewport back and
   undoing the scroll. This would have explained why arrivals from the
   filesystem behaved differently from ones typed into the box. Did not fix it.

**Worth trying next:** confirm which element actually scrolls, rather than
assuming it is `main` — a wrapper or `.content` may be the real scroller, in
which case every attempt above was scrolling the wrong node. `document.
scrollingElement` and walking up from the row via `scrollHeight > clientHeight`
would settle it. Also worth testing scrolling in a plain WebKitGTK page to
establish what the engine supports at all, separately from this app.

### Bug: Ctrl+Z does not undo typing inside a text field

**Reported:** 2026-08-31

Editing a task's title in the detail panel, making a typo, and pressing `Ctrl+Z`
does not restore the previous text. Ordinary text-editing undo is missing.

This is **not** the same as the task-level undo above and will not be fixed by
it. That one reverses a *task state change* (a task got completed, deleted,
retitled). This one is the browser's native per-field undo history for
characters you are typing right now. A user pressing `Ctrl+Z` mid-edit means the
second, and would be startled if it reverted a different task instead.

**Likely cause:** the title and description are `bind:value` textareas whose
value is reassigned by Svelte, and the debounced autosave writes back into the
same draft. Programmatically setting `.value` discards the browser's native undo
stack for that element, so by the time the user presses `Ctrl+Z` there is
nothing left to undo.

**Worth checking before fixing:** whether the window-level `keydown` handler in
`App.svelte` is swallowing the event. It should not be - it returns early when
the target is an input and when a modifier is held - but confirm rather than
assume, because that would be the cheap fix.

**Likely real fix:** stop clobbering the native undo stack. Either update state
from the input event without writing back to the element, or maintain a small
per-field undo history and handle `Ctrl+Z` explicitly while a field has focus.

**Interaction with task-level undo:** whichever lands second must respect focus.
`Ctrl+Z` with the cursor in a text field means "undo my typing"; `Ctrl+Z` with
focus in the list means "undo what I just did to that task". Getting this
backwards is worse than having neither.

---

## Later

### Hand a task to an AI agent

The seam exists: the detail panel composes a full prompt (title, metadata,
absolute file path, and an instruction to set `status: done` when finished) and
copies it to the clipboard. The remaining work is choosing what to actually
invoke and how to surface progress and results on the task.

### Recurring tasks

`repeat: weekly` in the frontmatter, re-spawning the task on completion. Needs a
decision on whether the recurrence lives in one file that moves its `due` date
forward, or spawns a new file per occurrence — the second is friendlier to git
history and to agents, at the cost of more files.

### Tracked upstream: tray uses the deprecated appindicator library

**Noted:** 2026-08-31

Every launch prints `libayatana-appindicator is deprecated. Please use
libayatana-appindicator-glib in newly written code.` The successor is real and
packaged (`libayatana-appindicator-glib2` in Ubuntu), but yatta cannot adopt
it, because we do not choose the library:

```
yatta -> tauri 2 -> tray-icon 0.24.2 -> libappindicator 0.9.0
                                      -> libappindicator-sys 0.9.0
```

`libappindicator-sys` dlopens a hardcoded list - `libayatana-appindicator3.so.1`,
`libappindicator3.so.1`, then the unversioned fallbacks. The glib variant is not
among them, and `tray-icon` exposes no backend choice: its only Linux feature is
`gtk`, which pulls `libappindicator` in unconditionally.

Adopting the successor is therefore an upstream change across three crates, and
not a rename: the glib version is a GLib-2.0-only reimplementation that drops
the GTK3 dependency, so the menu handling differs from what `muda` builds today.
The library is also absent from the `gnome-46-2404` platform snap, so the snap
would additionally have to stage it.

**Action:** none available to us. Revisit when `tray-icon` gains support. The
warning is cosmetic - the tray itself works.

**If the line must go sooner,** the only lever on our side is a GLib log writer
(`g_log_set_writer_func`) filtering the `libayatana-appindicator` domain. That
is roughly 20 lines plus a direct `glib` dependency, and it installs a global
log filter to suppress one third-party message. Defensible, since the warning is
addressed to developers of that library rather than to our users, who can do
nothing about it - but it is a hack, and it is not currently done.

---

## Known gaps

- **`snapcore/action-build` still targets Node 20**, which GitHub has deprecated.
  Our own actions are current; this is the last one warning, and there is no
  newer version to move to — `v1.3.0` is the latest tag and declares
  `using: 'node20'`. GitHub currently forces it onto Node 24, so it works, but
  it will break when that shim is removed. Replacing it means hand-rolling
  snapcraft and LXD setup in the workflow, which trades a harmless warning for
  fragility in the one part of the pipeline that has been reliable. Better
  resolved upstream, in Canonical's own repository.

- **`sort_by: "manual"` is dead code.** It is in the TypeScript `Settings` type,
  has no case in `compare()`, and is not offered in the sort menu. Either
  implement drag-to-reorder in the list or delete it from the type — as it
  stands the type promises something the code does not do.
- **The frontend store is untested.** `quickadd`, `importer` and `checklist` have
  unit tests; the store's filtering, grouping and board logic do not.
- ~~No LICENSE file.~~ Resolved 2026-08-31: MIT.
- **Global hotkeys do not work under Wayland.** The compositor owns shortcuts;
  this is not fixable in the app. Documented in the README.
- **Saving a task from the UI rewrites its frontmatter**, dropping custom keys
  added by hand.
- **macOS is unimplemented.** Launch-at-login returns an explicit error there,
  and nothing has been built or tested on it. Windows builds in CI; macOS would
  need a Mac for signing and notarisation.

---

## Done

- **2026-09-02** — 0.7.0: the quick-add popup takes an optional description,
  and its title now reads as an editable field rather than blending into the
  card.

- **2026-09-02** — 0.6.1: reverted the resident quick-add popup. It shipped in
  0.6.0 and broke typing on the second open.

- **2026-09-02** — 0.6.0: task context menu with per-task archive (which did
  not exist before — archiving was all-or-nothing and buried in the grouping
  menu), undoable delete and archive, resident quick-add popup, and picking a
  sidebar view clears the search.

- **2026-09-02** — 0.5.0: arrivals and completions are announced. The creation
  message links into the task's details; completing offers an undo. Sidesteps
  the parked scroll problem: a message is visible wherever the row landed.

- **2026-09-02** — Newly-arrived tasks glow briefly, detected in the store so
  it covers every route in, including files written straight into the vault.

- **2026-09-02** — 0.4.1: startup roughly 3x faster. First paint went from
  ~1.6s to ~0.53s by trimming the font stack; see SNAP_PACKAGING.md for how
  to re-measure with `YATTA_TIMING=1`.

- **2026-09-01** — 0.4.0: About window and settings section, optional
  close-to-tray, and optional launch at login.

- **2026-09-01** — CI on GitHub Actions: tests plus native amd64 and arm64
  snap builds on every push. Publishing stays manual.

- **2026-09-01** — 0.3.0: quick-add popup window from the tray and the global
  hotkey, replacing a menu item that duplicated "open the app".

- **2026-08-31** — 0.2.0: calendar view. Month grid of what is due ahead and
  what was completed behind, archived tasks included.
- **2026-08-31** — Long task titles wrap in the details panel instead of being
  clipped, with newlines folded so they cannot corrupt the frontmatter.

- **2026-08-31** — 0.1.1: list view reflowed — wider column, and rows are
  rules-and-padding rather than individual cards.

- **2026-08-31** — Renamed to yatta (やった, "did it!"), MIT licensed.
- **2026-08-31** — Task detail panel is resizable (drag, arrow keys, or
  double-click to reset); the width persists.
- **2026-08-31** — "Share this view": copies the current list as markdown in
  quick-add syntax, so the export round-trips through the importer.

- **2026-08-31** — Interactive checklists in task descriptions, with progress
  shown on the row.
- **2026-08-31** — Deadline reminders: desktop notification once or twice a day
  at configurable times, silent when nothing is due.
- **2026-08-31** — Keyboard navigation: `J`/`K` to move, `X` to complete, `E` to
  open, arrows to move a board card.

- **2026-08-31** — Kanban board view, drag or arrow keys to change status.
- **2026-08-31** — Bulk importer, one task per line, with markdown checklist and
  heading-as-tag handling.
- **2026-08-31** — Archive as a file move into `archive/`, with restore.
- **2026-08-31** — First-run picker for where the vault lives; creates nothing
  until you choose.
- **2026-08-31** — Snap packaging (strict confinement, core24, gnome extension).
