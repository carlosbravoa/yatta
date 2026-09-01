<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openPath, openUrl } from "@tauri-apps/plugin-opener";
  import { fade, scale } from "svelte/transition";
  import { store } from "../store.svelte";
  import Icon from "./Icon.svelte";

  const REPO = "https://github.com/carlosbravoa/yatta";

  const THEMES = [
    ["system", "System", "circle"],
    ["light", "Light", "sun"],
    ["dark", "Dark", "moon"],
  ] as const;

  // svelte-ignore state_referenced_locally
  let hotkeyDraft = $state(store.settings.hotkey);
  let version = $state("");

  $effect(() => {
    invoke<{ version: string }>("app_info")
      .then((info) => (version = info.version))
      .catch(() => {});
  });

  const HOURS = Array.from({ length: 24 }, (_, h) => String(h).padStart(2, "0"));
  const MINUTES = ["00", "15", "30", "45"];

  const times = $derived(store.settings.reminder_times);

  /** Second slot defaults to late afternoon: the useful complement to a
   *  morning reminder is one before the day ends, not one an hour later. */
  function setCount(count: number) {
    const next = count === 1 ? [times[0] ?? "09:00"] : [times[0] ?? "09:00", times[1] ?? "17:00"];
    store.updateSettings({ reminder_times: next });
  }

  function setPart(index: number, part: "h" | "m", value: string) {
    const [h, m] = (times[index] ?? "09:00").split(":");
    const next = [...times];
    next[index] = part === "h" ? `${value}:${m}` : `${h}:${value}`;
    store.updateSettings({ reminder_times: next });
  }

  async function chooseVault() {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "Choose a folder for your tasks",
      defaultPath: store.vaultPath,
    });
    if (typeof picked === "string" && picked !== store.vaultPath) {
      await store.updateSettings({ vault_path: picked });
      store.notify("Vault moved. Existing files stay where they were.");
    }
  }

  function close() {
    store.showSettings = false;
  }
</script>

<div
  class="scrim"
  role="presentation"
  onclick={close}
  transition:fade={{ duration: 140 }}
></div>

