<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { fade, fly } from "svelte/transition";
  import { store } from "../store.svelte";
  import Icon from "./Icon.svelte";

  let busy = $state(false);
  let err = $state<string | null>(null);
  // svelte-ignore state_referenced_locally
  let chosen = $state(store.vaultPath);

  /** Show `~/Documents/yatta` rather than the full absolute path. */
  const pretty = $derived.by(() => {
    const home = chosen.match(/^(\/home\/[^/]+|\/Users\/[^/]+)/)?.[1];
    return home ? chosen.replace(home, "~") : chosen;
  });

  async function browse() {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "Choose a folder for your tasks",
      defaultPath: chosen,
    });
    if (typeof picked === "string") {
      chosen = picked;
      err = null;
    }
  }

  async function confirm() {
    if (busy || !chosen.trim()) return;
    busy = true;
    err = null;
    const ok = await store.updateSettings({ vault_path: chosen, first_run_done: true });
    busy = false;
    if (!ok) err = "That folder could not be used. Try another one.";
  }
</script>

<div class="wrap" transition:fade={{ duration: 200 }}>
  <div class="card" in:fly={{ y: 14, duration: 320 }}>
    <div class="halo">
      <svg width="30" height="30" viewBox="0 0 24 24" aria-hidden="true">
        <defs>
          <linearGradient id="ob-grad" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stop-color="var(--accent)" />
            <stop offset="100%" stop-color="var(--accent-2)" />
          </linearGradient>
        </defs>
        <rect width="24" height="24" rx="6" fill="url(#ob-grad)" />
        <path
          d="M5.8 12.5 10.1 16.8 19.2 5.6"
          fill="none"
          stroke="#fff"
          stroke-width="2.2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </div>

    <h1>Where should your tasks live?</h1>
    <p class="lede">
      yatta stores every task as a plain markdown file in a folder you choose. Put it
      somewhere you can reach — your editor, your backups and any AI agent all read the same
      files.
    </p>

    <button class="path" onclick={browse} title="Choose a different folder">
      <Icon name="folder" size={15} />
      <span class="p">{pretty}</span>
      <span class="change">Change</span>
    </button>

    {#if err}
      <div class="err">
        <Icon name="alert" size={14} />
        <span>{err}</span>
      </div>
    {/if}

    <div class="actions">
      <button class="btn primary go" onclick={confirm} disabled={busy}>
        {busy ? "Setting up…" : "Use this folder"}
      </button>
    </div>

    <p class="fine">
      You can move it later in Settings. Nothing is created until you choose.
    </p>
  </div>
</div>

<style>
  .wrap {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    padding: 24px;
    background:
      radial-gradient(90% 60% at 15% 0%, var(--bg-wash-1), transparent 70%),
      radial-gradient(80% 60% at 85% 100%, var(--bg-wash-2), transparent 70%),
      var(--bg);
  }

  .card {
    width: min(460px, 100%);
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 12px;
    padding: 34px 32px 26px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-lg);
  }

  .halo {
    display: grid;
    place-items: center;
    width: 62px;
    height: 62px;
    border-radius: 20px;
    margin-bottom: 2px;
    background:
      radial-gradient(120% 120% at 30% 20%, var(--bg-wash-1), transparent 70%),
      radial-gradient(120% 120% at 70% 90%, var(--bg-wash-2), transparent 70%),
      var(--surface-2);
  }

  h1 {
    font-size: 19px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .lede {
    font-size: 13px;
    line-height: 1.6;
    color: var(--text-dim);
    max-width: 42ch;
  }

  .path {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    margin-top: 6px;
    padding: 0 10px 0 12px;
    height: 42px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    color: var(--text-dim);
    transition: border-color 120ms var(--ease), background 120ms var(--ease);
  }
  .path:hover {
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
    background: var(--surface);
  }
  .p {
    flex: 1;
    min-width: 0;
    text-align: left;
    font-family: ui-monospace, "Ubuntu Mono", "SF Mono", Menlo, monospace;
    font-size: 12.5px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
  }
  .change {
    flex: none;
    font-size: 12px;
    font-weight: 600;
    color: var(--accent);
  }

  .err {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 9px 11px;
    border-radius: var(--radius-sm);
    font-size: 12.5px;
    text-align: left;
    color: var(--p-urgent);
    background: color-mix(in srgb, var(--p-urgent) 11%, transparent);
  }

  .actions { width: 100%; margin-top: 4px; }
  .go {
    width: 100%;
    height: 40px;
    justify-content: center;
    font-size: 14px;
    font-weight: 600;
  }
  .go:disabled { opacity: 0.6; cursor: default; }

  .fine {
    font-size: 11.5px;
    color: var(--text-faint);
    margin-top: 2px;
  }
</style>
