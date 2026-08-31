<script lang="ts">
  import { openPath } from "@tauri-apps/plugin-opener";
  import { tagStyle } from "../colors";
  import { store } from "../store.svelte";
  import type { ViewId } from "../store.svelte";
  import Icon from "./Icon.svelte";

  // All tasks leads, and is the default view: anything narrower hides every
  // undated task, which is most of them for most people.
  const VIEWS: { id: ViewId; label: string; icon: string }[] = [
    { id: "all", label: "All tasks", icon: "layers" },
    { id: "today", label: "Today", icon: "clock" },
    { id: "upcoming", label: "Upcoming", icon: "calendar" },
    { id: "nodate", label: "No deadline", icon: "inbox" },
    { id: "done", label: "Done", icon: "check" },
  ];

  let counts = $derived(store.counts);

  const folderName = $derived(
    store.vaultPath.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || store.vaultPath
  );

  async function revealVault() {
    try {
      await openPath(store.vaultPath);
    } catch (e) {
      store.notify(String(e));
    }
  }
</script>

<aside class="sidebar">
  <div class="brand">
    <svg width="22" height="22" viewBox="0 0 24 24" aria-hidden="true">
      <defs>
        <linearGradient id="brand-grad" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stop-color="var(--accent)" />
          <stop offset="100%" stop-color="var(--accent-2)" />
        </linearGradient>
      </defs>
      <rect width="24" height="24" rx="6" fill="url(#brand-grad)" />
      <path
        d="M5.8 12.5 10.1 16.8 19.2 5.6"
        fill="none"
        stroke="#fff"
        stroke-width="2.2"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
    <span class="wordmark">yatta</span>
  </div>

  <nav aria-label="Views">
    {#each VIEWS as view (view.id)}
      <button
        class="nav"
        class:active={store.view === view.id}
        onclick={() => (store.view = view.id)}
        aria-current={store.view === view.id ? "page" : undefined}
      >
        <Icon name={view.icon} />
        <span class="label">{view.label}</span>
        {#if view.id === "today" && store.overdue > 0}
          <span class="count overdue" title="{store.overdue} overdue">{store.overdue}</span>
        {:else if counts[view.id as keyof typeof counts] > 0}
          <span class="count">{counts[view.id as keyof typeof counts]}</span>
        {/if}
      </button>
    {/each}

    {#if store.counts.archived > 0}
      <button
        class="nav"
        class:active={store.view === "archived"}
        onclick={() => (store.view = "archived")}
      >
        <Icon name="archive" />
        <span class="label">Archive</span>
        <span class="count">{store.counts.archived}</span>
      </button>
    {/if}
  </nav>

  {#if store.tags.length > 0}
    <div class="section">
      <Icon name="tag" size={12} stroke={2} />
      <span>Tags</span>
    </div>
    <nav class="tags" aria-label="Tags">
      {#each store.tags as tag (tag.name)}
        <button
          class="nav"
          class:active={store.view === `tag:${tag.name}`}
          onclick={() => (store.view = `tag:${tag.name}`)}
        >
          <span class="dot" style={tagStyle(tag.name)}></span>
          <span class="label">{tag.name}</span>
          <span class="count">{tag.count}</span>
        </button>
      {/each}
    </nav>
  {/if}

  <div class="spacer"></div>

  <div class="footer">
    <button class="vault" onclick={revealVault} title={store.vaultPath}>
      <Icon name="folder" size={14} />
      <span class="label">{folderName}</span>
      <Icon name="external" size={12} />
    </button>
    <button
      class="btn icon"
      onclick={() => (store.showSettings = true)}
      title="Settings"
      aria-label="Settings"
    >
      <Icon name="settings" />
    </button>
  </div>
</aside>

<style>
  .sidebar {
    width: var(--sidebar-w);
    flex: none;
    display: flex;
    flex-direction: column;
    padding: 10px 10px 8px;
    gap: 2px;
    border-right: 1px solid var(--border);
    background:
      radial-gradient(120% 60% at 0% 0%, var(--bg-wash-1), transparent 70%),
      radial-gradient(90% 50% at 10% 100%, var(--bg-wash-2), transparent 70%),
      var(--surface);
    overflow-y: auto;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 8px 14px;
  }
  .wordmark {
    font-size: 15px;
    font-weight: 700;
    letter-spacing: -0.015em;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 9px;
    height: 32px;
    padding: 0 9px;
    border-radius: var(--radius-sm);
    color: var(--text-dim);
    font-weight: 500;
    transition: background 110ms var(--ease), color 110ms var(--ease);
  }
  .nav:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .nav.active {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .label {
    flex: 1;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    font-size: 11.5px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--text-faint);
    background: var(--surface-2);
    border-radius: 99px;
    padding: 1px 7px;
  }
  .nav.active .count {
    background: transparent;
    color: inherit;
  }
  .count.overdue {
    background: var(--overdue);
    color: #fff;
  }

  .section {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 18px 10px 6px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .dot {
    width: 9px;
    height: 9px;
    margin: 0 3px;
    border-radius: 99px;
    flex: none;
    background: hsl(var(--tag-h) 68% 56%);
  }

  .spacer {
    flex: 1;
    min-height: 16px;
  }

  .footer {
    display: flex;
    align-items: center;
    gap: 4px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }

  .vault {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 32px;
    padding: 0 9px;
    border-radius: var(--radius-sm);
    color: var(--text-faint);
    font-size: 12.5px;
    transition: background 110ms var(--ease), color 110ms var(--ease);
  }
  .vault:hover {
    background: var(--surface-2);
    color: var(--text);
  }
</style>
