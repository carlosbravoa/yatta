import { taskToMarkdown, viewToMarkdown } from "./share";
import { emptyTask, type Task } from "./types";
import { parseImport } from "./importer";

let pass = 0, fail = 0;
function check(label: string, actual: unknown, expected: unknown) {
  const a = JSON.stringify(actual), e = JSON.stringify(expected);
  if (a === e) pass++;
  else { fail++; console.log(`  FAIL ${label}\n       got      ${a}\n       expected ${e}`); }
}

function task(over: Partial<Task>): Task {
  return { ...emptyTask(), ...over };
}

check("plain task", taskToMarkdown(task({ title: "Buy milk" })), "- [ ] Buy milk");
check("done task", taskToMarkdown(task({ title: "Buy milk", status: "done" })), "- [x] Buy milk");
check("doing renders unchecked",
  taskToMarkdown(task({ title: "Half done", status: "doing" })), "- [ ] Half done");
check("full metadata",
  taskToMarkdown(task({ title: "Ship v1", due: "2026-09-15", priority: "high", tags: ["work", "release"] })),
  "- [ ] Ship v1 @2026-09-15 !high #work #release");
check("priority none is omitted",
  taskToMarkdown(task({ title: "X", priority: "none" })), "- [ ] X");

const ungrouped = viewToMarkdown("Today", [
  { label: "", tasks: [task({ title: "One" }), task({ title: "Two", status: "done" })] },
]);
check("no heading when ungrouped", ungrouped, "# Today\n\n- [ ] One\n- [x] Two\n");

const grouped = viewToMarkdown("All tasks", [
  { label: "#work", tasks: [task({ title: "Ship v1", priority: "high" })] },
  { label: "#home", tasks: [task({ title: "Buy milk" })] },
]);
check("group headings", grouped,
  "# All tasks\n\n## #work\n\n- [ ] Ship v1 !high\n\n## #home\n\n- [ ] Buy milk\n");

check("empty groups are skipped",
  viewToMarkdown("T", [{ label: "a", tasks: [] }, { label: "b", tasks: [task({ title: "Only" })] }]),
  "# T\n\n## b\n\n- [ ] Only\n");
check("empty view", viewToMarkdown("Done", []), "# Done\n\n_Nothing here._\n");

// The point of using quick-add syntax: the export is importable.
const original = [
  task({ title: "Ship v1", due: "2026-09-15", priority: "high", tags: ["work"] }),
  task({ title: "Review PR", status: "done" }),
];
const round = parseImport(viewToMarkdown("All tasks", [{ label: "", tasks: original }]),
                          { headingsAsTags: false });
check("round-trips: count", round.length, 2);
check("round-trips: title", round[0].title, "Ship v1");
check("round-trips: due", round[0].due, "2026-09-15");
check("round-trips: priority", round[0].priority, "high");
check("round-trips: tags", round[0].tags, ["work"]);
check("round-trips: done state", round[1].status, "done");

console.log(`\n${pass} passed, ${fail} failed`);
if (fail) throw new Error(`${fail} share test(s) failed`);