<div class="modal" role="dialog" aria-modal="true" aria-label="Settings" transition:scale={{ duration: 170, start: 0.97 }}>
  <header>
    <h2>Settings</h2>
    <button class="btn icon" onclick={close} aria-label="Close settings"><Icon name="x" /></button>
  </header>

  <div class="scroll">
    <section>
      <h3>Your tasks</h3>
      <div class="row">
        <div class="text">
          <span class="label">Vault folder</span>
          <span class="hint" title={store.vaultPath}>{store.vaultPath}</span>
        </div>
        <div class="actions">
          <button class="btn" onclick={() => openPath(store.vaultPath)}>
            <Icon name="external" size={13} />Open
          </button>
          <button class="btn primary" onclick={chooseVault}>Change…</button>
        </div>
      </div>
      <p class="note">
        Every task is one markdown file in this folder. Point an agent at it, sync it, or edit it
        by hand — the app follows whatever is on disk.
      </p>
    </section>

    <section>
      <h3>Appearance</h3>
      <div class="row">
        <span class="label">Theme</span>
        <div class="segs">
          {#each THEMES as [value, label, icon] (value)}
            <button
              class="seg"
              class:on={store.settings.theme === value}
              onclick={() => store.updateSettings({ theme: value })}
            >
              <Icon name={icon} size={13} />{label}
            </button>
          {/each}
        </div>
      </div>
    </section>

    <section>
      <h3>Reminders</h3>
      <label class="row toggle">
        <div class="text">
          <span class="label">Remind me about deadlines</span>
          <span class="hint">
            A desktop notification listing what is overdue or due today. Nothing is sent
            when nothing is due.
          </span>
        </div>
        <input
          type="checkbox"
          checked={store.settings.reminders_enabled}
          onchange={(e) => store.updateSettings({ reminders_enabled: e.currentTarget.checked })}
        />
      </label>

      {#if store.settings.reminders_enabled}
        <div class="row">
          <span class="label">How often</span>
          <div class="segs">
            <button class="seg" class:on={times.length <= 1} onclick={() => setCount(1)}>
              Once a day
            </button>
            <button class="seg" class:on={times.length >= 2} onclick={() => setCount(2)}>
              Twice a day
            </button>
          </div>
        </div>

        {#each times as time, i (i)}
          <div class="row">
            <span class="label">{times.length > 1 ? (i === 0 ? "First" : "Second") : "At"}</span>
            <div class="timepick">
              <select
                value={time.split(":")[0]}
                onchange={(e) => setPart(i, "h", e.currentTarget.value)}
                aria-label="Hour"
              >
                {#each HOURS as h (h)}<option value={h}>{h}</option>{/each}
              </select>
              <span class="colon">:</span>
              <select
                value={time.split(":")[1]}
                onchange={(e) => setPart(i, "m", e.currentTarget.value)}
                aria-label="Minute"
              >
                {#each MINUTES as m (m)}<option value={m}>{m}</option>{/each}
              </select>
            </div>
          </div>
        {/each}
      {/if}
    </section>

    <section>
      <h3>Startup and window</h3>
      <label class="row toggle">
        <div class="text">
          <span class="label">Start yatta when I log in</span>
          <span class="hint">Adds a standard autostart entry; removing the toggle removes it.</span>
        </div>
        <input
          type="checkbox"
          checked={store.settings.autostart}
          onchange={(e) => store.updateSettings({ autostart: e.currentTarget.checked })}
        />
      </label>

      {#if store.supportsTray && store.settings.tray_enabled}
        <label class="row toggle">
          <div class="text">
            <span class="label">Keep running when I close the window</span>
            <span class="hint">
              Closing hides yatta to the tray instead of quitting. Reopen it from the tray icon.
            </span>
          </div>
          <input
            type="checkbox"
            checked={store.settings.close_to_tray}
            onchange={(e) => store.updateSettings({ close_to_tray: e.currentTarget.checked })}
          />
        </label>
      {:else}
        <p class="note">
          Closing to the tray needs the tray icon, which is switched off below.
        </p>
      {/if}
    </section>

    <section>
      <h3>Version history</h3>
      <label class="row toggle">
        <div class="text">
          <span class="label">Commit changes to git automatically</span>
          <span class="hint">
            {#if store.isGitRepo}
              This folder is a git repo. Every change is committed a few seconds after you stop editing.
            {:else}
              This folder is not a git repo yet. Run <code>git init</code> in it to enable this.
            {/if}
          </span>
        </div>
        <input
          type="checkbox"
          checked={store.settings.git_autocommit}
          disabled={!store.isGitRepo}
          onchange={(e) => store.updateSettings({ git_autocommit: e.currentTarget.checked })}
        />
      </label>
      <p class="note">
        Uses the <code>git</code> command already on your system — nothing is bundled, and the
        feature simply stays off where git isn't available.
      </p>
    </section>

    {#if store.supportsTray}
      <section>
        <h3>Desktop integration</h3>
        <label class="row toggle">
          <div class="text">
            <span class="label">Tray icon and global hotkey</span>
            <span class="hint">Keeps yatta reachable when the window is closed.</span>
          </div>
          <input
            type="checkbox"
            checked={store.settings.tray_enabled}
            onchange={(e) => store.updateSettings({ tray_enabled: e.currentTarget.checked })}
          />
        </label>
        {#if store.settings.tray_enabled}
          <div class="row">
            <div class="text">
              <span class="label">Quick-add hotkey</span>
              <span class="hint">
                Wayland reserves global shortcuts for the desktop, so bind this in your system
                keyboard settings instead if it doesn't take.
              </span>
            </div>
            <input
              class="field hk"
              bind:value={hotkeyDraft}
              onblur={() => store.updateSettings({ hotkey: hotkeyDraft })}
              onkeydown={(e) => e.key === "Enter" && e.currentTarget.blur()}
              aria-label="Quick-add hotkey"
            />
          </div>
        {/if}
      </section>
    {/if}

    <section class="about">
      <h3>About</h3>
      <div class="row">
        <div class="text">
          <span class="label">yatta {version}</span>
          <span class="hint">
            Yet Another Text-based TODO App. Also 「やった」 — <em>yatta</em>, “did it!”
          </span>
        </div>
        <div class="actions">
          <button class="btn" onclick={() => openUrl(REPO)}>
            <Icon name="external" size={13} />Source
          </button>
          <button class="btn" onclick={() => openUrl(REPO + "/issues")}>Issues</button>
        </div>
      </div>
      <p class="note">MIT licensed · © 2026 Carlos Bravo</p>
    </section>

    <section class="shortcuts">
      <h3>Keyboard</h3>
      <dl>
        <div><dt><kbd>N</kbd></dt><dd>New task</dd></div>
        <div><dt><kbd>/</kbd></dt><dd>Search</dd></div>
        <div><dt><kbd>J</kbd><kbd>K</kbd></dt><dd>Move between tasks</dd></div>
        <div><dt><kbd>X</kbd></dt><dd>Complete the selected task</dd></div>
        <div><dt><kbd>E</kbd></dt><dd>Open the selected task</dd></div>
        <div><dt><kbd>&larr;</kbd><kbd>&rarr;</kbd></dt><dd>Move a card (board view)</dd></div>
        <div><dt><kbd>&larr;</kbd><kbd>&rarr;</kbd></dt><dd>Resize details (when the grip has focus)</dd></div>
        <div><dt><kbd>Esc</kbd></dt><dd>Close panel or clear search</dd></div>
        <div><dt><kbd>Ctrl</kbd><kbd>R</kbd></dt><dd>Reload from disk</dd></div>
      </dl>
    </section>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    /* No backdrop-filter: it is decorative, and in WebKitGTK it forces a
       composited layer that can render the overlay soft. */
    background: rgba(10, 12, 20, 0.5);
  }

  .modal {
    position: fixed;
    z-index: 41;
    /* Centred with margin rather than a -50% translate: the translate lands
       on half-pixels whenever the box has an odd dimension, and the height is
       content-driven, which is what made the text render blurry. */
    inset: 0;
    margin: auto;
    width: min(560px, calc(100vw - 48px));
    height: fit-content;
    max-height: min(680px, calc(100vh - 64px));
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
    align-items: center;
    justify-content: space-between;
    padding: 14px 12px 14px 20px;
    border-bottom: 1px solid var(--border);
  }
  h2 { font-size: 16px; font-weight: 650; }

  .scroll { overflow-y: auto; padding: 4px 20px 20px; }

  section { padding: 16px 0; border-bottom: 1px solid var(--border); }
  section:last-child { border-bottom: 0; }

  h3 {
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--text-faint);
    margin-bottom: 12px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 34px;
  }
  .row + .row { margin-top: 12px; }
  .toggle { cursor: pointer; }

  .text { min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .label { font-size: 13.5px; font-weight: 500; }
  .hint {
    font-size: 12px;
    line-height: 1.45;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .actions { display: flex; gap: 6px; flex: none; }

  .note {
    margin-top: 10px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-faint);
  }

  code {
    font-size: 11.5px;
    padding: 1px 5px;
    border-radius: 5px;
    background: var(--surface-2);
  }

  .segs { display: flex; gap: 2px; padding: 2px; border-radius: var(--radius-sm); background: var(--surface-2); flex: none; }
  .seg {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 11px;
    border-radius: 6px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-faint);
  }
  .seg:hover { color: var(--text); }
  .seg.on { background: var(--surface); color: var(--accent); box-shadow: var(--shadow-sm); }

  input[type="checkbox"] {
    appearance: none;
    flex: none;
    width: 38px;
    height: 22px;
    border-radius: 99px;
    background: var(--border-strong);
    position: relative;
    cursor: pointer;
    transition: background 160ms var(--ease);
  }
  input[type="checkbox"]::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 99px;
    background: #fff;
    box-shadow: var(--shadow-sm);
    transition: transform 160ms var(--ease);
  }
  input[type="checkbox"]:checked {
    background: linear-gradient(135deg, var(--accent), var(--accent-2));
  }
  input[type="checkbox"]:checked::after { transform: translateX(16px); }
  input[type="checkbox"]:disabled { opacity: 0.45; cursor: default; }

  .hk { width: 190px; flex: none; font-size: 12.5px; text-align: center; }

  .timepick {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
    padding: 2px 6px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }
  .timepick select {
    appearance: none;
    padding: 3px 6px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    font: inherit;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    color: var(--text);
    cursor: pointer;
  }
  .timepick select:hover { background: var(--surface); }
  .colon { color: var(--text-faint); font-variant-numeric: tabular-nums; }

  .about em { font-style: normal; color: var(--accent); }

  .shortcuts dl { display: grid; grid-template-columns: 1fr 1fr; gap: 8px 20px; margin: 0; }
  .shortcuts div { display: flex; align-items: center; gap: 8px; }
  dt { display: flex; gap: 3px; }
  dd { margin: 0; font-size: 12.5px; color: var(--text-dim); }

  kbd {
    display: inline-grid;
    place-items: center;
    min-width: 20px;
    height: 20px;
    padding: 0 5px;
    border-radius: 5px;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    background: var(--surface-2);
    font-family: inherit;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-dim);
  }
</style>
