import { invoke } from "@tauri-apps/api/core";
import type { Settings, Task, VaultInfo } from "./types";

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (newSettings: Settings) =>
    invoke<Settings>("update_settings", { newSettings }),
  vaultInfo: () => invoke<VaultInfo>("vault_info"),
  listTasks: () => invoke<Task[]>("list_tasks"),
  saveTask: (task: Task) => invoke<Task>("save_task", { task }),
  createTasks: (tasks: Task[]) => invoke<Task[]>("create_tasks", { tasks }),
  restoreTask: (path: string) => invoke<Task>("restore_task", { path }),
  setStatus: (path: string, status: string) =>
    invoke<Task>("set_status", { path, status }),
  deleteTask: (path: string, title: string) =>
    invoke<void>("delete_task", { path, title }),
  archiveDone: () => invoke<number>("archive_done"),
  absolutePath: (path: string) => invoke<string>("absolute_path", { path }),
};
