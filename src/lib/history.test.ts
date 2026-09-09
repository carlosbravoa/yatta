import { editKind, TextHistory, type TextSnapshot } from "./history";

let pass = 0, fail = 0;
function check(label: string, actual: unknown, expected: unknown) {
  const a = JSON.stringify(actual), e = JSON.stringify(expected);
  if (a === e) pass++;
  else { fail++; console.log(`  FAIL ${label}\n       got      ${a}\n       expected ${e}`); }
}

function at(value: string, caret = value.length): TextSnapshot {
  return { value, start: caret, end: caret };
}

// -- Coalescing ------------------------------------------------------------

// Typing a word is one step, not one step per letter.
{
  const h = new TextHistory();
  h.record(at(""), "insertText", 1000);
  h.record(at("w"), "insertText", 1050);
  h.record(at("wo"), "insertText", 1100);
  check("a typing run undoes as one step", h.undo(at("wor"))?.value, "");
  check("and there is nothing behind it", h.canUndo, false);
}

// A pause between keystrokes ends the run.
{
  const h = new TextHistory();
  h.record(at(""), "insertText", 1000);
  h.record(at("w"), "insertText", 5000);
  check("a pause starts a new step", h.undo(at("wo"))?.value, "w");
  check("the earlier step is still there", h.undo(at("w"))?.value, "");
}

// Deleting and typing are different kinds, so they never merge.
{
  const h = new TextHistory();
  h.record(at("word"), "deleteContentBackward", 1000);
  h.record(at("wor"), "deleteContentBackward", 1050);
  h.record(at("wo"), "insertText", 1100);
  check("typing after deleting is its own step", h.undo(at("wok"))?.value, "wo");
  check("the deletions undo together", h.undo(at("wo"))?.value, "word");
}

// The whole point of the report: a wiped field comes back.
{
  const h = new TextHistory();
  h.record(at("the notes I did not mean to lose"), "", 1000);
  check("a replaced field is recoverable", h.undo(at("x"))?.value, "the notes I did not mean to lose");
}

// -- Redo ------------------------------------------------------------------

{
  const h = new TextHistory();
  h.record(at("a"), "", 1000);
  const undone = h.undo(at("ab"));
  check("undo hands back the old state", undone?.value, "a");
  check("redo returns to the new one", h.redo(at("a"))?.value, "ab");
  check("and undo works again after it", h.undo(at("ab"))?.value, "a");
}

// Editing after an undo drops the redo branch.
{
  const h = new TextHistory();
  h.record(at("a"), "", 1000);
  h.undo(at("ab"));
  check("there is a branch to redo", h.canRedo, true);
  h.record(at("a"), "insertText", 2000);
  check("a new edit discards it", h.canRedo, false);
}

// Typing right after an undo does not fold into the run it interrupted.
{
  const h = new TextHistory();
  h.record(at(""), "insertText", 1000);
  h.undo(at("ab"));
  h.record(at(""), "insertText", 1010);
  check("post-undo typing is its own step", h.undo(at("x"))?.value, "");
}

// -- Edges -----------------------------------------------------------------

{
  const h = new TextHistory();
  check("nothing to undo", h.undo(at("a")), null);
  check("nothing to redo", h.redo(at("a")), null);
}

{
  const h = new TextHistory(2);
  h.record(at("1"), "", 1000);
  h.record(at("2"), "", 2000);
  h.record(at("3"), "", 3000);
  check("the stack is capped", h.undo(at("4"))?.value, "3");
  check("dropping the oldest step", [h.undo(at("3"))?.value, h.canUndo], ["2", false]);
}

// The caret comes back with the text.
{
  const h = new TextHistory();
  h.record({ value: "hello world", start: 5, end: 11 }, "", 1000);
  check("selection restored", h.undo(at("hello")), { value: "hello world", start: 5, end: 11 });
}

// -- Edit kinds ------------------------------------------------------------

check("letters group", editKind("insertText", "a"), "insertText");
check("a space breaks the run", editKind("insertText", " "), "");
check("a newline breaks the run", editKind("insertText", "\n"), "");
check("backspaces group", editKind("deleteContentBackward", null), "deleteContentBackward");
check("delete and backspace stay apart", editKind("deleteContentForward", null) === editKind("deleteContentBackward", null), false);
check("a paste stands alone", editKind("insertFromPaste", null), "");
check("an unknown edit stands alone", editKind("", null), "");

console.log(`\n${pass} passed, ${fail} failed`);
if (fail) throw new Error(`${fail} history test(s) failed`);
