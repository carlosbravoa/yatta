export type Priority = "urgent" | "high" | "medium" | "low" | "none";
export type Status = "todo" | "doing" | "done";

export interface Task {
  id: string;
  title: string;
  status: Status;
  priority: Priority;
  /** `YYYY-MM-DD`, or null when the task has no deadline. */
  due: string | null;
  tags: string[];
  created: string;
  completed: string | null;
  description: string;
  /** Vault-relative path. Empty string for a task not yet written to disk. */
  path: string;
  /** The file had no `id:` — hand- or agent-written, not yet normalised. */
  adopted: boolean;
  /** The file lives under `archive/`. Derived from its path, not stored. */
  archived: boolean;
}

export interface Settings {
  vault_path: string;
  theme: "system" | "light" | "dark";
  group_by: "none" | "tag" | "priority" | "due";
  layout: "list" | "board" | "calendar";
  sort_by: "manual" | "due" | "priority" | "created" | "title";
  show_done: boolean;
  git_autocommit: boolean;
  tray_enabled: boolean;
  hotkey: string;
  first_run_done: boolean;
  /** Width of the task detail panel, in CSS pixels. */
  detail_width: number;
  reminders_enabled: boolean;
  /** Local `HH:MM` times. One entry = once a day, two = twice a day. */
  reminder_times: string[];
  last_reminder: string;
}

export interface VaultInfo {
  path: string;
  exists: boolean;
  is_git_repo: boolean;
  supports_tray: boolean;
  /** A settings file already existed, i.e. this is a returning user. */
  had_settings: boolean;
}

export const PRIORITIES: Priority[] = ["urgent", "high", "medium", "low", "none"];

export const PRIORITY_LABEL: Record<Priority, string> = {
  urgent: "Urgent",
  high: "High",
  medium: "Medium",
  low: "Low",
  none: "None",
};

/** Sort weight; lower comes first. */
export const PRIORITY_RANK: Record<Priority, number> = {
  urgent: 0,
  high: 1,
  medium: 2,
  low: 3,
  none: 4,
};

export function emptyTask(): Task {
  return {
    id: "",
    title: "",
    status: "todo",
    priority: "none",
    due: null,
    tags: [],
    created: new Date().toISOString().slice(0, 10),
    completed: null,
    description: "",
    path: "",
    adopted: false,
    archived: false,
  };
}
