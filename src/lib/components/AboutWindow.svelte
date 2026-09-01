<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { api } from "../api";
  import Icon from "./Icon.svelte";

  const REPO = "https://github.com/carlosbravoa/yatta";
  let version = $state("");

  onMount(async () => {
    try {
      version = (await invoke<{ version: string }>("app_info")).version;
      const settings = await api.getSettings();
      const dark =
        settings.theme === "dark" ||
        (settings.theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
      document.documentElement.dataset.theme = dark ? "dark" : "light";
    } catch {
      /* the window is still useful without either */
    }
  });

  function close() {
    getCurrentWindow().close().catch(() => {});
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") close();
  }
</script>

<svelte:window {onkeydown} />

<div class="card">
  <div class="drag" data-tauri-drag-region></div>
  <button class="x" onclick={close} aria-label="Close"><Icon name="x" size={14} /></button>

  <svg width="72" height="72" viewBox="0 0 24 24" aria-hidden="true">
    <defs>
      <linearGradient id="about-grad" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0%" stop-color="var(--accent)" />
        <stop offset="100%" stop-color="var(--accent-2)" />
      </linearGradient>
    </defs>
    <rect width="24" height="24" rx="6" fill="url(#about-grad)" />
    <path d="M5.8 12.5 10.1 16.8 19.2 5.6" fill="none" stroke="#fff" stroke-width="2.2"
          stroke-linecap="round" stroke-linejoin="round" />
  </svg>

  <h1>yatta</h1>
  <p class="version">{version ? `Version ${version}` : ""}</p>

  <p class="tagline">Yet Another Text-based TODO App</p>
  <p class="jp">Also 「やった」 — <em>yatta</em>, “did it!”</p>

  <p class="blurb">
    Every task is a plain markdown file in a folder you own, so your editor, your
    git history and any AI agent can all read and write them without an API.
  </p>

  <div class="links">
    <button onclick={() => openUrl(REPO)}>
      <Icon name="external" size={13} />Source code
    </button>
    <button onclick={() => openUrl(`${REPO}/issues`)}>
      <Icon name="alert" size={13} />Report an issue
    </button>
  </div>

  <p class="legal">MIT licensed · © 2026 Carlos Bravo</p>
</div>

<style>
  .card {
    position: relative;
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 5px;
    padding: 30px 30px 22px;
    border: 1px solid var(--border-strong);
    border-radius: 14px;
    background:
      radial-gradient(120% 60% at 10% 0%, var(--bg-wash-1), transparent 70%),
      radial-gradient(100% 55% at 90% 100%, var(--bg-wash-2), transparent 70%),
      var(--surface);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }

  /* A grab strip along the top: the window has no titlebar to drag by. */
  .drag { position: absolute; inset: 0 0 auto 0; height: 34px; }

  .x {
    position: absolute;
    top: 10px;
    right: 10px;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    color: var(--text-faint);
    transition: background 110ms var(--ease), color 110ms var(--ease);
  }
  .x:hover { background: var(--surface-2); color: var(--text); }

  h1 { font-size: 26px; font-weight: 700; letter-spacing: -0.02em; margin-top: 10px; }
  .version { font-size: 12px; color: var(--text-faint); font-variant-numeric: tabular-nums; }
  .tagline { font-size: 13.5px; font-weight: 600; color: var(--text-dim); margin-top: 12px; }
  .jp { font-size: 13px; color: var(--text-faint); }
  .jp em { font-style: normal; color: var(--accent); }

  .blurb {
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--text-dim);
    max-width: 34ch;
    margin-top: 14px;
  }

  .links { display: flex; gap: 6px; margin-top: 18px; }
  .links button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border-radius: 99px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-dim);
    background: var(--surface-2);
    transition: background 110ms var(--ease), color 110ms var(--ease);
  }
  .links button:hover { background: var(--accent-soft); color: var(--accent); }

  .legal { margin-top: auto; font-size: 11px; color: var(--text-faint); }
</style>
