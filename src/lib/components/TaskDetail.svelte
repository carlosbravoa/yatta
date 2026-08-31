<script lang="ts">
  import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { marked } from "marked";
  import { fly } from "svelte/transition";
  import { api } from "../api";
  import { checklistProgress, toggleChecklistItem } from "../checklist";
  import { tagStyle } from "../colors";
  import { addDays, formatLong, toISO, todayISO } from "../dates";
  import { matchDate } from "../quickadd";
  import { store } from "../store.svelte";
  import { PRIORITIES, PRIORITY_LABEL, type Priority, type Status, type Task } from "../types";
  import Icon from "./Icon.svelte";

  let { task }: { task: Task } = $props();

  function clone(t: Task): Task {
    return JSON.parse(JSON.stringify($state.snapshot(t))) as Task;
  }

  /* The panel edits a local draft, seeded once from the task it was opened
     with. App.svelte wraps this component in `{#key task.path}`, so choosing a
     different task remounts it with a fresh draft rather than reconciling one
     mid-edit. Capturing the initial value here is therefore deliberate. */
  // svelte-ignore state_referenced_locally
  let draft = $state<Task>(clone(task));
  // svelte-ignore state_referenced_locally
  let dueText = $state(task.due ? formatLong(task.due) : "");
  let tagInput = $state("");
  let titleEl = $state<HTMLTextAreaElement | undefined>();
  /* Open straight into preview when the description is a checklist: the point
     of those boxes is to tick them, and raw `- [ ]` source cannot be. */
  // svelte-ignore state_referenced_locally
  let showPreview = $state(checklistProgress(task.description) !== null);
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  // -- Resizing --------------------------------------------------------------

  const MIN_WIDTH = 300;
  /** Never let the panel squeeze the list below this. */
  const MIN_LIST_WIDTH = 380;

  // svelte-ignore state_referenced_locally
  let width = $state(store.settings.detail_width);
  let resizing = $state(false);
  let viewport = $state(typeof window === "undefined" ? 1280 : window.innerWidth);

  function clamp(px: number, vw: number): number {
    return Math.max(MIN_WIDTH, Math.min(px, Math.max(MIN_WIDTH, vw - MIN_LIST_WIDTH)));
  }

  // Re-clamp when the window shrinks, so a wide panel saved on a big screen
  // cannot swallow the whole list on a small one.
  const applied = $derived(clamp(width, viewport));

  function onResize() {
    viewport = window.innerWidth;
  }

  function startResize(event: PointerEvent) {
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    resizing = true;
    event.preventDefault();
  }

  function moveResize(event: PointerEvent) {
    if (!resizing) return;
    width = clamp(Math.round(window.innerWidth - event.clientX), window.innerWidth);
  }

  function endResize(event: PointerEvent) {
    if (!resizing) return;
    resizing = false;
    (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
    // Persist once, on release -- not on every pointer move.
    store.updateSettings({ detail_width: applied });
  }

  /** Keyboard equivalent, so the panel is not mouse-only. */
  function resizeKey(event: KeyboardEvent) {
    const step = event.shiftKey ? 64 : 16;
    if (event.key === "ArrowLeft") width = clamp(applied + step, viewport);
    else if (event.key === "ArrowRight") width = clamp(applied - step, viewport);
    else if (event.key === "Home") width = 380;
    else return;
    event.preventDefault();
    store.updateSettings({ detail_width: clamp(width, viewport) });
  }

  function resetWidth() {
    width = 380;
    store.updateSettings({ detail_width: 380 });
  }

  // Don't lose a debounced edit when the panel closes or switches tasks.
  $effect(() => () => {
    if (saveTimer !== undefined) flush();
  });

  /* marked emits task-list checkboxes as `disabled`. Strip that so they can be
     ticked: the click handler below rewrites the source line, so the file stays
     the source of truth rather than the DOM. */
  const rendered = $derived(
    draft.description.trim()
      ? (marked.parse(draft.description, { async: false }) as string).replace(
          /(<input[^>]*?)\sdisabled(?:="[^"]*")?/g,
          "$1"
        )
      : ""
  );

  const progress = $derived(checklistProgress(draft.description));

  /** Delegated: map a clicked checkbox to its index among all of them. */
  function onChecklistClick(event: MouseEvent) {
    const target = event.target as HTMLElement | null;
    if (!(target instanceof HTMLInputElement) || target.type !== "checkbox") return;

    const container = event.currentTarget as HTMLElement;
    const boxes = [...container.querySelectorAll('input[type="checkbox"]')];
    const index = boxes.indexOf(target);
    if (index < 0) return;

    draft.description = toggleChecklistItem(draft.description, index);
    flush();
  }

  const suggestions = $derived(
    tagInput.trim()
      ? store.tags
          .map((t) => t.name)
          .filter((n) => n.includes(tagInput.trim().toLowerCase()) && !draft.tags.includes(n))
          .slice(0, 5)
      : []
  );

  /* Grow the title box to fit its content.
   *
   *  CSS `field-sizing: content` would do this natively, but WebKitGTK does
   *  not implement it, so on Linux the box stayed one row tall and clipped the
   *  title -- the one field that must never be cut off. */
  function autogrow() {
    const el = titleEl;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }

  // Runs after the DOM updates, so it also sizes correctly on open and
  // whenever the title changes from elsewhere.
  $effect(() => {
    draft.title;
    autogrow();
  });

  /** A title is a single line by definition: the frontmatter stores it as a
   *  YAML scalar, which cannot hold a raw newline. Wrap visually, never
   *  actually break. */
  function onTitleInput() {
    if (/[\r\n]/.test(draft.title)) {
      draft.title = draft.title.replace(/[\r\n]+/g, " ");
    }
    autogrow();
    scheduleSave();
  }

  function titleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      // Commit rather than insert a newline.
      event.preventDefault();
      (event.currentTarget as HTMLTextAreaElement).blur();
    }
  }

  function scheduleSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(flush, 450);
  }

  async function flush() {
    clearTimeout(saveTimer);
    if (!draft.title.trim()) return;
    const saved = await store.save(draft);
    if (saved) {
      draft.id = saved.id;
      draft.path = saved.path;
      draft.adopted = false;
    }
  }

  function close() {
    flush();
    store.openPath = null;
  }

  function setStatus(status: Status) {
    draft.status = status;
    draft.completed = status === "done" ? todayISO() : null;
    scheduleSave();
  }

  function setPriority(priority: Priority) {
    draft.priority = draft.priority === priority ? "none" : priority;
    scheduleSave();
  }

  function setDue(iso: string | null) {
    draft.due = iso;
    dueText = iso ? formatLong(iso) : "";
    scheduleSave();
  }

  /** Commit whatever the user typed into the deadline box. */
  function commitDue() {
    const raw = dueText.trim();
    if (!raw) {
      setDue(null);
      return;
    }
    const iso = matchDate(raw);
    if (iso) {
      setDue(iso);
    } else {
      // Unparseable: snap back rather than silently keep a bogus deadline.
      dueText = draft.due ? formatLong(draft.due) : "";
      store.notify(`Couldn't read "${raw}" as a date`);
    }
  }

  function addTag(name: string) {
    const clean = name.trim().replace(/^#/, "").toLowerCase();
    if (clean && !draft.tags.includes(clean)) {
      draft.tags.push(clean);
      scheduleSave();
    }
    tagInput = "";
  }

  function removeTag(name: string) {
    draft.tags = draft.tags.filter((t) => t !== name);
    scheduleSave();
  }

  function tagKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" || event.key === "," || event.key === "Tab") {
      if (tagInput.trim()) {
        event.preventDefault();
        addTag(suggestions[0] && event.key === "Tab" ? suggestions[0] : tagInput);
      }
    } else if (event.key === "Backspace" && !tagInput && draft.tags.length) {
      removeTag(draft.tags[draft.tags.length - 1]);
    }
  }

  async function openInEditor() {
    try {
      await openPath(await api.absolutePath(draft.path));
    } catch (e) {
      store.notify(String(e));
    }
  }

  async function revealFile() {
    try {
      await revealItemInDir(await api.absolutePath(draft.path));
    } catch (e) {
      store.notify(String(e));
    }
  }

  /* Groundwork for handing a task to an agent. Until there's an integration to
     call, the useful version of "run this with an agent" is composing the
     prompt and putting it on the clipboard. */
  async function copyAgentPrompt() {
    const lines = [`Task: ${draft.title}`];
    if (draft.due) lines.push(`Deadline: ${draft.due}`);
    if (draft.priority !== "none") lines.push(`Priority: ${draft.priority}`);
    if (draft.tags.length) lines.push(`Tags: ${draft.tags.join(", ")}`);
    if (draft.path) lines.push(`Source file: ${await api.absolutePath(draft.path).catch(() => draft.path)}`);
    if (draft.description.trim()) lines.push("", "Details:", draft.description.trim());
    lines.push("", "Please carry this out. When it's done, set `status: done` in the file's frontmatter.");

    try {
      await navigator.clipboard.writeText(lines.join("\n"));
      store.notify("Agent prompt copied to the clipboard");
    } catch {
      store.notify("Could not reach the clipboard");
    }
  }

  async function remove() {
    await store.remove(draft);
  }
