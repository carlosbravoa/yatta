<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { api } from "../api";
  import { tagStyle } from "../colors";
  import { formatDue } from "../dates";
  import { parseQuickAdd } from "../quickadd";
  import { emptyTask, PRIORITY_LABEL } from "../types";
  import Icon from "./Icon.svelte";

  let value = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let input = $state<HTMLInputElement | undefined>();

  const parsed = $derived(parseQuickAdd(value));
  const hasMeta = $derived(
    parsed.due !== null || parsed.priority !== "none" || parsed.tags.length > 0
  );

  onMount(async () => {
    input?.focus();
    // The popup is its own window, so it has to apply the theme itself.
    try {
      const settings = await api.getSettings();
      const dark =
        settings.theme === "dark" ||
        (settings.theme === "system" &&
          window.matchMedia("(prefers-color-scheme: dark)").matches);
      document.documentElement.dataset.theme = dark ? "dark" : "light";
    } catch {
      /* styling falls back to the system preference */
    }
  });

  async function close() {
    try {
      await getCurrentWindow().close();
    } catch {
      /* the window may already be going away */
    }
  }

  async function submit() {
    if (!parsed.title.trim() || busy) return;
    busy = true;
    error = null;

    const task = emptyTask();
    task.title = parsed.title;
    task.due = parsed.due;
    task.priority = parsed.priority;
    task.tags = [...parsed.tags];

    try {
      await api.saveTask(task);
      // Tells an open main window to reload: the file watcher ignores our own
      // writes, so it would not otherwise notice. This also closes the popup.
      await invoke("quick_add_done");
    } catch (e) {
      error = String(e);
      busy = false;
    }
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      submit();
    } else if (event.key === "Escape") {
      event.preventDefault();
      close();
    }
  }
</script>

<div class="popup">
  <div class="bar" data-tauri-drag-region>
    <Icon name="plus" size={18} stroke={2} />
    <input
      bind:this={input}
      bind:value
      {onkeydown}
      placeholder="Add a task — try “Send the report tomorrow !high #work”"
      aria-label="Add a task"
      spellcheck="false"
      disabled={busy}
    />
    <kbd>esc</kbd>
  </div>

  {#if error}
    <div class="line error"><Icon name="alert" size={12} />{error}</div>
  {:else if hasMeta}
    <div class="line">
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
  {:else}
    <div class="line muted">
      <span><kbd>↵</kbd> to add</span>
      <span class="sep">·</span>
      <span><code>!high</code> priority</span>
      <span class="sep">·</span>
      <span><code>#tag</code></span>
      <span class="sep">·</span>
      <span><code>tomorrow</code> or <code>@15 sep</code></span>
    </div>
  {/if}
</div>

<style>
  .popup {
    height: 100vh;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-strong);
    border-radius: 14px;
    background: var(--surface);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 16px;
    height: 64px;
    flex: none;
    color: var(--accent);
  }

  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    font-size: 17px;
    color: var(--text);
    background: none;
    border: 0;
    outline: none;
  }
  input::placeholder { color: var(--text-faint); }

  kbd {
    flex: none;
    padding: 2px 7px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    font: inherit;
    font-size: 10.5px;
    font-weight: 600;
    color: var(--text-faint);
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 0 16px 14px;
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .line strong { font-weight: 600; color: var(--text); }
  .line.muted { color: var(--text-faint); gap: 5px; }
  .line.muted kbd { padding: 1px 5px; }
  .line.error { color: var(--p-urgent); }
  .sep { opacity: 0.5; }

  code {
    font-size: 11.5px;
    padding: 1px 5px;
    border-radius: 5px;
    background: var(--surface-2);
    color: var(--text-dim);
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
  .chip.due { color: var(--accent); background: var(--accent-soft); }
  .chip.prio[data-p="urgent"] { color: var(--p-urgent); background: color-mix(in srgb, var(--p-urgent) 14%, transparent); }
  .chip.prio[data-p="high"]   { color: var(--p-high);   background: color-mix(in srgb, var(--p-high) 16%, transparent); }
  .chip.prio[data-p="medium"] { color: var(--p-medium); background: color-mix(in srgb, var(--p-medium) 16%, transparent); }
  .chip.prio[data-p="low"]    { color: var(--p-low);    background: color-mix(in srgb, var(--p-low) 16%, transparent); }
</style>
