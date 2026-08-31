<script lang="ts">
  import { checklistProgress } from "../checklist";
  import { tagStyle } from "../colors";
  import { dueTone, formatDue } from "../dates";
  import { store } from "../store.svelte";
  import type { Task } from "../types";
  import Icon from "./Icon.svelte";

  let { task }: { task: Task } = $props();

  const isDone = $derived(task.status === "done");
  const tone = $derived(dueTone(task.due));
  const progress = $derived(checklistProgress(task.description));
  const snippet = $derived(
    task.description.replace(/[#*`>_[\]]/g, "").replace(/\s+/g, " ").trim().slice(0, 110)
  );

  function open() {
    store.openPath = task.path;
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      open();
    }
  }

  function toggle(event: MouseEvent) {
    event.stopPropagation();
    store.toggle(task);
  }
</script>

<div
  class="row"
  class:done={isDone}
  class:selected={store.openPath === task.path}
  role="button"
  tabindex="0"
  data-task
  data-path={task.path}
  onclick={open}
  {onkeydown}
>
  <span class="pbar" data-p={task.priority}></span>

  <button
    class="check"
    class:checked={isDone}
    class:doing={task.status === "doing"}
    onclick={toggle}
    aria-label={isDone ? `Mark "${task.title}" as not done` : `Mark "${task.title}" as done`}
  >
    <Icon name="check" size={13} stroke={3} />
  </button>

  <div class="body">
    <div class="titleline">
      <span class="title">{task.title}</span>
      {#if task.adopted}
        <span class="adopted" title="Written outside the app. Saving it will add the standard frontmatter.">
          <Icon name="alert" size={12} />
        </span>
      {/if}
    </div>
    {#if snippet}
      <div class="snippet">{snippet}</div>
    {/if}
  </div>

  <div class="meta">
    {#if progress}
      <span
        class="progress"
        class:complete={progress.done === progress.total}
        title="{progress.done} of {progress.total} steps done"
      >
        <Icon name="check" size={10} stroke={3} />
        {progress.done}/{progress.total}
      </span>
    {/if}
    {#each task.tags.slice(0, 3) as tag (tag)}
      <span class="tag" style={tagStyle(tag)}>{tag}</span>
    {/each}
    {#if task.tags.length > 3}
      <span class="more">+{task.tags.length - 3}</span>
    {/if}
    {#if task.due}
      <span class="due" data-tone={tone}>
        <Icon name={tone === "overdue" ? "alert" : "calendar"} size={11} />
        {formatDue(task.due)}
      </span>
    {/if}
  </div>
</div>

<style>
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 9px 13px 9px 15px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    overflow: hidden;
    transition:
      transform 130ms var(--ease),
      box-shadow 130ms var(--ease),
      border-color 130ms var(--ease),
      opacity 200ms var(--ease);
  }
  .row:hover {
    border-color: var(--border-strong);
    box-shadow: var(--shadow);
    transform: translateY(-1px);
  }
  .row.selected {
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .row.done {
    opacity: 0.52;
  }
  .row.done:hover {
    opacity: 0.8;
  }

  /* Priority reads as a colour stripe rather than a word, so scanning the
     list is a glance and not a read. */
  .pbar {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--p-none);
  }
  .pbar[data-p="urgent"] { background: var(--p-urgent); }
  .pbar[data-p="high"]   { background: var(--p-high); }
  .pbar[data-p="medium"] { background: var(--p-medium); }
  .pbar[data-p="low"]    { background: var(--p-low); }

  .check {
    flex: none;
    display: grid;
    place-items: center;
    width: 19px;
    height: 19px;
    border-radius: 6px;
    border: 1.75px solid var(--border-strong);
    color: transparent;
    background: transparent;
    transition: background 140ms var(--ease), border-color 140ms var(--ease), transform 140ms var(--ease);
  }
  .check:hover {
    border-color: var(--accent);
    transform: scale(1.08);
  }
  .check :global(.icon) {
    transform: scale(0.4);
    opacity: 0;
    transition: transform 180ms cubic-bezier(0.2, 1.5, 0.4, 1), opacity 120ms var(--ease);
  }
  .check.checked {
    background: linear-gradient(135deg, var(--accent), var(--accent-2));
    border-color: transparent;
    color: #fff;
  }
  .check.checked :global(.icon) {
    transform: scale(1);
    opacity: 1;
  }
  .check.doing {
    border-color: var(--p-medium);
    background: color-mix(in srgb, var(--p-medium) 22%, transparent);
  }

  .body {
    flex: 1;
    min-width: 0;
  }

  .titleline {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .title {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .done .title {
    text-decoration: line-through;
    text-decoration-thickness: 1.5px;
    color: var(--text-dim);
  }

  .adopted {
    display: flex;
    color: var(--today);
  }

  .snippet {
    font-size: 12.5px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin-top: 1px;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 5px;
    flex: none;
  }

  .progress {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    height: 20px;
    padding: 0 7px;
    border-radius: 99px;
    font-size: 10.5px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    color: var(--text-dim);
    background: var(--surface-2);
  }
  .progress.complete { color: var(--p-low); background: color-mix(in srgb, var(--p-low) 16%, transparent); }

  .more {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-faint);
  }

  .due {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 20px;
    padding: 0 8px;
    border-radius: 99px;
    font-size: 11.5px;
    font-weight: 600;
    white-space: nowrap;
    background: var(--surface-2);
    color: var(--text-dim);
  }
  .due[data-tone="overdue"] {
    background: color-mix(in srgb, var(--overdue) 15%, transparent);
    color: var(--overdue);
  }
  .due[data-tone="today"] {
    background: color-mix(in srgb, var(--today) 18%, transparent);
    color: color-mix(in srgb, var(--today) 82%, var(--text));
  }
  .due[data-tone="soon"] {
    background: var(--accent-soft);
    color: var(--accent);
  }

  @media (max-width: 720px) {
    .snippet, .tag, .more { display: none; }
    .progress { display: inline-flex; }
  }
</style>