</script>

<svelte:window onresize={onResize} />

<aside
  class="panel"
  class:resizing
  style="width: {applied}px"
  transition:fly={{ x: 380, duration: 240, opacity: 1 }}
>
  <!-- A focusable `separator` is the WAI-ARIA window-splitter pattern: with
       aria-valuenow/min/max it is a widget, not decoration, and the arrow keys
       below drive it. Svelte's rule does not model that case. -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="grip"
    role="separator"
    aria-orientation="vertical"
    aria-label="Resize the details panel"
    aria-valuenow={applied}
    aria-valuemin={MIN_WIDTH}
    aria-valuemax={Math.max(MIN_WIDTH, viewport - MIN_LIST_WIDTH)}
    tabindex="0"
    onpointerdown={startResize}
    onpointermove={moveResize}
    onpointerup={endResize}
    onpointercancel={endResize}
    ondblclick={resetWidth}
    onkeydown={resizeKey}
  ></div>

  <header>
    <div class="statusgroup" role="group" aria-label="Status">
      {#each [["todo", "To do"], ["doing", "Doing"], ["done", "Done"]] as [value, label] (value)}
        <button
          class="seg"
          class:on={draft.status === value}
          data-s={value}
          onclick={() => setStatus(value as Status)}
        >
          {label}
        </button>
      {/each}
    </div>
    <button class="btn icon" onclick={close} aria-label="Close details" title="Close (Esc)">
      <Icon name="x" />
    </button>
  </header>

  <div class="scroll">
    {#if draft.adopted}
      <div class="notice">
        <Icon name="alert" size={14} />
        <span>This file was written outside the app. Editing it here adds the standard frontmatter.</span>
      </div>
    {/if}

    <textarea
      class="title"
      bind:this={titleEl}
      bind:value={draft.title}
      oninput={onTitleInput}
      onkeydown={titleKeydown}
      onblur={flush}
      rows="1"
      placeholder="Task title"
      aria-label="Task title"
    ></textarea>

    <div class="field-row">
      <span class="flabel"><Icon name="calendar" size={13} />Deadline</span>
      <div class="dueedit">
        <input
          bind:value={dueText}
          onblur={commitDue}
          onkeydown={(e) => e.key === "Enter" && (e.currentTarget as HTMLInputElement).blur()}
          placeholder="No deadline — try “friday” or “15 sep”"
          aria-label="Deadline"
        />
        <div class="quick">
          <button onclick={() => setDue(todayISO())}>Today</button>
          <button onclick={() => setDue(toISO(addDays(new Date(), 1)))}>Tomorrow</button>
          <button onclick={() => setDue(toISO(addDays(new Date(), 7)))}>Next week</button>
          {#if draft.due}
            <button class="clear" onclick={() => setDue(null)}>Clear</button>
          {/if}
        </div>
      </div>
    </div>

    <div class="field-row">
      <span class="flabel"><Icon name="flag" size={13} />Priority</span>
      <div class="prios">
        {#each PRIORITIES.filter((p) => p !== "none") as p (p)}
          <button
            class="prio"
            class:on={draft.priority === p}
            data-p={p}
            onclick={() => setPriority(p)}
          >
            <span class="pdot"></span>{PRIORITY_LABEL[p]}
          </button>
        {/each}
      </div>
    </div>

    <div class="field-row">
      <span class="flabel"><Icon name="tag" size={13} />Tags</span>
      <div class="tagedit">
        <div class="chips">
          {#each draft.tags as tag (tag)}
            <span class="tag" style={tagStyle(tag)}>
              {tag}
              <button class="tagx" onclick={() => removeTag(tag)} aria-label={`Remove tag ${tag}`}>
                <Icon name="x" size={10} stroke={2.5} />
              </button>
            </span>
          {/each}
          <input
            bind:value={tagInput}
            onkeydown={tagKeydown}
            onblur={() => tagInput.trim() && addTag(tagInput)}
            placeholder={draft.tags.length ? "" : "Add a tag"}
            aria-label="Add a tag"
          />
        </div>
        {#if suggestions.length}
          <div class="suggest">
            {#each suggestions as s (s)}
              <button onclick={() => addTag(s)}>#{s}</button>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <div class="desc">
      <div class="deschead">
        <span class="flabel">Description</span>
        {#if progress}
          <span class="progress" class:complete={progress.done === progress.total}>
            <Icon name="check" size={11} stroke={3} />
            {progress.done}/{progress.total}
          </span>
        {/if}
        <span class="grow"></span>
        <button
          class="btn"
          onclick={() => (showPreview = !showPreview)}
          disabled={!draft.description.trim()}
        >
          <Icon name={showPreview ? "edit" : "eye"} size={13} />
          {showPreview ? "Edit" : "Preview"}
        </button>
      </div>
      {#if showPreview}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="rendered" onclick={onChecklistClick}>{@html rendered}</div>
      {:else}
        <textarea
          class="body"
          bind:value={draft.description}
          oninput={scheduleSave}
          onblur={flush}
          placeholder="Notes, links, checklists — plain markdown, stored in the file body."
          aria-label="Description"
        ></textarea>
      {/if}
    </div>
  </div>

  <footer>
    <button class="btn agent" onclick={copyAgentPrompt} title="Copy a ready-made prompt for an AI agent">
      <Icon name="sparkles" size={14} />
      Agent prompt
    </button>
    <div class="grow"></div>
    {#if draft.archived}
      <button class="btn" onclick={() => store.restore(draft)} title="Move this task back out of archive/">
        <Icon name="restore" size={14} />
        Restore
      </button>
    {/if}
    {#if draft.path}
      <button class="btn icon" onclick={openInEditor} title="Open the markdown file" aria-label="Open the markdown file">
        <Icon name="external" size={14} />
      </button>
      <button class="btn icon" onclick={revealFile} title="Show in file manager" aria-label="Show in file manager">
        <Icon name="folder" size={14} />
      </button>
      <button class="btn icon danger" onclick={remove} title="Delete task" aria-label="Delete task">
        <Icon name="trash" size={14} />
      </button>
    {/if}
  </footer>
</aside>

<style>
  .panel {
    position: relative;
    flex: none;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--border);
    background: var(--surface);
    box-shadow: -12px 0 32px -24px rgba(0, 0, 0, 0.5);
  }

  /* A wide-enough hit area straddling the border, without the border itself
     moving or the layout shifting. */
  .grip {
    position: absolute;
    left: -3px;
    top: 0;
    bottom: 0;
    width: 7px;
    z-index: 5;
    cursor: col-resize;
    touch-action: none;
  }
  .grip::after {
    content: "";
    position: absolute;
    inset: 0 3px;
    background: var(--accent);
    opacity: 0;
    transition: opacity 120ms var(--ease);
  }
  .grip:hover::after,
  .grip:focus-visible::after,
  .panel.resizing .grip::after {
    opacity: 1;
  }
  .grip:focus-visible { outline: none; }

  /* Text selection while dragging looks broken; the pointer capture keeps the
     drag itself working. */
  .panel.resizing { user-select: none; }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 10px 10px 14px;
    border-bottom: 1px solid var(--border);
  }

  .statusgroup {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }
  .seg {
    height: 25px;
    padding: 0 11px;
    border-radius: 6px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-faint);
    transition: background 120ms var(--ease), color 120ms var(--ease);
  }
  .seg:hover { color: var(--text); }
  .seg.on { background: var(--surface); color: var(--text); box-shadow: var(--shadow-sm); }
  .seg.on[data-s="doing"] { color: var(--p-medium); }
  .seg.on[data-s="done"] { color: var(--p-low); }

  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 16px 18px 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .notice {
    display: flex;
    gap: 8px;
    padding: 9px 11px;
    border-radius: var(--radius-sm);
    font-size: 12.5px;
    line-height: 1.45;
    color: color-mix(in srgb, var(--today) 80%, var(--text));
    background: color-mix(in srgb, var(--today) 13%, transparent);
  }

  textarea {
    width: 100%;
    resize: none;
    font-family: inherit;
  }

  .title {
    font-size: 20px;
    font-weight: 650;
    line-height: 1.35;
    letter-spacing: -0.015em;
    /* Height is driven by autogrow() above, not by `field-sizing`, which
       WebKitGTK does not support. overflow:hidden stops a scrollbar flashing
       during the measure-then-set step. */
    overflow: hidden;
    min-height: 1.35em;
  }
  .title::placeholder { color: var(--text-faint); }

  .field-row {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }

  .flabel {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    width: 84px;
    flex: none;
    padding-top: 7px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-faint);
  }

  .dueedit, .tagedit { flex: 1; min-width: 0; }

  .dueedit input {
    width: 100%;
    height: 30px;
    padding: 0 9px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    border: 1px solid transparent;
    font-size: 13px;
  }
  .dueedit input:focus { border-color: var(--accent); background: var(--surface); }

  .quick {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 6px;
  }
  .quick button {
    height: 22px;
    padding: 0 9px;
    border-radius: 99px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-dim);
    background: var(--surface-2);
    transition: background 110ms var(--ease), color 110ms var(--ease);
  }
  .quick button:hover { background: var(--accent-soft); color: var(--accent); }
  .quick .clear:hover { background: color-mix(in srgb, var(--p-urgent) 14%, transparent); color: var(--p-urgent); }

  .prios { display: flex; flex-wrap: wrap; gap: 4px; padding-top: 3px; }
  .prio {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 10px;
    border-radius: 99px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-dim);
    background: var(--surface-2);
    transition: background 120ms var(--ease), color 120ms var(--ease);
  }
  .pdot { width: 7px; height: 7px; border-radius: 99px; background: currentColor; opacity: 0.85; }
  .prio[data-p="urgent"] { --pc: var(--p-urgent); }
  .prio[data-p="high"]   { --pc: var(--p-high); }
  .prio[data-p="medium"] { --pc: var(--p-medium); }
  .prio[data-p="low"]    { --pc: var(--p-low); }
  .prio:hover { color: var(--pc); }
  .prio.on { background: color-mix(in srgb, var(--pc) 16%, transparent); color: var(--pc); }

  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
    min-height: 30px;
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    border: 1px solid transparent;
  }
  .chips:focus-within { border-color: var(--accent); background: var(--surface); }
  .chips input { flex: 1; min-width: 80px; height: 20px; font-size: 13px; }

  .tagx {
    display: flex;
    margin-right: -3px;
    opacity: 0.6;
    color: inherit;
  }
  .tagx:hover { opacity: 1; }

  .suggest { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 5px; }
  .suggest button {
    height: 21px;
    padding: 0 8px;
    border-radius: 99px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-dim);
    background: var(--surface-2);
  }
  .suggest button:hover { color: var(--accent); background: var(--accent-soft); }

  .desc { display: flex; flex-direction: column; gap: 7px; }
  .deschead { display: flex; align-items: center; gap: 8px; }
  .deschead .grow { flex: 1; }

  .progress {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    height: 18px;
    padding: 0 7px;
    border-radius: 99px;
    font-size: 10.5px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
    background: var(--surface-2);
  }
  .progress.complete { color: var(--p-low); background: color-mix(in srgb, var(--p-low) 16%, transparent); }
  .deschead .flabel { width: auto; padding-top: 0; }
  .deschead .btn { height: 26px; font-size: 12px; }
  .deschead .btn:disabled { opacity: 0.4; cursor: default; }

  .body {
    min-height: 180px;
    padding: 10px 11px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    border: 1px solid transparent;
    font-size: 13.5px;
    line-height: 1.6;
  }
  .body:focus { border-color: var(--accent); background: var(--surface); }

  .rendered {
    min-height: 180px;
    padding: 10px 11px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    font-size: 13.5px;
    line-height: 1.6;
    user-select: text;
  }
  .rendered :global(h1), .rendered :global(h2), .rendered :global(h3) {
    font-size: 15px;
    margin: 14px 0 6px;
  }
  .rendered :global(p) { margin: 0 0 10px; }
  .rendered :global(ul), .rendered :global(ol) { margin: 0 0 10px; padding-left: 20px; }
  .rendered :global(li) { margin-bottom: 3px; }
  .rendered :global(li:has(> input[type="checkbox"])) { list-style: none; margin-left: -18px; }
  .rendered :global(input[type="checkbox"]) {
    appearance: none;
    width: 14px;
    height: 14px;
    margin: 0 7px 0 0;
    vertical-align: -2px;
    border: 1.75px solid var(--border-strong);
    border-radius: 4px;
    cursor: pointer;
    transition: background 130ms var(--ease), border-color 130ms var(--ease);
  }
  .rendered :global(input[type="checkbox"]:hover) { border-color: var(--accent); }
  .rendered :global(input[type="checkbox"]:checked) {
    background: linear-gradient(135deg, var(--accent), var(--accent-2))
      no-repeat center / 10px 10px;
    border-color: transparent;
    background-image:
      url("data:image/svg+xml;charset=utf-8,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='white' stroke-width='4' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M20 6 9 17l-5-5'/%3E%3C/svg%3E"),
      linear-gradient(135deg, var(--accent), var(--accent-2));
    background-size: 10px 10px, 100% 100%;
    background-position: center, center;
    background-repeat: no-repeat, no-repeat;
  }
  .rendered :global(a) { color: var(--accent); }
  .rendered :global(code) {
    font-size: 12.5px;
    padding: 1px 5px;
    border-radius: 5px;
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .rendered :global(pre) {
    padding: 10px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    border: 1px solid var(--border);
    overflow-x: auto;
  }
  .rendered :global(pre code) { border: 0; padding: 0; background: none; }
  .rendered :global(blockquote) {
    margin: 0 0 10px;
    padding-left: 11px;
    border-left: 3px solid var(--border-strong);
    color: var(--text-dim);
  }

  footer {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 9px 10px;
    border-top: 1px solid var(--border);
  }
  .grow { flex: 1; }
  .agent {
    color: var(--accent);
    font-size: 13px;
  }
  .agent:hover { background: var(--accent-soft); color: var(--accent); }
</style>
