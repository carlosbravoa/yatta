/** Natural-language quick add.
 *
 *  "Send the report tomorrow !high #work" parses into a title, a deadline, a
 *  priority and a tag. Nothing here is required — plain text is a valid task.
 *
 *  Design note: `#tag`, `!priority` and `@date` are recognised anywhere in the
 *  string because they're unambiguous. Bare date words ("tomorrow", "friday")
 *  are only recognised at the *end*, so "Plan the friday standup" keeps its
 *  title intact while "Call mum friday" still gets a deadline.
 */

import { addDays, toISO, todayISO } from "./dates";
import type { Priority } from "./types";

export interface Parsed {
  title: string;
  due: string | null;
  priority: Priority;
  tags: string[];
}

const MONTHS = [
  "january", "february", "march", "april", "may", "june",
  "july", "august", "september", "october", "november", "december",
];
const WEEKDAYS = ["sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"];

function nextWeekday(target: number, allowToday = false): string {
  const now = new Date();
  let delta = (target - now.getDay() + 7) % 7;
  if (delta === 0 && !allowToday) delta = 7;
  return toISO(addDays(now, delta));
}

function addMonths(n: number): string {
  const d = new Date();
  d.setMonth(d.getMonth() + n);
  return toISO(d);
}

/** Resolve a complete phrase to `YYYY-MM-DD`, or null if it isn't a date. */
export function matchDate(phrase: string): string | null {
  const s = phrase.trim().toLowerCase().replace(/\s+/g, " ");
  if (!s) return null;

  if (/^(today|tod|now|tonight|eod)$/.test(s)) return todayISO();
  if (/^(tomorrow|tmr|tmrw|tom)$/.test(s)) return toISO(addDays(new Date(), 1));
  if (/^yesterday$/.test(s)) return toISO(addDays(new Date(), -1));
  if (/^(next week|nextweek)$/.test(s)) return toISO(addDays(new Date(), 7));
  if (/^(next month|nextmonth)$/.test(s)) return addMonths(1);
  if (/^(eow|end of week)$/.test(s)) return nextWeekday(5, true); // Friday

  // "in 3 days" / "in 2 weeks" / "in 1 month"
  const rel = s.match(/^in (\d{1,3}) (day|days|week|weeks|month|months)$/);
  if (rel) {
    const n = Number(rel[1]);
    if (rel[2].startsWith("day")) return toISO(addDays(new Date(), n));
    if (rel[2].startsWith("week")) return toISO(addDays(new Date(), n * 7));
    return addMonths(n);
  }

  // "friday", "next friday", "fri"
  const wd = s.match(/^(next )?([a-z]{3,9})$/);
  if (wd) {
    const idx = WEEKDAYS.findIndex(
      (d) => d === wd[2] || d.slice(0, 3) === wd[2]
    );
    if (idx >= 0) return nextWeekday(idx);
  }

  // ISO
  if (/^\d{4}-\d{1,2}-\d{1,2}$/.test(s)) {
    const [y, m, d] = s.split("-").map(Number);
    if (m >= 1 && m <= 12 && d >= 1 && d <= 31) return toISO(new Date(y, m - 1, d));
    return null;
  }

  // "25/12" or "25/12/2026". Ambiguous pairs are read day-first; a value
  // over 12 in either position settles it.
  const slash = s.match(/^(\d{1,2})[/.](\d{1,2})(?:[/.](\d{2,4}))?$/);
  if (slash) {
    let a = Number(slash[1]);
    let b = Number(slash[2]);
    let year = slash[3] ? Number(slash[3]) : new Date().getFullYear();
    if (year < 100) year += 2000;
    let day = a, month = b;
    if (a <= 12 && b > 12) { month = a; day = b; }
    if (month < 1 || month > 12 || day < 1 || day > 31) return null;
    const iso = toISO(new Date(year, month - 1, day));
    // A bare day/month that already passed means next year.
    if (!slash[3] && iso < todayISO()) return toISO(new Date(year + 1, month - 1, day));
    return iso;
  }

  // "sep 15" / "15 sep" / "september 15 2026"
  const named = s.match(/^([a-z]{3,9})\.? (\d{1,2})(?: (\d{4}))?$/)
    ?? s.match(/^(\d{1,2}) ([a-z]{3,9})\.?(?: (\d{4}))?$/);
  if (named) {
    const first = named[1];
    const monthName = /^\d/.test(first) ? named[2] : first;
    const dayStr = /^\d/.test(first) ? first : named[2];
    const mi = MONTHS.findIndex((m) => m === monthName || m.slice(0, 3) === monthName.slice(0, 3));
    if (mi >= 0) {
      const day = Number(dayStr);
      if (day < 1 || day > 31) return null;
      const year = named[3] ? Number(named[3]) : new Date().getFullYear();
      const iso = toISO(new Date(year, mi, day));
      if (!named[3] && iso < todayISO()) return toISO(new Date(year + 1, mi, day));
      return iso;
    }
  }

  return null;
}

function priorityFrom(token: string): Priority | null {
  const t = token.toLowerCase();
  if (t === "!!!" || t === "!urgent" || t === "!1" || t === "!p1") return "urgent";
  if (t === "!!" || t === "!high" || t === "!2" || t === "!p2") return "high";
  if (t === "!" || t === "!medium" || t === "!med" || t === "!3" || t === "!p3") return "medium";
  if (t === "!low" || t === "!4" || t === "!p4") return "low";
  if (t === "!none" || t === "!0") return "none";
  return null;
}

export function parseQuickAdd(input: string): Parsed {
  let text = " " + input + " ";
  const tags: string[] = [];
  let priority: Priority = "none";
  let due: string | null = null;

  // #tags
  text = text.replace(/(\s)#([\p{L}\p{N}_/-]+)/gu, (_m, sp: string, tag: string) => {
    const clean = tag.toLowerCase();
    if (!tags.includes(clean)) tags.push(clean);
    return sp;
  });

  // !priority
  text = text.replace(/(\s)(![\p{L}\d!]*)(?=\s)/gu, (m, sp: string, token: string) => {
    const p = priorityFrom(token);
    if (p === null) return m;
    priority = p;
    return sp;
  });

  // @date — try the longest phrase first so "@next week" beats "@next".
  text = text.replace(
    /(\s)@(\S+(?:\s+\S+){0,2})(?=\s|$)/g,
    (m, sp: string, phrase: string) => {
      if (due) return m;
      const words = phrase.split(/\s+/);
      for (let take = words.length; take >= 1; take--) {
        const candidate = words.slice(0, take).join(" ");
        const iso = matchDate(candidate);
        if (iso) {
          due = iso;
          return sp + words.slice(take).join(" ") + " ";
        }
      }
      return m;
    }
  );

  // Trailing bare date phrase, longest first.
  if (!due) {
    const words = text.trim().split(/\s+/).filter(Boolean);
    for (let take = Math.min(4, words.length - 1); take >= 1; take--) {
      const candidate = words.slice(words.length - take).join(" ");
      const iso = matchDate(candidate);
      if (iso) {
        due = iso;
        text = " " + words.slice(0, words.length - take).join(" ") + " ";
        break;
      }
    }
  }

  // "on friday" / "by friday" / "due friday" leave a dangling preposition.
  const title = text.replace(/\s+/g, " ").trim().replace(/\s+(on|by|due|at)$/i, "").trim();

  return { title, due, priority, tags };
}
