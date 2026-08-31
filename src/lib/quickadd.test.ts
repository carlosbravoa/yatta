import { matchDate, parseQuickAdd } from "./quickadd";

let pass = 0, fail = 0;
function check(label: string, actual: unknown, expected: unknown) {
  const a = JSON.stringify(actual), e = JSON.stringify(expected);
  if (a === e) { pass++; }
  else { fail++; console.log(`  FAIL ${label}\n       got      ${a}\n       expected ${e}`); }
}

const now = new Date();
const iso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,"0")}-${String(d.getDate()).padStart(2,"0")}`;
const plus = (n: number) => { const d = new Date(now); d.setDate(d.getDate()+n); return iso(d); };
console.log(`today = ${iso(now)} (${now.toDateString()})\n`);

console.log("-- parseQuickAdd --");
const t1 = parseQuickAdd("Send the quarterly report tomorrow !high #work");
check("t1.title", t1.title, "Send the quarterly report");
check("t1.due", t1.due, plus(1));
check("t1.priority", t1.priority, "high");
check("t1.tags", t1.tags, ["work"]);

const t2 = parseQuickAdd("Plan the friday standup");
check("t2 keeps mid-title weekday", t2.title, "Plan the friday standup");
check("t2 no due", t2.due, null);

const t3 = parseQuickAdd("Call mum friday");
check("t3 trailing weekday parsed", t3.title, "Call mum");
check("t3 due is a friday", new Date(t3.due + "T12:00").getDay(), 5);

const t4 = parseQuickAdd("Renew passport @2026-10-01 !urgent #personal #admin");
check("t4.title", t4.title, "Renew passport");
check("t4.due", t4.due, "2026-10-01");
check("t4.priority", t4.priority, "urgent");
check("t4.tags", t4.tags, ["personal", "admin"]);

check("plain text", parseQuickAdd("Buy milk"), { title: "Buy milk", due: null, priority: "none", tags: [] });
check("bang-bang = high", parseQuickAdd("Ship v1 !!").priority, "high");
check("in 2 weeks", parseQuickAdd("Ship v1 in 2 weeks").due, plus(14));
check("in 2 weeks title", parseQuickAdd("Ship v1 in 2 weeks").title, "Ship v1");
check("dangling preposition", parseQuickAdd("Pay rent by tomorrow").title, "Pay rent");
check("leading priority", parseQuickAdd("!!! fix prod outage").title, "fix prod outage");
check("leading priority value", parseQuickAdd("!!! fix prod outage").priority, "urgent");
check("numeric priority", parseQuickAdd("Write docs #proj/yatta !3").priority, "medium");
check("slashed tag", parseQuickAdd("Write docs #proj/yatta !3").tags, ["proj/yatta"]);
check("today view", parseQuickAdd("Review PR today !low").due, iso(now));
check("hash not a tag mid-word", parseQuickAdd("Review PR #482").tags, ["482"]);

console.log("-- matchDate --");
check("today", matchDate("today"), iso(now));
check("tomorrow", matchDate("tomorrow"), plus(1));
check("iso", matchDate("2026-09-15"), "2026-09-15");
check("next week", matchDate("next week"), plus(7));
check("15 sep", matchDate("15 sep")?.slice(5), "09-15");
check("sep 15", matchDate("sep 15")?.slice(5), "09-15");
check("garbage", matchDate("hello"), null);
check("impossible month", matchDate("32/13"), null);
check("not a date phrase", matchDate("in 5 fortnights"), null);
check("empty", matchDate(""), null);

console.log(`\n${pass} passed, ${fail} failed`);
if (fail) throw new Error(`${fail} quick-add test(s) failed`);
