/** Interactive checklists inside a task's description.
 *
 *  A description is markdown, so `- [ ] photos first` is already the natural
 *  way to break a task into steps. These helpers let the UI tick those boxes
 *  by rewriting the source line, which keeps the file the single source of
 *  truth -- no parallel state, and an agent reading the file sees the same
 *  thing you do.
 */

/** Matches a markdown task-list item, capturing the bracket contents. */
const ITEM = /^(\s*(?:[-*+]|\d+[.)])\s+\[)([ xX])(\][ \t]?)/;

export interface Progress {
  done: number;
  total: number;
}

/** Checklist progress for a description, or null when it has no checkboxes. */
export function checklistProgress(markdown: string): Progress | null {
  let done = 0;
  let total = 0;
  for (const line of markdown.split("\n")) {
    const m = line.match(ITEM);
    if (!m) continue;
    total++;
    if (m[2].toLowerCase() === "x") done++;
  }
  return total === 0 ? null : { done, total };
}

/**
 * Flip the checkbox at `index` (counted over checkbox lines only, in document
 * order, which is the order the renderer emits them in).
 *
 * Everything else about the line is preserved -- indentation, bullet style,
 * and the text -- so toggling a box produces a one-character diff.
 */
export function toggleChecklistItem(markdown: string, index: number): string {
  let seen = 0;
  return markdown
    .split("\n")
    .map((line) => {
      const m = line.match(ITEM);
      if (!m) return line;
      if (seen++ !== index) return line;
      const checked = m[2].toLowerCase() === "x";
      return line.replace(ITEM, `$1${checked ? " " : "x"}$3`);
    })
    .join("\n");
}
