import { parseImport } from "./importer";

let pass = 0, fail = 0;
function check(label: string, actual: unknown, expected: unknown) {
  const a = JSON.stringify(actual), e = JSON.stringify(expected);
  if (a === e) pass++;
  else { fail++; console.log(`  FAIL ${label}\n       got      ${a}\n       expected ${e}`); }
}

const plain = parseImport("Buy milk\nCall the dentist\nRenew passport");
check("plain lines", plain.length, 3);
check("plain titles", plain.map((t) => t.title), ["Buy milk", "Call the dentist", "Renew passport"]);

const md = parseImport(`# Work
- [ ] Ship the beta !high
- [x] Review PR 482
* Write the changelog

## Personal errands
1. Buy milk
2) Renew passport @2026-10-01`);

check("markdown count", md.length, 5);
check("strips list markers", md[2].title, "Write the changelog");
check("checkbox done", md[1].status, "done");
check("checkbox open", md[0].status, "todo");
check("inline priority", md[0].priority, "high");
check("heading tag", md[0].tags, ["work"]);
check("second heading tag", md[3].tags, ["personal-errands"]);
check("numbered markers", md[3].title, "Buy milk");
check("date in imported line", md[4].due, "2026-10-01");

const nested = parseImport(`- Ship the beta
    Cut the branch first.
    - [ ] run the snap build
- Buy milk`);
check("nested count", nested.length, 2);
check("nested description", nested[0].description, "Cut the branch first.\n- [ ] run the snap build");
check("sibling unaffected", nested[1].title, "Buy milk");

const noTags = parseImport("# Work\n- Ship it", { headingsAsTags: false });
check("headings can be off", noTags[0].tags, []);

check("blank lines skipped", parseImport("\n\n  \n").length, 0);
check("empty checkbox skipped", parseImport("- [ ]   ").length, 0);
check("bare heading makes no task", parseImport("# Just a heading").length, 0);

console.log(`\n${pass} passed, ${fail} failed`);
if (fail) throw new Error(`${fail} importer test(s) failed`);
