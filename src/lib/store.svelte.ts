import { api } from "./api";
import { mark } from "./timing";
import { daysUntil, todayISO } from "./dates";
import { PRIORITY_RANK, type Settings, type Status, type Task } from "./types";

export type ViewId =
  | "all"
  | "today"
  | "upcoming"
  | "nodate"
  | "done"
  | "archived"
  | `tag:${string}`;

export interface Column {
  id: Status;
  label: string;
  tasks: Task[];
}

export interface Group {
  key: string;
  label: string;
  tasks: Task[];
}

const DEFAULT_SETTINGS: Settings = {
  vault_path: "",
  theme: "system",
  group_by: "none",
  layout: "list",
  sort_by: "due",
  show_done: false,
  git_autocommit: false,
  tray_enabled: true,
  hotkey: "CmdOrCtrl+Shift+Space",
  first_run_done: false,
  close_to_tray: false,
  autostart: false,
  detail_width: 380,
  reminders_enabled: true,
  reminder_times: ["09:00"],
  last_reminder: "",
};

function matchesQuery(task: Task, q: string): boolean {
  if (!q) return true;
  const needle = q.toLowerCase();
  return (
    task.title.toLowerCase().includes(needle) ||
    task.description.toLowerCase().includes(needle) ||
    task.tags.some((t) => t.includes(needle))
  );
}

/** Dated tasks first, soonest deadline first; undated sink to the bottom. */
function dueOrder(a: Task, b: Task): number {
  if (a.due && b.due) return a.due.localeCompare(b.due);
  if (a.due) return -1;
  if (b.due) return 1;
  return 0;
}

function compare(a: Task, b: Task, sortBy: Settings["sort_by"]): number {
  switch (sortBy) {
    case "priority":
      return (
        PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority] ||
        dueOrder(a, b) ||
        a.title.localeCompare(b.title)
      );
    case "title":
      return a.title.localeCompare(b.title);
    case "created":
      return b.created.localeCompare(a.created);
    case "due":
    default:
      return (
        dueOrder(a, b) ||
        PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority] ||
        a.title.localeCompare(b.title)
      );
  }
}

function dueBucket(task: Task): string {
  if (!task.due) return "none";
  const d = daysUntil(task.due);
  if (d < 0) return "overdue";
  if (d === 0) return "today";
  if (d <= 7) return "week";
  return "later";
}

const DUE_LABELS: Record<string, string> = {
  overdue: "Overdue",
  today: "Today",
  week: "This week",
  later: "Later",
  none: "No deadline",
};

function labelFor(mode: string, key: string): string {
  if (mode === "tag") return key === " untagged" ? "Untagged" : `#${key}`;
  if (mode === "priority") {
    return key === "none" ? "No priority" : key[0].toUpperCase() + key.slice(1);
  }
  return DUE_LABELS[key] ?? key;
}

class Store {
  tasks = $state<Task[]>([]);
  settings = $state<Settings>({ ...DEFAULT_SETTINGS });
  vaultPath = $state("");
  isGitRepo = $state(false);
  supportsTray = $state(true);

  view = $state<ViewId>("all");
  query = $state("");
  loading = $state(true);
  error = $state<string | null>(null);
  toast = $state<string | null>(null);

  /** Vault-relative path of the task open in the detail panel. */
  openPath = $state<string | null>(null);
  showSettings = $state(false);
  showImport = $state(false);
  /** Text the importer opens pre-filled with (e.g. a multi-line paste). */
  importText = $state("");

  private toastTimer: ReturnType<typeof setTimeout> | undefined;

  // -- Derived --------------------------------------------------------------

  // Archived tasks are excluded everywhere except the Archive view. They are
  // still ordinary files -- `archive/` is just a folder.
  open = $derived(this.tasks.filter((t) => !t.archived && t.status !== "done"));
  done = $derived(this.tasks.filter((t) => !t.archived && t.status === "done"));
  archivedTasks = $derived(this.tasks.filter((t) => t.archived));

  counts = $derived({
    all: this.open.length,
    today: this.open.filter((t) => t.due !== null && daysUntil(t.due) <= 0).length,
    upcoming: this.open.filter((t) => t.due !== null && daysUntil(t.due) > 0).length,
    nodate: this.open.filter((t) => t.due === null).length,
    done: this.done.length,
    archived: this.archivedTasks.length,
  });

