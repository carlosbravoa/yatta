/** Month-grid maths for the calendar view.
 *
 *  Two questions, one grid: what is due on a future day, and what got done on a
 *  past one. The second is the harder one to answer anywhere else -- every task
 *  file already records `completed:`, so the history exists; it just was not
 *  being shown.
 */

import { addDays, fromISO, toISO } from "./dates";
import type { Task } from "./types";

export interface DayBuckets {
  /** Open tasks with this deadline. */
  due: Task[];
  /** Tasks marked done on this day, archived ones included. */
  done: Task[];
}

/** 0 = Sunday … 6 = Saturday, from the user's locale where it is available. */
export function firstDayOfWeek(locale?: string): number {
  try {
    const tag = locale ?? (typeof navigator === "undefined" ? "en-GB" : navigator.language);
    // getWeekInfo is not everywhere yet; the catch below covers that.
    const info = (new Intl.Locale(tag) as unknown as { getWeekInfo?: () => { firstDay: number } })
      .getWeekInfo?.();
    if (info?.firstDay) return info.firstDay % 7; // Intl uses 1=Mon..7=Sun
  } catch {
    /* fall through */
  }
  return 1; // Monday
}

/** `YYYY-MM` anchor shifted by whole months, without day-of-month drift. */
export function shiftMonth(anchor: string, delta: number): string {
  const [y, m] = anchor.split("-").map(Number);
  // Anchoring to day 1 avoids the classic "31 Jan + 1 month = 3 Mar" bug.
  const d = new Date(y, m - 1 + delta, 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

export function monthOf(iso: string): string {
  return iso.slice(0, 7);
}

/**
 * Weeks of ISO dates covering `anchor`'s month, padded to whole weeks with the
 * neighbouring months' days. Only as many rows as the month needs -- a fixed
 * six would leave a dead row in most months.
 */
export function monthGrid(anchor: string, weekStart = 1): string[][] {
  const [y, m] = anchor.split("-").map(Number);
  const first = new Date(y, m - 1, 1);
  const daysInMonth = new Date(y, m, 0).getDate();

  const lead = (first.getDay() - weekStart + 7) % 7;
  const total = lead + daysInMonth;
  const weeks = Math.ceil(total / 7);

  const start = addDays(first, -lead);
  const grid: string[][] = [];
  for (let w = 0; w < weeks; w++) {
    const row: string[] = [];
    for (let d = 0; d < 7; d++) row.push(toISO(addDays(start, w * 7 + d)));
    grid.push(row);
  }
  return grid;
}

/**
 * Index tasks by day.
 *
 * A task can appear twice -- once on its deadline and once on the day it was
 * completed -- because those answer different questions.
 *
 * Archived tasks are deliberately included in `done`: archiving is only a file
 * move, and dropping them would make the history of what you actually finished
 * evaporate the moment you tidied up.
 */
export function bucketByDate(tasks: Task[]): Map<string, DayBuckets> {
  const map = new Map<string, DayBuckets>();
  const at = (day: string): DayBuckets => {
    let bucket = map.get(day);
    if (!bucket) map.set(day, (bucket = { due: [], done: [] }));
    return bucket;
  };

  for (const task of tasks) {
    if (task.completed) at(task.completed).done.push(task);
    // A completed task's deadline is history, not something still owed.
    if (task.due && task.status !== "done") at(task.due).due.push(task);
  }
  return map;
}

export function monthLabel(anchor: string, locale?: string): string {
  const [y, m] = anchor.split("-").map(Number);
  return new Date(y, m - 1, 1).toLocaleDateString(locale, { month: "long", year: "numeric" });
}

export function weekdayLabels(weekStart = 1, locale?: string): string[] {
  // 2024-01-07 was a Sunday, so offsetting from it gives any weekday order.
  const sunday = new Date(2024, 0, 7);
  return Array.from({ length: 7 }, (_, i) =>
    addDays(sunday, weekStart + i).toLocaleDateString(locale, { weekday: "short" })
  );
}

export function isSameMonth(iso: string, anchor: string): boolean {
  return iso.startsWith(anchor);
}

export function dayLabel(iso: string, locale?: string): string {
  return fromISO(iso).toLocaleDateString(locale, {
    weekday: "long",
    day: "numeric",
    month: "long",
  });
}
