<script lang="ts">
  import { flip } from "svelte/animate";
  import { tagStyle } from "../colors";
  import { dueTone, formatDue } from "../dates";
  import { store } from "../store.svelte";
  import type { Status, Task } from "../types";
  import Icon from "./Icon.svelte";

  const columns = $derived(store.boardColumns);

  let dragging = $state<string | null>(null);
  let over = $state<Status | null>(null);

  function dragstart(event: DragEvent, task: Task) {
    dragging = task.path;
    event.dataTransfer?.setData("text/plain", task.path);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }

  function dragend() {
    dragging = null;
    over = null;
  }

  function dragover(event: DragEvent, status: Status) {
    // Without preventDefault the browser refuses the drop entirely.
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    over = status;
  }

  function drop(event: DragEvent, status: Status) {
    event.preventDefault();
    const path = event.dataTransfer?.getData("text/plain") || dragging;
    over = null;
    dragging = null;
    if (!path) return;
    const task = store.tasks.find((t) => t.path === path);
    if (task) store.setStatus(task, status);
  }

  /** Keyboard equivalent of dragging: move a card one column along. */
  function nudge(task: Task, direction: 1 | -1) {
    const order: Status[] = ["todo", "doing", "done"];
    const next = order[order.indexOf(task.status) + direction];
    if (next) store.setStatus(task, next);
  }

  function onCardKey(event: KeyboardEvent, task: Task) {
    if (event.key === "ArrowRight") {
      event.preventDefault();
      nudge(task, 1);
    } else if (event.key === "ArrowLeft") {
      event.preventDefault();
      nudge(task, -1);
    }
  }
</script>

<div class="board">
  {#each columns as column (column.id)}
    <section
      class="column"
      class:over={over === column.id}
      data-col={column.id}
      aria-label={column.label}
      ondragover={(e) => dragover(e, column.id)}
      ondragleave={() => (over === column.id ? (over = null) : null)}
      ondrop={(e) => drop(e, column.id)}
    >
      <header>
        <span class="dot" data-col={column.id}></span>
        <h3>{column.label}</h3>
        <span class="count">{column.tasks.length}</span>
      </header>

      <div class="cards">
        {#each column.tasks as task (task.path)}
          <button
            class="card"
            class:fresh={store.isFresh(task.path)}
            class:dragging={dragging === task.path}
            class:done={task.status === "done"}
            data-task
            data-path={task.path}
            draggable="true"
            ondragstart={(e) => dragstart(e, task)}
            ondragend={dragend}
            onclick={() => (store.openPath = task.path)}
            onkeydown={(e) => onCardKey(e, task)}
            animate:flip={{ duration: 200 }}
          >
            <span class="pbar" data-p={task.priority}></span>
            <span class="title">{task.title}</span>
            {#if task.tags.length || task.due}
              <div class="meta">
                {#if task.due}
                  <span class="due" data-tone={dueTone(task.due)}>
                    <Icon name={dueTone(task.due) === "overdue" ? "alert" : "calendar"} size={10} />
                    {formatDue(task.due)}
                  </span>
                {/if}
                {#each task.tags.slice(0, 2) as tag (tag)}
                  <span class="tag" style={tagStyle(tag)}>{tag}</span>
                {/each}
                {#if task.tags.length > 2}<span class="more">+{task.tags.length - 2}</span>{/if}
              </div>
            {/if}
          </button>
        {/each}

        {#if column.tasks.length === 0}
          <div class="placeholder">Drop a task here</div>
        {/if}
      </div>
    </section>
  {/each}
</div>

<style>
  .board {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
    padding-bottom: 32px;
    align-items: start;
  }

  .column {
    display: flex;
    flex-direction: column;
    min-height: 200px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-2);
    transition: background 130ms var(--ease), border-color 130ms var(--ease);
  }
  .column.over {
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    background: var(--accent-soft);
  }

  header {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 0 4px 10px;
  }
  h3 {
    flex: 1;
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  .count {
    font-size: 11px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--text-faint);
    background: var(--surface);
    border-radius: 99px;
    padding: 1px 7px;
  }
  .dot { width: 8px; height: 8px; border-radius: 99px; flex: none; }
  .dot[data-col="todo"]  { background: var(--text-faint); }
  .dot[data-col="doing"] { background: var(--p-medium); }
  .dot[data-col="done"]  { background: var(--p-low); }

  .cards { display: flex; flex-direction: column; gap: 6px; }

  .card {
    position: relative;
    width: 100%;
    text-align: left;
    font: inherit;
    color: inherit;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 9px 10px 9px 13px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface);
    box-shadow: var(--shadow-sm);
    overflow: hidden;
    cursor: grab;
    transition: transform 120ms var(--ease), box-shadow 120ms var(--ease), opacity 120ms var(--ease);
  }
  .card:hover { box-shadow: var(--shadow); transform: translateY(-1px); }

  /* A newly-arrived task glows briefly so you can see where it landed. Drawn
     as an overlay rather than by animating background-color, so it does not
     fight the hover and selected states underneath it. */
  .fresh::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    pointer-events: none;
    background: linear-gradient(
      90deg,
      color-mix(in srgb, var(--accent) 26%, transparent),
      color-mix(in srgb, var(--accent-2) 18%, transparent)
    );
    animation: fresh-fade 3000ms var(--ease) forwards;
  }
  @keyframes fresh-fade {
    0%   { opacity: 1; }
    30%  { opacity: 1; }
    100% { opacity: 0; }
  }

  .card:active { cursor: grabbing; }
  .card.dragging { opacity: 0.4; }
  .card.done .title { text-decoration: line-through; color: var(--text-dim); }

  .pbar { position: absolute; left: 0; top: 0; bottom: 0; width: 3px; background: transparent; }
  .pbar[data-p="urgent"] { background: var(--p-urgent); }
  .pbar[data-p="high"]   { background: var(--p-high); }
  .pbar[data-p="medium"] { background: var(--p-medium); }
  .pbar[data-p="low"]    { background: var(--p-low); }

  .title { font-size: 13px; font-weight: 500; line-height: 1.4; }

  .meta { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; }

  .due {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    height: 18px;
    padding: 0 7px;
    border-radius: 99px;
    font-size: 10.5px;
    font-weight: 700;
    background: var(--surface-2);
    color: var(--text-dim);
    white-space: nowrap;
  }
  .due[data-tone="overdue"] { background: color-mix(in srgb, var(--overdue) 15%, transparent); color: var(--overdue); }
  .due[data-tone="today"]   { background: color-mix(in srgb, var(--today) 18%, transparent); color: color-mix(in srgb, var(--today) 82%, var(--text)); }
  .due[data-tone="soon"]    { background: var(--accent-soft); color: var(--accent); }

  .more { font-size: 10.5px; font-weight: 700; color: var(--text-faint); }

  .placeholder {
    padding: 16px 8px;
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius-sm);
    text-align: center;
    font-size: 12px;
    color: var(--text-faint);
  }

  @media (max-width: 780px) {
    .board { grid-template-columns: 1fr; }
  }
</style>
