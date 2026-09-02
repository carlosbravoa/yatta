<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import Board from "./lib/components/Board.svelte";
  import Calendar from "./lib/components/Calendar.svelte";
  import Icon from "./lib/components/Icon.svelte";
  import ImportPanel from "./lib/components/ImportPanel.svelte";
  import Onboarding from "./lib/components/Onboarding.svelte";
  import QuickAdd from "./lib/components/QuickAdd.svelte";
  import SettingsPanel from "./lib/components/SettingsPanel.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import TaskDetail from "./lib/components/TaskDetail.svelte";
  import TaskList from "./lib/components/TaskList.svelte";
  import TopBar from "./lib/components/TopBar.svelte";
  import { store } from "./lib/store.svelte";
  import { mark } from "./lib/timing";

  let quickadd = $state<QuickAdd | undefined>();
  let topbar = $state<TopBar | undefined>();

  const openTask = $derived(store.taskAt(store.openPath));

  // Theme. "system" follows the desktop and keeps following it, so switching
  // the OS to dark at sunset changes the app without a restart.
  $effect(() => {
    const theme = store.settings.theme;
    const root = document.documentElement;
    if (theme !== "system") {
      root.dataset.theme = theme;
      return;
    }
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => (root.dataset.theme = mq.matches ? "dark" : "light");
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  });

  onMount(() => {
    mark("js:mounted");
    // Two frames after mount is the earliest point the window has actually
    // painted something, as opposed to having been told to.
    requestAnimationFrame(() => requestAnimationFrame(() => mark("js:first-paint")));
    store.init().then(() => mark("js:data-ready"));

    const pending: Promise<UnlistenFn>[] = [
      // Someone edited the markdown outside the app -- or the quick-add popup
      // saved a task, which the file watcher deliberately ignores as our own
      // write and so would not otherwise report.
      listen("vault-changed", () => store.reload()),
    ];

    return () => {
      for (const p of pending) p.then((off) => off()).catch(() => {});
    };
  });

  /** Every focusable task in the current view, in visual order. */
  function taskElements(): HTMLElement[] {
    return [...document.querySelectorAll<HTMLElement>("[data-task]")];
  }

  /** Move focus by `step`, starting the selection if nothing is focused yet. */
  function moveSelection(step: number) {
    const items = taskElements();
    if (items.length === 0) return;

    const current = items.indexOf(document.activeElement as HTMLElement);
    // Entering from elsewhere selects the first task rather than jumping to
    // the end, whichever direction the keypress was.
    const next = current < 0 ? 0 : Math.min(items.length - 1, Math.max(0, current + step));

    items[next].focus();
    items[next].scrollIntoView({ block: "nearest" });
  }

  function focusedTask() {
    const path = (document.activeElement as HTMLElement | null)?.dataset?.path;
    return path ? store.tasks.find((t) => t.path === path) ?? null : null;
  }

  function isTyping(target: EventTarget | null): boolean {
    const el = target as HTMLElement | null;
    return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
  }

  function onkeydown(event: KeyboardEvent) {
    // The first-run picker owns the window; nothing behind it is reachable.
    if (store.needsOnboarding) return;

    if (event.key === "Escape") {
      if (store.showImport) store.showImport = false;
      else if (store.showSettings) store.showSettings = false;
      else if (store.openPath !== null) store.openPath = null;
      else if (store.query) store.query = "";
      else (document.activeElement as HTMLElement | null)?.blur();
      return;
    }

    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "r") {
      event.preventDefault();
      store.reload();
      return;
    }

    if (isTyping(event.target) || event.ctrlKey || event.metaKey || event.altKey) return;

    switch (event.key) {
      case "n":
        event.preventDefault();
        quickadd?.focus();
        break;
      case "/":
        event.preventDefault();
        topbar?.focusSearch();
        break;
      case "j":
      case "ArrowDown":
        event.preventDefault();
        moveSelection(1);
        break;
      case "k":
      case "ArrowUp":
        event.preventDefault();
        moveSelection(-1);
        break;
      case "x": {
        // Complete without opening. Focus survives because the row keeps its
        // DOM identity; if the view filters it out, fall to the next task.
        const task = focusedTask();
        if (!task) break;
        event.preventDefault();
        const items = taskElements();
        const index = items.indexOf(document.activeElement as HTMLElement);
        store.toggle(task).then(() => {
          const after = taskElements();
          if (!after.includes(document.activeElement as HTMLElement)) {
            after[Math.min(index, after.length - 1)]?.focus();
          }
        });
        break;
      }
      case "e": {
        const task = focusedTask();
        if (!task) break;
        event.preventDefault();
        store.openPath = task.path;
        break;
      }
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="app">
  <Sidebar />

  <main>
    <div class="content" style={store.settings.layout === "board" ? "--content-max: 1500px" : ""}>
      <TopBar bind:this={topbar} />
      <QuickAdd bind:this={quickadd} />

      {#if store.error}
        <div class="banner">
          <Icon name="alert" size={15} />
          <span>{store.error}</span>
          <button class="btn" onclick={() => store.init()}>
            <Icon name="refresh" size={13} />Retry
          </button>
        </div>
      {/if}

      {#if store.loading}
        <div class="loading">Loading your tasks…</div>
      {:else if store.settings.layout === "board"}
        <Board />
      {:else if store.settings.layout === "calendar"}
        <Calendar />
      {:else}
        <TaskList />
      {/if}
    </div>
  </main>

  {#if openTask}
    <!-- Keyed on the path so picking another task remounts the editor with a
         clean draft instead of reconciling one that may be mid-edit. -->
    {#key openTask.path}
      <TaskDetail task={openTask} />
    {/key}
  {/if}
</div>

{#if !store.loading && store.needsOnboarding}
  <Onboarding />
{/if}

{#if store.showSettings}
  <SettingsPanel />
{/if}

{#if store.showImport}
  <ImportPanel initial={store.importText} />
{/if}

{#if store.toast}
  <div class="toast" transition:fly={{ y: 12, duration: 180 }}>
    <span class="msg">{store.toast.message}</span>
    {#if store.toast.action}
      <button class="undo" onclick={() => store.runToastAction()}>
        {store.toast.action.label}
      </button>
    {/if}
    <button class="close" onclick={() => store.dismissToast()} aria-label="Dismiss">
      <Icon name="x" size={12} stroke={2.5} />
    </button>
  </div>
{/if}

<style>
  .app {
    display: flex;
    height: 100%;
    background: var(--bg);
  }

  main {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
  }

  .content {
    max-width: var(--content-max, 1240px);
    margin: 0 auto;
    padding: 22px 26px 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .banner {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 10px 12px;
    border-radius: var(--radius);
    font-size: 13px;
    color: var(--p-urgent);
    background: color-mix(in srgb, var(--p-urgent) 11%, transparent);
    border: 1px solid color-mix(in srgb, var(--p-urgent) 24%, transparent);
  }
  .banner span { flex: 1; }
  .banner .btn { height: 26px; color: inherit; }

  .loading {
    padding: 48px 0;
    text-align: center;
    color: var(--text-faint);
    font-size: 13px;
  }

  .toast {
    position: fixed;
    /* Above the first-run picker, which is the one screen that most needs to
       surface an error. */
    z-index: 70;
    bottom: 22px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: 70vw;
    padding: 8px 10px 8px 16px;
    border-radius: 99px;
    font-size: 13px;
    font-weight: 500;
    color: var(--accent-contrast);
    background: var(--text);
    box-shadow: var(--shadow-lg);
  }
  .msg {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .undo {
    flex: none;
    height: 24px;
    padding: 0 11px;
    border-radius: 99px;
    font-size: 12.5px;
    font-weight: 700;
    color: var(--bg);
    background: color-mix(in srgb, var(--accent-contrast) 22%, transparent);
    transition: background 110ms var(--ease);
  }
  .undo:hover { background: color-mix(in srgb, var(--accent-contrast) 38%, transparent); }

  .close {
    flex: none;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 99px;
    color: var(--accent-contrast);
    opacity: 0.55;
    transition: opacity 110ms var(--ease);
  }
  .close:hover { opacity: 1; }
</style>
