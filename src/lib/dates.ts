/** Date helpers. Everything is day-granular and local — deadlines are days,
 *  not instants, so there are no timezones to get wrong. */

export function todayISO(): string {
  return toISO(new Date());
}

export function toISO(d: Date): string {
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

export function fromISO(iso: string): Date {
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(y, (m ?? 1) - 1, d ?? 1);
}

export function addDays(d: Date, n: number): Date {
  const copy = new Date(d);
  copy.setDate(copy.getDate() + n);
  return copy;
}

/** Whole days from today; negative means overdue. */
export function daysUntil(iso: string): number {
  const today = fromISO(todayISO()).getTime();
  const target = fromISO(iso).getTime();
  return Math.round((target - today) / 86_400_000);
}

export type DueTone = "overdue" | "today" | "soon" | "later" | "none";

export function dueTone(iso: string | null): DueTone {
  if (!iso) return "none";
  const d = daysUntil(iso);
  if (d < 0) return "overdue";
  if (d === 0) return "today";
  if (d <= 3) return "soon";
  return "later";
}

const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/** Short, human label for a deadline: "Today", "in 3 days", "Mar 4". */
export function formatDue(iso: string | null): string {
  if (!iso) return "";
  const d = daysUntil(iso);
  if (d === 0) return "Today";
  if (d === 1) return "Tomorrow";
  if (d === -1) return "Yesterday";
  if (d < 0) return `${Math.abs(d)} days ago`;
  if (d < 7) return WEEKDAYS[fromISO(iso).getDay()];
  const date = fromISO(iso);
  const month = date.toLocaleString(undefined, { month: "short" });
  const sameYear = date.getFullYear() === new Date().getFullYear();
  return sameYear ? `${month} ${date.getDate()}` : `${month} ${date.getDate()}, ${date.getFullYear()}`;
}

export function formatLong(iso: string | null): string {
  if (!iso) return "";
  return fromISO(iso).toLocaleDateString(undefined, {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
  });
}
