<script lang="ts">
  import { viewToMarkdown } from "../share";
  import { store } from "../store.svelte";
  import Icon from "./Icon.svelte";

  const TITLES: Record<string, string> = {
    today: "Today",
    upcoming: "Upcoming",
    all: "All tasks",
    nodate: "No deadline",
    done: "Done",
    archived: "Archive",
  };

  const title = $derived(
    store.view.startsWith("tag:") ? `#${store.view.slice(4)}` : TITLES[store.view] ?? "Tasks"
  );

  const subtitle = $derived.by(() => {
    const n = store.visible.length;
    if (store.query) return `${n} match${n === 1 ? "" : "es"}`;
    const noun = n === 1 ? "task" : "tasks";
    if (store.view === "today" && store.overdue > 0) {
      return `${n} ${noun} · ${store.overdue} overdue`;
    }
    return `${n} ${noun}`;
  });

  let searchInput = $state<HTMLInputElement | undefined>();

  export function focusSearch() {
    searchInput?.focus();
    searchInput?.select();
  }

  const GROUPS = [
    ["none", "No grouping"],
    ["tag", "Group by tag"],
    ["priority", "Group by priority"],
    ["due", "Group by deadline"],
  ] as const;

  const SORTS = [
    ["due", "Deadline"],
    ["priority", "Priority"],
    ["created", "Recently added"],
    ["title", "Title"],
  ] as const;

  let menu = $state<HTMLDetailsElement | undefined>();

  /** Copy the current list as markdown, in the app's own quick-add syntax so
   *  the recipient can paste it straight into the importer. */
  async function shareView() {
    const markdown = viewToMarkdown(title, store.groups);
    try {
      await navigator.clipboard.writeText(markdown);
      const n = store.visible.length;
      store.notify(`Copied ${n} task${n === 1 ? "" : "s"} as markdown`);
    } catch {
      store.notify("Could not reach the clipboard");
    }
  }

  function closeMenu() {
    if (menu) menu.open = false;
  }
</script>

<header class="topbar">
  <div class="titleblock">
    <h1>{title}</h1>
    <span class="sub">{subtitle}</span>
  </div>

  <div class="search" class:active={store.query.length > 0}>
    <Icon name="search" size={14} />
    <input
      bind:this={searchInput}
      bind:value={store.query}
      placeholder="Search"
      aria-label="Search tasks"
      spellcheck="false"
      onkeydown={(e) => e.key === "Escape" && (store.query = "")}
    />
    {#if store.query}
      <button class="clear" onclick={() => (store.query = "")} aria-label="Clear search">
        <Icon name="x" size={12} stroke={2.5} />
      </button>
    {/if}
  </div>

  {#if store.settings.layout === "list"}
    <button
      class="btn icon"
      onclick={shareView}
      disabled={store.visible.length === 0}
      title="Copy this list as markdown"
      aria-label="Copy this list as markdown"
    >
      <Icon name="share" size={14} />
    </button>
  {/if}

  <div class="layout" role="group" aria-label="Layout">
    <button
      class:on={store.settings.layout === "list"}
      onclick={() => store.updateSettings({ layout: "list" })}
      title="List view"
      aria-label="List view"
    >
      <Icon name="list" size={14} />
    </button>
    <button
      class:on={store.settings.layout === "board"}
      onclick={() => store.updateSettings({ layout: "board" })}
      title="Board view"
      aria-label="Board view"
    >
      <Icon name="columns" size={14} />
    </button>
    <button
      class:on={store.settings.layout === "calendar"}
      onclick={() => store.updateSettings({ layout: "calendar" })}
      title="Calendar view"
      aria-label="Calendar view"
    >
      <Icon name="calendar" size={14} />
    </button>
  </div>

  <details class="menu" bind:this={menu} onfocusout={(e) => {
    if (!e.currentTarget.contains(e.relatedTarget as Node)) closeMenu();
  }}>
    <summary class="btn" title="Grouping and sorting">
      <Icon name="layers" size={14} />
      <Icon name="chevronDown" size={12} />
    </summary>
    <div class="pop">
      <div class="pophead">Grouping</div>
      {#each GROUPS as [value, label] (value)}
        <button
          class="item"
          class:on={store.settings.group_by === value}
          onclick={() => { store.updateSettings({ group_by: value }); closeMenu(); }}
        >
          <Icon name="check" size={13} />{label}
        </button>
      {/each}
      <div class="pophead">Sort by</div>
      {#each SORTS as [value, label] (value)}
        <button
          class="item"
          class:on={store.settings.sort_by === value}
          onclick={() => { store.updateSettings({ sort_by: value }); closeMenu(); }}
        >
          <Icon name="check" size={13} />{label}
        </button>
      {/each}
      <div class="sep"></div>
      <button class="item plain" onclick={() => { store.importText = ""; store.showImport = true; closeMenu(); }}>
        <Icon name="import" size={13} />Import tasks…
      </button>
      <button
        class="item"
        class:on={store.settings.show_done}
        onclick={() => store.updateSettings({ show_done: !store.settings.show_done })}
      >
        <Icon name="check" size={13} />Show completed inline
      </button>
      {#if store.counts.done > 0}
        <button class="item plain" onclick={() => { store.archive(); closeMenu(); }}>
          <Icon name="archive" size={13} />Archive {store.counts.done} completed
        </button>
      {/if}
    </div>
  </details>
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 4px 0 16px;
  }

  .titleblock { flex: 1; min-width: 0; }
  h1 {
    font-size: 21px;
    font-weight: 700;
    letter-spacing: -0.02em;
    line-height: 1.2;
  }
  .sub {
    font-size: 12.5px;
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 190px;
    height: 32px;
    padding: 0 9px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    border: 1px solid transparent;
    color: var(--text-faint);
    transition: width 160ms var(--ease), border-color 120ms var(--ease), background 120ms var(--ease);
  }
  .search:focus-within {
    width: 240px;
    border-color: var(--accent);
    background: var(--surface);
    color: var(--accent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    color: var(--text);
  }
  .clear { display: flex; color: var(--text-faint); }
  .clear:hover { color: var(--text); }

  .btn.icon:disabled { opacity: 0.4; cursor: default; }
  .btn.icon:disabled:hover { background: none; color: var(--text-dim); }

  .layout {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    flex: none;
  }
  .layout button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 26px;
    border-radius: 6px;
    color: var(--text-faint);
    transition: background 120ms var(--ease), color 120ms var(--ease);
  }
  .layout button:hover { color: var(--text); }
  .layout button.on { background: var(--surface); color: var(--accent); box-shadow: var(--shadow-sm); }

  .menu { position: relative; }
  summary {
    list-style: none;
    gap: 3px;
  }
  summary::-webkit-details-marker { display: none; }
  .menu[open] summary { background: var(--surface-2); color: var(--text); }

  .pop {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    z-index: 30;
    width: 216px;
    padding: 5px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    box-shadow: var(--shadow-lg);
  }

  .pophead {
    padding: 7px 9px 4px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 30px;
    padding: 0 9px;
    border-radius: 7px;
    font-size: 13px;
    color: var(--text-dim);
    text-align: left;
  }
  .item:hover { background: var(--surface-2); color: var(--text); }
  .item :global(.icon) { opacity: 0; flex: none; }
  .item.on { color: var(--accent); font-weight: 600; }
  .item.on :global(.icon) { opacity: 1; }
  .item.plain :global(.icon) { opacity: 1; }

  .sep { height: 1px; margin: 5px 0; background: var(--border); }
</style>