  /** True until the user has chosen where their tasks should live. */
  needsOnboarding = $derived(!this.settings.first_run_done);

  overdue = $derived(
    this.open.filter((t) => t.due !== null && daysUntil(t.due) < 0).length
  );

  /** Every tag in use, most-used first, with its open-task count. */
  tags = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const task of this.open) {
      for (const tag of task.tags) counts.set(tag, (counts.get(tag) ?? 0) + 1);
    }
    return [...counts.entries()]
      .map(([name, count]) => ({ name, count }))
      .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
  });

  /** Tasks belonging to the current view.
   *
   *  `includeDone` is forced on for the board, which needs a Done column to
   *  drag into whatever the "show completed inline" setting says about lists.
   */
  private pool(includeDone: boolean): Task[] {
    const view = this.view;
    if (view === "archived") return this.archivedTasks;
    if (view === "done") return this.done;

    const base =
      includeDone || this.settings.show_done ? [...this.open, ...this.done] : this.open;

    if (view.startsWith("tag:")) {
      const tag = view.slice(4);
      return base.filter((t) => t.tags.includes(tag));
    }
    if (view === "today") return base.filter((t) => t.due !== null && daysUntil(t.due) <= 0);
    if (view === "upcoming") return base.filter((t) => t.due !== null && daysUntil(t.due) > 0);
    if (view === "nodate") return base.filter((t) => t.due === null);
    return base;
  }

  private arrange(tasks: Task[]): Task[] {
    return tasks
      .filter((t) => matchesQuery(t, this.query))
      .sort((a, b) => compare(a, b, this.settings.sort_by));
  }

  visible = $derived.by(() => this.arrange(this.pool(false)));

  /** Every task the search box matches -- archived and completed included.
   *  The calendar narrows by date rather than by the sidebar view, and its
   *  whole point is the record of finished work, so nothing is excluded. */
  matching = $derived(this.tasks.filter((t) => matchesQuery(t, this.query)));

  boardColumns = $derived.by((): Column[] => {
    const tasks = this.arrange(this.pool(true));
    return [
      { id: "todo", label: "To do", tasks: tasks.filter((t) => t.status === "todo") },
      { id: "doing", label: "In progress", tasks: tasks.filter((t) => t.status === "doing") },
      { id: "done", label: "Done", tasks: tasks.filter((t) => t.status === "done") },
    ];
  });

  groups = $derived.by((): Group[] => {
    const tasks = this.visible;
    const mode = this.settings.group_by;
    if (mode === "none" || tasks.length === 0) {
      return [{ key: "all", label: "", tasks }];
    }

    const buckets = new Map<string, Task[]>();
    const push = (key: string, task: Task) => {
      const list = buckets.get(key);
      if (list) list.push(task);
      else buckets.set(key, [task]);
    };

    for (const task of tasks) {
      if (mode === "tag") {
        // A task with several tags appears under each of them.
        if (task.tags.length === 0) push(" untagged", task);
        else for (const tag of task.tags) push(tag, task);
      } else if (mode === "priority") {
        push(task.priority, task);
      } else {
        push(dueBucket(task), task);
      }
    }

    const order =
      mode === "priority"
        ? ["urgent", "high", "medium", "low", "none"]
        : mode === "due"
          ? ["overdue", "today", "week", "later", "none"]
          : [...buckets.keys()].sort((a, b) => a.localeCompare(b));

    return order
      .filter((key) => buckets.has(key))
      .map((key) => ({ key, label: labelFor(mode, key), tasks: buckets.get(key)! }));
  });

  // -- Actions --------------------------------------------------------------

  async init() {
    this.loading = true;
    try {
      mark("init:start");
      this.settings = await api.getSettings();
      mark("init:settings");
      const info = await api.vaultInfo();
      mark("init:vault-info");
      this.vaultPath = info.path;
      this.isGitRepo = info.is_git_repo;
      this.supportsTray = info.supports_tray;

      // Skip the first-run picker only for someone who has genuinely used
      // yatta before: a settings file exists but predates the picker, so
      // asking again would re-pose a question they already answered.
      //
      // `info.exists` alone is not enough. On a fresh install a folder may
      // simply happen to sit at the default vault path, and silently adopting
      // it would hand the user a vault they never chose.
      if (!this.settings.first_run_done && info.had_settings && info.exists) {
        await this.updateSettings({ first_run_done: true });
      }

      mark("init:pre-reload");
      await this.reload();
      mark("init:tasks");
      this.error = null;
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loading = false;
    }
  }

  async reload() {
    try {
      this.tasks = await api.listTasks();
      this.error = null;
    } catch (e) {
      this.error = String(e);
    }
  }

  async save(task: Task): Promise<Task | null> {
    try {
      const saved = await api.saveTask($state.snapshot(task) as Task);
      const idx = this.tasks.findIndex((t) => t.path === saved.path);
      if (idx >= 0) this.tasks[idx] = saved;
      else this.tasks.push(saved);
      return saved;
    } catch (e) {
      this.notify(String(e));
      return null;
    }
  }

  async toggle(task: Task) {
    const next = task.status === "done" ? "todo" : "done";
    const idx = this.tasks.findIndex((t) => t.path === task.path);

    // Optimistic: the checkbox must feel instant; disk catches up after.
    if (idx >= 0) {
      this.tasks[idx] = {
        ...this.tasks[idx],
        status: next,
        completed: next === "done" ? todayISO() : null,
      };
    }
    try {
      const saved = await api.setStatus(task.path, next);
      if (idx >= 0) this.tasks[idx] = saved;
    } catch (e) {
      this.notify(String(e));
      await this.reload();
    }
  }

  async remove(task: Task) {
    try {
      await api.deleteTask(task.path, task.title);
      this.tasks = this.tasks.filter((t) => t.path !== task.path);
      if (this.openPath === task.path) this.openPath = null;
      this.notify(`Deleted "${task.title}"`);
    } catch (e) {
      this.notify(String(e));
    }
  }

  /** Bulk create from the importer: one backend call, one git commit. */
  async importTasks(drafts: Task[]): Promise<number> {
    try {
      const created = await api.createTasks($state.snapshot(drafts) as Task[]);
      this.tasks.push(...created);
      this.notify(`Imported ${created.length} task${created.length === 1 ? "" : "s"}`);
      return created.length;
    } catch (e) {
      this.notify(String(e));
      return 0;
    }
  }

  /** Move a task back out of `archive/`. */
  async restore(task: Task) {
    try {
      const restored = await api.restoreTask(task.path);
      const idx = this.tasks.findIndex((t) => t.path === task.path);
      if (idx >= 0) this.tasks[idx] = restored;
      else this.tasks.push(restored);
      if (this.openPath === task.path) this.openPath = restored.path;
      this.notify(`Restored "${restored.title}"`);
    } catch (e) {
      this.notify(String(e));
    }
  }

  /** Board drag-and-drop target. */
  async setStatus(task: Task, status: Status) {
    if (task.status === status) return;
    const idx = this.tasks.findIndex((t) => t.path === task.path);
    if (idx >= 0) {
      this.tasks[idx] = {
        ...this.tasks[idx],
        status,
        completed: status === "done" ? todayISO() : null,
      };
    }
    try {
      const saved = await api.setStatus(task.path, status);
      if (idx >= 0) this.tasks[idx] = saved;
    } catch (e) {
      this.notify(String(e));
      await this.reload();
    }
  }

  async archive() {
    try {
      const moved = await api.archiveDone();
      await this.reload();
      this.notify(
        moved === 0 ? "Nothing to archive" : `Archived ${moved} task${moved === 1 ? "" : "s"}`
      );
    } catch (e) {
      this.notify(String(e));
    }
  }

  /** Returns false if the backend rejected the change (e.g. an unusable
   *  vault folder), so callers like the first-run picker can stay put. */
  async updateSettings(patch: Partial<Settings>): Promise<boolean> {
    const next = { ...$state.snapshot(this.settings), ...patch } as Settings;
    try {
      this.settings = await api.updateSettings(next);
      const info = await api.vaultInfo();
      this.vaultPath = info.path;
      this.isGitRepo = info.is_git_repo;
      await this.reload();
      return true;
    } catch (e) {
      this.notify(String(e));
      return false;
    }
  }

  notify(message: string) {
    this.toast = message;
    clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => (this.toast = null), 3200);
  }

  taskAt(path: string | null): Task | null {
    if (path === null) return null;
    return this.tasks.find((t) => t.path === path) ?? null;
  }
}

export const store = new Store();
