<script lang="ts">
  import { openPath as revealPath, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { fade } from "svelte/transition";
  import { api } from "../api";
  import { store } from "../store.svelte";
  import type { Task } from "../types";
  import Icon from "./Icon.svelte";

  const MENU_W = 210;
  const MENU_H = 260;

  const menu = $derived(store.contextMenu);
  const task = $derived(menu ? store.tasks.find((t) => t.path === menu.path) : undefined);

  /* Keep the menu on screen when the click lands near an edge, rather than
     letting it hang off the bottom or the right. */
  const pos = $derived.by(() => {
    if (!menu) return { left: 0, top: 0 };
    const left = Math.min(menu.x, window.innerWidth - MENU_W - 8);
    const top = Math.min(menu.y, window.innerHeight - MENU_H - 8);
    return { left: Math.max(8, left), top: Math.max(8, top) };
  });

  function close() {
    store.contextMenu = null;
  }

  /* The target has to be captured before the menu closes. `task` is derived
     from `store.contextMenu`, so dismissing first makes it undefined and every
     action silently no-ops -- which is exactly what happened. */
  async function run(fn: (target: Task) => unknown | Promise<unknown>) {
    const target = task;
    close();
    if (target) await fn(target);
  }

  async function openFile(target: Task) {
    try {
      await revealPath(await api.absolutePath(target.path));
    } catch (e) {
      store.notify(String(e));
    }
  }

  async function showInFolder(target: Task) {
    try {
      await revealItemInDir(await api.absolutePath(target.path));
    } catch (e) {
      store.notify(String(e));
    }
  }
</script>

{#if menu && task}
  <!-- Clicking anywhere else, or right-clicking again, dismisses. -->
  <div
    class="scrim"
    role="presentation"
    onclick={close}
    oncontextmenu={(e) => {
      e.preventDefault();
      close();
    }}
  ></div>

  <div
    class="menu"
    role="menu"
    tabindex="-1"
    style="left: {pos.left}px; top: {pos.top}px"
    transition:fade={{ duration: 90 }}
  >
    <div class="title">{task.title}</div>

    <button role="menuitem" onclick={() => run((t) => (store.openPath = t.path))}>
      <Icon name="edit" size={13} />Open details
    </button>

    <button role="menuitem" onclick={() => run((t) => store.toggle(t))}>
      <Icon name="check" size={13} />
      {task.status === "done" ? "Mark as not done" : "Mark as done"}
    </button>

    <div class="sep"></div>

    <button role="menuitem" onclick={() => run(openFile)}>
      <Icon name="external" size={13} />Open the markdown file
    </button>
    <button role="menuitem" onclick={() => run(showInFolder)}>
      <Icon name="folder" size={13} />Show in file manager
    </button>

    <div class="sep"></div>

    {#if task.archived}
      <button role="menuitem" onclick={() => run((t) => store.restore(t))}>
        <Icon name="restore" size={13} />Restore from archive
      </button>
    {:else}
      <button role="menuitem" onclick={() => run((t) => store.archiveTask(t))}>
        <Icon name="archive" size={13} />Archive
      </button>
    {/if}

    <button class="danger" role="menuitem" onclick={() => run((t) => store.remove(t))}>
      <Icon name="trash" size={13} />Delete
    </button>
  </div>
{/if}

<style>
  /* Above the toast (70). A context menu is the most immediate thing on
     screen -- it sits where you just clicked -- and the toast lives along the
     bottom edge, precisely where you right-click a task near the end of a
     list. Losing that overlap made Archive and Delete unreachable until the
     toast was dismissed by hand. */
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 79;
  }

  .menu {
    position: fixed;
    z-index: 80;
    width: 210px;
    padding: 5px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    box-shadow: var(--shadow-lg);
  }

  .title {
    padding: 6px 9px 7px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    border-bottom: 1px solid var(--border);
    margin-bottom: 4px;
  }

  button {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    height: 30px;
    padding: 0 9px;
    border-radius: 7px;
    font-size: 13px;
    color: var(--text-dim);
    text-align: left;
    transition: background 100ms var(--ease), color 100ms var(--ease);
  }
  button:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  button.danger:hover {
    background: color-mix(in srgb, var(--p-urgent) 14%, transparent);
    color: var(--p-urgent);
  }

  .sep {
    height: 1px;
    margin: 4px 0;
    background: var(--border);
  }
</style>
