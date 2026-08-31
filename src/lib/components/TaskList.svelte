<script lang="ts">
  import { flip } from "svelte/animate";
  import { fade } from "svelte/transition";
  import { store } from "../store.svelte";
  import Icon from "./Icon.svelte";
  import TaskRow from "./TaskRow.svelte";

  const groups = $derived(store.groups);
  const isEmpty = $derived(store.visible.length === 0);

  const empty = $derived.by(() => {
    if (store.query) {
      return { icon: "search", title: `No matches for "${store.query}"`, body: "Try a shorter search, or clear it to see everything." };
    }
    const view = store.view;
    if (view.startsWith("tag:")) {
      return { icon: "tag", title: `Nothing tagged #${view.slice(4)}`, body: "Tag a task and it will show up here." };
    }
    switch (view) {
      case "today":
        return { icon: "sparkles", title: "Nothing due today", body: "Clear runway. Add something above, or check what's Upcoming." };
      case "upcoming":
        return { icon: "calendar", title: "Nothing scheduled", body: "Tasks with a future deadline land here." };
      case "nodate":
        return { icon: "inbox", title: "Everything has a deadline", body: "Tasks without a date collect here." };
      case "done":
        return { icon: "check", title: "Nothing completed yet", body: "Tick a task off and it moves here." };
      case "archived":
        return { icon: "archive", title: "The archive is empty", body: "Archiving moves completed tasks into an `archive/` folder in your vault." };
      default:
        return { icon: "sparkles", title: "No tasks yet", body: "Add your first one above. It becomes a markdown file you own." };
    }
  });
</script>

<div class="list">
  {#if isEmpty}
    <div class="empty" in:fade={{ duration: 180 }}>
      <div class="halo"><Icon name={empty.icon} size={26} stroke={1.6} /></div>
      <h2>{empty.title}</h2>
      <p>{empty.body}</p>
    </div>
  {:else}
    {#each groups as group (group.key)}
      <section>
        {#if group.label}
          <header>
            <h3>{group.label}</h3>
            <span class="count">{group.tasks.length}</span>
            <span class="rule"></span>
          </header>
        {/if}
        <div class="rows">
          {#each group.tasks as task (task.path)}
            <div animate:flip={{ duration: 220 }} in:fade={{ duration: 140 }}>
              <TaskRow {task} />
            </div>
          {/each}
        </div>
      </section>
    {/each}
  {/if}
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: 22px;
    padding-bottom: 40px;
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 2px 9px;
  }
  h3 {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  .count {
    font-size: 11px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--text-faint);
    background: var(--surface-2);
    border-radius: 99px;
    padding: 1px 7px;
  }
  .rule {
    flex: 1;
    height: 1px;
    background: var(--border);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 6px;
    padding: 64px 24px;
    color: var(--text-faint);
  }
  .halo {
    display: grid;
    place-items: center;
    width: 60px;
    height: 60px;
    margin-bottom: 8px;
    border-radius: 20px;
    color: var(--accent);
    background:
      radial-gradient(120% 120% at 30% 20%, var(--bg-wash-1), transparent 70%),
      radial-gradient(120% 120% at 70% 90%, var(--bg-wash-2), transparent 70%),
      var(--surface-2);
  }
  h2 {
    font-size: 15px;
    font-weight: 650;
    color: var(--text);
  }
  p {
    font-size: 13px;
    max-width: 40ch;
    line-height: 1.55;
  }
</style>
