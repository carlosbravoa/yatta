/** Bulk import: one task per line.
 *
 *  Built to swallow whatever you already have — a plain list, a markdown
 *  checklist copied out of a meeting doc, a numbered agenda — without asking
 *  you to reformat it first. Every line goes through the same quick-add parser,
 *  so `!high`, `#tag` and dates work here too.
 *
 *  Beyond that it understands the shape of the text:
 *    - `- `, `* `, `1. ` list markers are stripped
 *    - `- [x]` imports as already done
 *    - `# Heading` sections become a tag on the lines beneath them
 *    - indented lines attach to the task above as its description
 */

import { todayISO } from "./dates";
import { parseQuickAdd } from "./quickadd";
import { emptyTask, type Status, type Task } from "./types";

export interface ImportOptions {
  /** Turn `## Work` into a `work` tag on everything below it. */
  headingsAsTags?: boolean;
}

function tagify(heading: string): string | null {
  const clean = heading
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, "-")
    .replace(/^-+|-+$/g, "");
  return clean || null;
}

export function parseImport(text: string, options: ImportOptions = {}): Task[] {
  const headingsAsTags = options.headingsAsTags ?? true;
  const tasks: Task[] = [];
  let sectionTag: string | null = null;

  for (const raw of text.split(/\r?\n/)) {
    if (!raw.trim()) continue;

    const indent = (raw.match(/^[ \t]*/)?.[0] ?? "").replace(/\t/g, "  ").length;
    let body = raw.trim();

    const heading = body.match(/^#{1,6}\s+(.+)$/);
    if (heading) {
      sectionTag = headingsAsTags ? tagify(heading[1]) : null;
      continue;
    }

    // An indented line belongs to the task above it, verbatim, so pasted
    // sub-bullets and notes survive as the description.
    if (indent >= 2 && tasks.length > 0) {
      const prev = tasks[tasks.length - 1];
      prev.description = prev.description ? `${prev.description}\n${body}` : body;
      continue;
    }

    body = body.replace(/^(?:[-*+]|\d+[.)])\s+/, "");

    let status: Status = "todo";
    const checkbox = body.match(/^\[([ xX-])\]\s*/);
    if (checkbox) {
      if (/[xX]/.test(checkbox[1])) status = "done";
      body = body.slice(checkbox[0].length);
    }
    if (!body.trim()) continue;

    const parsed = parseQuickAdd(body);
    if (!parsed.title) continue;

    const task = emptyTask();
    task.title = parsed.title;
    task.due = parsed.due;
    task.priority = parsed.priority;
    task.tags = [...parsed.tags];
    if (sectionTag && !task.tags.includes(sectionTag)) task.tags.push(sectionTag);
    task.status = status;
    if (status === "done") task.completed = todayISO();

    tasks.push(task);
  }

  return tasks;
}
