<script lang="ts">
  import { tagStyle } from "../colors";
  import { formatDue } from "../dates";
  import { parseQuickAdd } from "../quickadd";
  import { store } from "../store.svelte";
  import { emptyTask, PRIORITY_LABEL } from "../types";
  import Icon from "./Icon.svelte";

  let value = $state("");
  let input = $state<HTMLInputElement | undefined>();
  let busy = $state(false);

  const parsed = $derived(parseQuickAdd(value));
  const hasMeta = $derived(
    parsed.due !== null || parsed.priority !== "none" || parsed.tags.length > 0
  );

  /** Called from App on the `n` shortcut and the tray's quick-add item. */
  export function focus() {
    input?.focus();
    input?.select();
  }

  async function submit() {
    const { title, due, priority, tags } = parsed;
    if (!title.trim() || busy) return;
    busy = true;

    const task = emptyTask();
    task.title = title;
    task.due = due;
    task.priority = priority;
    task.tags = [...tags];

    // Creating from inside a tag view files it under that tag; creating from
    // Today gives it today's deadline. Both are what you meant by being there.
    if (store.view.startsWith("tag:")) {
      const viewTag = store.view.slice(4);
      if (!task.tags.includes(viewTag)) task.tags.push(viewTag);
    } else if (store.view === "today" && task.due === null) {
      task.due = new Date().toISOString().slice(0, 10);
    }

    const saved = await store.save(task);
    busy = false;
    if (saved) {
      value = "";
      input?.focus();
    }
  }

  /* Pasting several lines into a single-task box is unambiguous: you have a
     list. Hand it to the importer rather than mangling it into one title. */
  function onpaste(event: ClipboardEvent) {
    const pasted = event.clipboardData?.getData("text") ?? "";
    if (!/\n\s*\S/.test(pasted.trim())) return;
    event.preventDefault();
    store.importText = pasted;
    store.showImport = true;
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      submit();
    } else if (event.key === "Escape") {
      value = "";
      input?.blur();
    }
  }
</script>

<div class="quickadd" class:filled={value.length > 0}>
  <div class="bar">
    <Icon name="plus" size={17} stroke={2} />
    <input
      bind:this={input}
      bind:value
      {onkeydown}
      {onpaste}
      placeholder="Add a task — try “Send the report tomorrow !high #work”"
      aria-label="Add a task"
      spellcheck="false"
    />
    {#if value.trim()}
      <button class="btn primary go" onclick={submit} disabled={busy || !parsed.title.trim()}>
        Add
        <kbd>&crarr;</kbd>
      </button>
    {/if}
  </div>

  {#if hasMeta}
    <div class="preview">
      <span class="muted">Will create</span>
      <strong>{parsed.title || "…"}</strong>
      {#if parsed.due}
        <span class="chip due"><Icon name="calendar" size={11} />{formatDue(parsed.due)}</span>
      {/if}
      {#if parsed.priority !== "none"}
        <span class="chip prio" data-p={parsed.priority}>
          <Icon name="flag" size={11} />{PRIORITY_LABEL[parsed.priority]}
        </span>
      {/if}
      {#each parsed.tags as tag (tag)}
        <span class="tag" style={tagStyle(tag)}>#{tag}</span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .quickadd {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    box-shadow: var(--shadow-sm);
    transition: border-color 140ms var(--ease), box-shadow 140ms var(--ease);
  }
  .quickadd:focus-within {
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    box-shadow: 0 0 0 3px var(--accent-soft), var(--shadow-sm);
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px 0 13px;
    height: 46px;
    color: var(--text-faint);
  }
  .quickadd:focus-within .bar {
    color: var(--accent);
  }

  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    font-size: 14.5px;
    color: var(--text);
  }
  input::placeholder {
    color: var(--text-faint);
  }

  .go {
    height: 28px;
    font-size: 13px;
    gap: 7px;
  }
  .go:disabled {
    opacity: 0.5;
    cursor: default;
  }

  kbd {
    font: inherit;
    font-size: 11px;
    opacity: 0.75;
  }

  .preview {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 0 13px 10px;
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .preview strong {
    font-weight: 600;
    color: var(--text);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 20px;
    padding: 0 8px;
    border-radius: 99px;
    font-size: 11.5px;
    font-weight: 600;
    background: var(--surface-2);
    color: var(--text-dim);
  }
  .chip.due {
    color: var(--accent);
    background: var(--accent-soft);
  }
  .chip.prio[data-p="urgent"] { color: var(--p-urgent); background: color-mix(in srgb, var(--p-urgent) 14%, transparent); }
  .chip.prio[data-p="high"]   { color: var(--p-high);   background: color-mix(in srgb, var(--p-high) 16%, transparent); }
  .chip.prio[data-p="medium"] { color: var(--p-medium); background: color-mix(in srgb, var(--p-medium) 16%, transparent); }
  .chip.prio[data-p="low"]    { color: var(--p-low);    background: color-mix(in srgb, var(--p-low) 16%, transparent); }
</style>
