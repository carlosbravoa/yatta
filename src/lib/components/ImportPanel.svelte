<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { tagStyle } from "../colors";
  import { formatDue } from "../dates";
  import { parseImport } from "../importer";
  import { store } from "../store.svelte";
  import { PRIORITY_LABEL } from "../types";
  import Icon from "./Icon.svelte";

  interface Props {
    /** Text to open the importer pre-filled with, e.g. a multi-line paste. */
    initial?: string;
  }
  let { initial = "" }: Props = $props();

  /* The panel is mounted fresh each time it opens, so seeding from the prop
     once is the intent -- there is no later `initial` to track. */
  // svelte-ignore state_referenced_locally
  let text = $state(initial);
  let headingsAsTags = $state(true);
  let skipped = $state<Set<number>>(new Set());
  let busy = $state(false);
  let area = $state<HTMLTextAreaElement | undefined>();

  const parsed = $derived(parseImport(text, { headingsAsTags }));
  const chosen = $derived(parsed.filter((_, i) => !skipped.has(i)));

  $effect(() => {
    area?.focus();
  });

  function toggle(index: number) {
    const next = new Set(skipped);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    skipped = next;
  }

  async function run() {
    if (!chosen.length || busy) return;
    busy = true;
    const n = await store.importTasks(chosen);
    busy = false;
    if (n > 0) store.showImport = false;
  }

  function close() {
    store.showImport = false;
  }
</script>

<div class="scrim" role="presentation" onclick={close} transition:fade={{ duration: 140 }}></div>

<div
  class="modal"
  role="dialog"
  aria-modal="true"
  aria-label="Import tasks"
  transition:scale={{ duration: 170, start: 0.97 }}
