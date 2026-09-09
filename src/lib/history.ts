/** Undo and redo inside a single text box.
 *
 *  Every text field here is bound to state the app also writes from elsewhere
 *  -- a save round-trip renaming the file, the watcher picking up an edit on
 *  disk, a suggestion filling the box in. Assigning `value` from script is
 *  exactly what clears the webview's own undo stack, so Ctrl+Z in the title or
 *  the description had nothing to fall back to and typing over something was
 *  final.
 *
 *  So each field keeps its own stack instead. `TextHistory` below is the whole
 *  policy and is pure -- snapshots in, snapshots out, no DOM -- and
 *  `undoable` is the thin Svelte action that feeds it real events.
 */

export interface TextSnapshot {
  value: string;
  start: number;
  end: number;
}

/** Successive edits of the same kind within this window become one undo step,
 *  so Ctrl+Z takes back a word you just typed rather than one letter. */
const COALESCE_MS = 600;

/** How many steps back a single field remembers. */
const LIMIT = 200;

export class TextHistory {
  private past: TextSnapshot[] = [];
  private future: TextSnapshot[] = [];
  private lastAt = 0;
  private lastKind = "";

  constructor(private limit: number = LIMIT) {}

  get canUndo(): boolean {
    return this.past.length > 0;
  }

  get canRedo(): boolean {
    return this.future.length > 0;
  }

  /** Record the state a field held *before* an edit.
   *
   *  `kind` groups edits into runs: the same non-empty kind in quick
   *  succession folds into the step already on the stack. An empty kind never
   *  folds, which is how a paste, a newline or a space ends the run it lands
   *  in -- those are the boundaries you expect a single undo to stop at. */
  record(before: TextSnapshot, kind: string, at: number = Date.now()): void {
    const continues =
      kind !== "" && kind === this.lastKind && at - this.lastAt < COALESCE_MS && this.past.length > 0;

    // A fresh edit is a new branch: whatever was redoable is now unreachable.
    this.future.length = 0;
    this.lastAt = at;
    this.lastKind = kind;
    if (continues) return;

    this.past.push(before);
    if (this.past.length > this.limit) this.past.shift();
  }

  /** The state to restore, or null when there is nothing left to undo.
   *  `current` is what the field holds right now, so redo can return to it. */
  undo(current: TextSnapshot): TextSnapshot | null {
    const previous = this.past.pop();
    if (!previous) return null;
    this.future.push(current);
    // Typing after an undo starts its own step rather than joining the run
    // that was interrupted.
    this.lastKind = "";
    return previous;
  }

  redo(current: TextSnapshot): TextSnapshot | null {
    const next = this.future.pop();
    if (!next) return null;
    this.past.push(current);
    this.lastKind = "";
    return next;
  }
}

/** The kind token for an edit, from the event that made it. Whitespace and
 *  anything that is not plain typing or deleting get "", so they stand alone
 *  in the history. */
export function editKind(inputType: string, data: string | null): string {
  if (inputType === "insertText") return data && !/\s/.test(data) ? "insertText" : "";
  if (inputType.startsWith("delete")) return inputType;
  return "";
}

type TextField = HTMLInputElement | HTMLTextAreaElement;

function snapshot(node: TextField): TextSnapshot {
  return {
    value: node.value,
    start: node.selectionStart ?? node.value.length,
    end: node.selectionEnd ?? node.value.length,
  };
}

/** Svelte action: give a text input or textarea a working Ctrl+Z / Ctrl+Shift+Z
 *  (and Ctrl+Y), independent of whatever the webview does. */
export function undoable(node: TextField) {
  const history = new TextHistory();
  let before = snapshot(node);
  // Set while we write the value back ourselves, so restoring a step is not
  // mistaken for an edit and recorded as one.
  let applying = false;

  function onBeforeInput() {
    // Read the field afresh rather than trusting the last snapshot: the value
    // may have been replaced from the store since the previous keystroke.
    if (!applying) before = snapshot(node);
  }

  function onInput(event: Event) {
    if (applying) return;
    const edit = event as InputEvent;
    history.record(before, editKind(edit.inputType ?? "", edit.data ?? null));
    // Keeps the field usable even where `beforeinput` never arrives; the
    // caret is then the post-edit one, which is close enough to restore to.
    before = snapshot(node);
  }

  function apply(next: TextSnapshot) {
    applying = true;
    try {
      node.value = next.value;
      try {
        node.setSelectionRange(next.start, next.end);
      } catch {
        // Some input types refuse a selection; the text still restored.
      }
      // What tells Svelte's binding -- and the save this field schedules --
      // that the value moved. Setting `value` alone is silent.
      node.dispatchEvent(new Event("input", { bubbles: true }));
    } finally {
      applying = false;
    }
    before = snapshot(node);
  }

  function onKeydown(event: Event) {
    const key = event as KeyboardEvent;
    if (!(key.ctrlKey || key.metaKey) || key.altKey) return;
    const letter = key.key.toLowerCase();
    const undo = letter === "z" && !key.shiftKey;
    const redo = (letter === "z" && key.shiftKey) || letter === "y";
    if (!undo && !redo) return;

    // Claimed either way: a half-working native undo underneath would only
    // disagree with the stack shown here.
    event.preventDefault();
    const next = undo ? history.undo(snapshot(node)) : history.redo(snapshot(node));
    if (next) apply(next);
  }

  node.addEventListener("beforeinput", onBeforeInput);
  node.addEventListener("input", onInput);
  node.addEventListener("keydown", onKeydown);

  return {
    destroy() {
      node.removeEventListener("beforeinput", onBeforeInput);
      node.removeEventListener("input", onInput);
      node.removeEventListener("keydown", onKeydown);
    },
  };
}
