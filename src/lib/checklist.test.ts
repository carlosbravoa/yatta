import { checklistProgress, toggleChecklistItem } from "./checklist";

let pass = 0, fail = 0;
function check(label: string, actual: unknown, expected: unknown) {
  const a = JSON.stringify(actual), e = JSON.stringify(expected);
  if (a === e) pass++;
  else { fail++; console.log(`  FAIL ${label}\n       got      ${a}\n       expected ${e}`); }
}

const doc = `Book an appointment first.

- [ ] photos
- [x] find the old passport
* [ ] fill the form
1. [ ] pay the fee

Some trailing prose.`;

check("progress", checklistProgress(doc), { done: 1, total: 4 });
check("no checkboxes", checklistProgress("just prose\n- a bullet"), null);
check("empty", checklistProgress(""), null);

check("tick the first", toggleChecklistItem(doc, 0).split("\n")[2], "- [x] photos");
check("untick a done one", toggleChecklistItem(doc, 1).split("\n")[3], "- [x] find the old passport".replace("[x]", "[ ]"));
check("star bullets work", toggleChecklistItem(doc, 2).split("\n")[4], "* [x] fill the form");
check("numbered items work", toggleChecklistItem(doc, 3).split("\n")[5], "1. [x] pay the fee");

check("out-of-range index is a no-op", toggleChecklistItem(doc, 99), doc);
check("prose is untouched", toggleChecklistItem(doc, 0).split("\n")[0], "Book an appointment first.");

const indented = "- [ ] parent\n    - [ ] child";
check("indentation preserved", toggleChecklistItem(indented, 1), "- [ ] parent\n    - [x] child");

check("uppercase X counts as done", checklistProgress("- [X] done"), { done: 1, total: 1 });
check("uppercase X toggles off", toggleChecklistItem("- [X] done", 0), "- [ ] done");

// A one-character diff keeps the file history clean.
const before = "- [ ] photos";
const after = toggleChecklistItem(before, 0);
check("single char changed", [...before].filter((c, i) => c !== after[i]).length, 1);

console.log(`\n${pass} passed, ${fail} failed`);
if (fail) throw new Error(`${fail} checklist test(s) failed`);
