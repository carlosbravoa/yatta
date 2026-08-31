/** "Share this view" -- render what is currently on screen as markdown.
 *
 *  The output deliberately uses the app's own quick-add syntax (`@date`,
 *  `!priority`, `#tag`) rather than a prose format. That makes the export
 *  round-trip: paste it into the importer and you get the same tasks back.
 *  Sharing a list with someone becomes a way of sending them the tasks.
 *
 *  Only what the list shows is included -- titles and their row metadata, not
 *  descriptions.
 */

import type { Task } from "./types";

/** The shape the store's groups already have; kept local so this module stays
 *  free of the runes store and can be unit-tested on its own. */
export interface ShareGroup {
  label: string;
  tasks: Task[];
}

export function taskToMarkdown(task: Task): string {
  const box = task.status === "done" ? "x" : " ";
  const parts = [`- [${box}] ${task.title.trim()}`];

  if (task.due) parts.push(`@${task.due}`);
  if (task.priority !== "none") parts.push(`!${task.priority}`);
  for (const tag of task.tags) parts.push(`#${tag}`);

  return parts.join(" ");
}

export function viewToMarkdown(title: string, groups: ShareGroup[]): string {
  const lines: string[] = [`# ${title}`, ""];

  const total = groups.reduce((n, g) => n + g.tasks.length, 0);
  if (total === 0) {
    lines.push("_Nothing here._");
    return lines.join("\n") + "\n";
  }

  for (const group of groups) {
    if (group.tasks.length === 0) continue;
    // A single unlabelled group means grouping is off: no heading needed.
    if (group.label) {
      lines.push(`## ${group.label}`, "");
    }
    for (const task of group.tasks) lines.push(taskToMarkdown(task));
    lines.push("");
  }

  return lines.join("\n").replace(/\n+$/, "\n");
}
