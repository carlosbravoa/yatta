import { bucketByDate, monthGrid, monthLabel, shiftMonth, weekdayLabels } from "./calendar";
import { emptyTask, type Task } from "./types";

let pass = 0, fail = 0;
function check(label: string, actual: unknown, expected: unknown) {
  const a = JSON.stringify(actual), e = JSON.stringify(expected);
  if (a === e) pass++;
  else { fail++; console.log(`  FAIL ${label}\n       got      ${a}\n       expected ${e}`); }
}
function task(over: Partial<Task>): Task { return { ...emptyTask(), ...over }; }

// --- month arithmetic ------------------------------------------------------
check("next month", shiftMonth("2026-08", 1), "2026-09");
check("previous month", shiftMonth("2026-08", -1), "2026-07");
check("rolls over the year", shiftMonth("2026-12", 1), "2027-01");
check("rolls back the year", shiftMonth("2026-01", -1), "2025-12");
// The bug this guards: adding a month to a 31-day date can skip a month.
check("no day-of-month drift", shiftMonth("2026-01", 1), "2026-02");
check("twelve months is a year", shiftMonth("2026-03", 12), "2027-03");

// --- grid ------------------------------------------------------------------
const aug = monthGrid("2026-08", 1);           // Aug 2026 starts on a Saturday
check("whole weeks only", aug.every((w) => w.length === 7), true);
check("starts on the week start", new Date(aug[0][0] + "T12:00").getDay(), 1);
check("covers the first of the month", aug.flat().includes("2026-08-01"), true);
check("covers the last of the month", aug.flat().includes("2026-08-31"), true);
check("no dead trailing row", aug.length, 6);

const feb = monthGrid("2027-02", 1);           // Feb 2027: 28 days, starts Monday
check("a neat month needs four rows", feb.length, 4);
check("and no padding at all", feb.flat()[0], "2027-02-01");

const sunStart = monthGrid("2026-08", 0);
check("week can start on Sunday", new Date(sunStart[0][0] + "T12:00").getDay(), 0);

// Contiguity is checked with calendar arithmetic, not millisecond deltas: a
// day that crosses a DST boundary is 23 or 25 hours long, so "+86400000" is
// simply false twice a year. (This machine sits in America/Santiago, where
// 2026-09-06 is 23 hours -- an earlier version of this test failed there, and
// the grid was right all along.)
function nextDay(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  const n = new Date(y, m - 1, d + 1);
  return `${n.getFullYear()}-${String(n.getMonth() + 1).padStart(2, "0")}-${String(n.getDate()).padStart(2, "0")}`;
}
check("grid days are contiguous",
  aug.flat().every((d, i, all) => i === 0 || nextDay(all[i - 1]) === d), true);

// The failure mode this guards: building the grid by adding 86400000ms would
// repeat or skip a day across a DST change. Spanning a whole year exercises
// both transitions in any timezone.
check("no repeated or skipped days across a year", (() => {
  const seen = new Set<string>();
  for (let i = 0; i < 12; i++) {
    for (const day of monthGrid(shiftMonth("2026-01", i), 1).flat()) {
      const key = `${i}:${day}`;
      if (seen.has(key)) return false;
      seen.add(key);
    }
  }
  return true;
})(), true);

check("every grid day is a real date", (() => {
  for (let i = 0; i < 12; i++) {
    for (const day of monthGrid(shiftMonth("2026-01", i), 1).flat()) {
      const [y, m, d] = day.split("-").map(Number);
      const back = new Date(y, m - 1, d);
      if (back.getDate() !== d || back.getMonth() !== m - 1) return false;
    }
  }
  return true;
})(), true);

// --- bucketing -------------------------------------------------------------
const tasks = [
  task({ title: "Due soon", due: "2026-09-10" }),
  task({ title: "Done today", status: "done", completed: "2026-08-31" }),
  task({ title: "Done and archived", status: "done", completed: "2026-08-31", archived: true }),
  task({ title: "Done late", status: "done", due: "2026-08-20", completed: "2026-08-25" }),
  task({ title: "No dates" }),
];
const idx = bucketByDate(tasks);

check("due lands on its deadline", idx.get("2026-09-10")?.due.map((t) => t.title), ["Due soon"]);
check("archived work still counts as done",
  idx.get("2026-08-31")?.done.map((t) => t.title), ["Done today", "Done and archived"]);
check("completion is recorded on the day it happened",
  idx.get("2026-08-25")?.done.map((t) => t.title), ["Done late"]);
check("a finished task stops being owed on its old deadline",
  idx.get("2026-08-20"), undefined);
check("undated tasks appear nowhere", [...idx.values()].flatMap((b) => [...b.due, ...b.done]).map((t) => t.title).includes("No dates"), false);

// --- labels ----------------------------------------------------------------
check("weekday labels, Monday first", weekdayLabels(1, "en-GB").length, 7);
check("Monday really is first", weekdayLabels(1, "en-GB")[0].startsWith("Mon"), true);
check("Sunday first when asked", weekdayLabels(0, "en-GB")[0].startsWith("Sun"), true);
check("month label", monthLabel("2026-08", "en-GB"), "August 2026");

console.log(`\n${pass} passed, ${fail} failed`);
if (fail) throw new Error(`${fail} calendar test(s) failed`);