>
  <header>
    <div>
      <h2>Import tasks</h2>
      <p class="sub">One task per line. Paste a list, a checklist or an agenda.</p>
    </div>
    <button class="btn icon" onclick={close} aria-label="Close importer"><Icon name="x" /></button>
  </header>

  <div class="split">
    <div class="pane">
      <label class="paneheader" for="import-text">Paste here</label>
      <textarea
        id="import-text"
        bind:this={area}
        bind:value={text}
        spellcheck="false"
        placeholder={"# Work\n- [ ] Ship the beta tomorrow !high\n- [x] Review PR 482\n    the follow-up notes go here\n\n# Personal\n1. Renew passport @2026-10-01"}
      ></textarea>
    </div>

    <div class="pane preview">
      <div class="paneheader">
        <span>Preview</span>
        {#if parsed.length}
          <span class="pill">{chosen.length} of {parsed.length}</span>
        {/if}
      </div>

      <div class="rows">
        {#if parsed.length === 0}
          <div class="hint">
            <Icon name="sparkles" size={20} stroke={1.6} />
            <p>
              Each line becomes a task. <code>- [x]</code> imports as done,
              <code>#&nbsp;Headings</code> become tags, and indented lines attach as the
              description. <code>!high</code>, <code>#tag</code> and dates work per line.
            </p>
          </div>
        {:else}
          {#each parsed as task, i (i)}
            <button class="row" class:off={skipped.has(i)} onclick={() => toggle(i)}>
              <span class="check" class:on={!skipped.has(i)}>
                <Icon name="check" size={11} stroke={3} />
              </span>
              <span class="pbar" data-p={task.priority}></span>
              <span class="body">
                <span class="title">{task.title}</span>
                {#if task.description}
                  <span class="snippet">{task.description.replace(/\s+/g, " ").slice(0, 70)}</span>
                {/if}
              </span>
              <span class="meta">
                {#if task.status === "done"}<span class="chip done">done</span>{/if}
                {#if task.priority !== "none"}
                  <span class="chip prio" data-p={task.priority}>{PRIORITY_LABEL[task.priority]}</span>
                {/if}
                {#if task.due}<span class="chip due">{formatDue(task.due)}</span>{/if}
                {#each task.tags as tag (tag)}
                  <span class="tag" style={tagStyle(tag)}>{tag}</span>
                {/each}
              </span>
            </button>
          {/each}
        {/if}
      </div>
    </div>
  </div>

  <footer>
    <label class="opt">
      <input type="checkbox" bind:checked={headingsAsTags} />
      <span>Turn headings into tags</span>
    </label>
    <div class="grow"></div>
    <button class="btn" onclick={close}>Cancel</button>
    <button class="btn primary" onclick={run} disabled={!chosen.length || busy}>
      {busy ? "Importing…" : `Import ${chosen.length || ""} task${chosen.length === 1 ? "" : "s"}`}
    </button>
  </footer>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    /* See SettingsPanel: dropped for crispness in WebKitGTK. */
    background: rgba(10, 12, 20, 0.5);
  }

  .modal {
    position: fixed;
    z-index: 41;
    /* See SettingsPanel: margin centring keeps the box on whole pixels, which
       a -50% translate does not. */
    inset: 0;
    margin: auto;
    width: min(880px, calc(100vw - 48px));
    height: min(620px, calc(100vh - 64px));
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 12px 14px 20px;
    border-bottom: 1px solid var(--border);
  }
  h2 { font-size: 16px; font-weight: 650; }
  .sub { font-size: 12.5px; color: var(--text-faint); margin-top: 2px; }

  .split { flex: 1; display: grid; grid-template-columns: 1fr 1fr; min-height: 0; }
  .pane { display: flex; flex-direction: column; min-height: 0; }
  .preview { border-left: 1px solid var(--border); }

  .paneheader {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 16px 8px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .pill {
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0;
    text-transform: none;
    color: var(--accent);
    background: var(--accent-soft);
    border-radius: 99px;
    padding: 1px 8px;
  }

  textarea {
    flex: 1;
    margin: 0 16px 16px;
    padding: 11px 12px;
    resize: none;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    border: 1px solid transparent;
    font-family: ui-monospace, "Ubuntu Mono", "SF Mono", Menlo, monospace;
    font-size: 12.5px;
    line-height: 1.65;
  }
  textarea:focus { border-color: var(--accent); background: var(--surface); }
  textarea::placeholder { color: var(--text-faint); }

  .rows {
    flex: 1;
    overflow-y: auto;
    padding: 0 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .hint {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 28px 16px;
    color: var(--text-faint);
    text-align: center;
    align-items: center;
  }
  .hint p { font-size: 12.5px; line-height: 1.6; max-width: 34ch; }
  code {
    font-size: 11.5px;
    padding: 1px 4px;
    border-radius: 4px;
    background: var(--surface-2);
    color: var(--text-dim);
  }

  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 7px 10px 7px 13px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface);
    text-align: left;
    overflow: hidden;
    transition: opacity 120ms var(--ease), border-color 120ms var(--ease);
  }
  .row:hover { border-color: var(--border-strong); }
  .row.off { opacity: 0.4; }

  .pbar { position: absolute; left: 0; top: 0; bottom: 0; width: 3px; background: transparent; }
  .pbar[data-p="urgent"] { background: var(--p-urgent); }
  .pbar[data-p="high"]   { background: var(--p-high); }
  .pbar[data-p="medium"] { background: var(--p-medium); }
  .pbar[data-p="low"]    { background: var(--p-low); }

  .check {
    flex: none;
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: 5px;
    border: 1.75px solid var(--border-strong);
    color: transparent;
  }
  .check.on {
    background: linear-gradient(135deg, var(--accent), var(--accent-2));
    border-color: transparent;
    color: #fff;
  }

  .body { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .title {
    font-size: 13px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .snippet {
    font-size: 11.5px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta { display: flex; align-items: center; gap: 4px; flex: none; }
  .chip {
    height: 18px;
    padding: 0 7px;
    border-radius: 99px;
    font-size: 10.5px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    background: var(--surface-2);
    color: var(--text-dim);
    white-space: nowrap;
  }
  .chip.due { color: var(--accent); background: var(--accent-soft); }
  .chip.done { color: var(--p-low); background: color-mix(in srgb, var(--p-low) 16%, transparent); }
  .chip.prio[data-p="urgent"] { color: var(--p-urgent); background: color-mix(in srgb, var(--p-urgent) 14%, transparent); }
  .chip.prio[data-p="high"]   { color: var(--p-high);   background: color-mix(in srgb, var(--p-high) 16%, transparent); }
  .chip.prio[data-p="medium"] { color: var(--p-medium); background: color-mix(in srgb, var(--p-medium) 16%, transparent); }
  .chip.prio[data-p="low"]    { color: var(--p-low);    background: color-mix(in srgb, var(--p-low) 16%, transparent); }

  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 11px 14px;
    border-top: 1px solid var(--border);
  }
  .grow { flex: 1; }
  .opt { display: flex; align-items: center; gap: 8px; font-size: 12.5px; color: var(--text-dim); cursor: pointer; }
  .opt input {
    appearance: none;
    width: 34px;
    height: 20px;
    border-radius: 99px;
    background: var(--border-strong);
    position: relative;
    cursor: pointer;
    transition: background 160ms var(--ease);
  }
  .opt input::after {
    content: "";
    position: absolute;
    top: 3px; left: 3px;
    width: 14px; height: 14px;
    border-radius: 99px;
    background: #fff;
    transition: transform 160ms var(--ease);
  }
  .opt input:checked { background: linear-gradient(135deg, var(--accent), var(--accent-2)); }
  .opt input:checked::after { transform: translateX(14px); }

  .btn:disabled { opacity: 0.5; cursor: default; }

  @media (max-width: 820px) {
    .split { grid-template-columns: 1fr; }
    .preview { border-left: 0; border-top: 1px solid var(--border); }
  }
</style>
