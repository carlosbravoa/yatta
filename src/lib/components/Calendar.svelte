<script lang="ts">
  import { fade } from "svelte/transition";
  import {
    bucketByDate, dayLabel, firstDayOfWeek, isSameMonth, monthGrid, monthLabel,
    monthOf, shiftMonth, weekdayLabels,
  } from "../calendar";
  import { todayISO } from "../dates";
  import { store } from "../store.svelte";
  import Icon from "./Icon.svelte";
  import TaskRow from "./TaskRow.svelte";

  const weekStart = firstDayOfWeek();
  const today = todayISO();

  let anchor = $state(monthOf(todayISO()));
  let selected = $state<string | null>(todayISO());

  const grid = $derived(monthGrid(anchor, weekStart));
  const weekdays = $derived(weekdayLabels(weekStart));

  /* Indexed over every task, archived included: archiving is only a file move,
     and excluding those would erase the record of what was actually finished.
     The search box still applies, which is what makes "when did I do X?"
     answerable. */
  const index = $derived(bucketByDate(store.matching));

  const day = $derived(selected ? index.get(selected) : undefined);

  function go(delta: number) {
    anchor = shiftMonth(anchor, delta);
  }

  function toToday() {
    anchor = monthOf(today);
    selected = today;
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === "ArrowLeft") { event.preventDefault(); go(-1); }
    else if (event.key === "ArrowRight") { event.preventDefault(); go(1); }
  }
</script>

<div class="calendar">
  <header>
    <h2>{monthLabel(anchor)}</h2>
    <div class="nav">
      <button class="btn icon" onclick={() => go(-1)} aria-label="Previous month">
        <Icon name="chevronRight" size={14} />
      </button>
      <button class="btn today" onclick={toToday}>Today</button>
      <button class="btn icon" onclick={() => go(1)} aria-label="Next month">
        <Icon name="chevronRight" size={14} />
      </button>
    </div>
  </header>

  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="grid" role="grid" tabindex="-1" onkeydown={onKey}>
    <div class="weekdays" role="row">
      {#each weekdays as name (name)}<span role="columnheader">{name}</span>{/each}
    </div>

    {#each grid as week, w (w)}
      <div class="week" role="row">
        {#each week as date (date)}
          {@const buckets = index.get(date)}
          {@const outside = !isSameMonth(date, anchor)}
          <button
            class="day"
            class:outside
            class:today={date === today}
            class:selected={date === selected}
            class:empty={!buckets}
            role="gridcell"
            onclick={() => (selected = date)}
          >
            <span class="num">{Number(date.slice(8))}</span>
            {#if buckets}
              <span class="marks">
                {#if buckets.done.length}
                  <span class="mark done" title="{buckets.done.length} completed">
                    {buckets.done.length} done
                  </span>
                {/if}
                {#if buckets.due.length}
                  <span
                    class="mark due"
                    class:overdue={date < today}
                    title="{buckets.due.length} {date < today ? 'still open' : 'due'}"
                  >
                    {buckets.due.length} {date < today ? "open" : "due"}
                  </span>
                {/if}
              </span>
            {/if}
          </button>
        {/each}
      </div>
    {/each}
  </div>

  {#if selected}
    <section class="detail" in:fade={{ duration: 140 }}>
      <h3>{dayLabel(selected)}</h3>

      {#if !day || (day.done.length === 0 && day.due.length === 0)}
        <p class="nothing">
          {selected < today ? "Nothing recorded for this day." : "Nothing scheduled."}
        </p>
      {:else}
        {#if day.done.length}
          <div class="band">
            <span class="badge done"><Icon name="check" size={11} stroke={3} /></span>
            <span>Completed</span>
            <span class="count">{day.done.length}</span>
          </div>
          <div class="rows">
            {#each day.done as task (task.path)}<TaskRow {task} />{/each}
          </div>
        {/if}

        {#if day.due.length}
          <div class="band">
            <span class="badge due"><Icon name="calendar" size={11} /></span>
            <span>{selected < today ? "Still open" : "Due"}</span>
            <span class="count">{day.due.length}</span>
          </div>
          <div class="rows">
            {#each day.due as task (task.path)}<TaskRow {task} />{/each}
          </div>
        {/if}
      {/if}
    </section>
  {/if}
</div>

<style>
  .calendar { display: flex; flex-direction: column; gap: 16px; padding-bottom: 32px; }

  header { display: flex; align-items: center; gap: 12px; }
  h2 {
    flex: 1;
    font-size: 17px;
    font-weight: 650;
    letter-spacing: -0.015em;
    text-transform: capitalize;
  }
  .nav { display: flex; align-items: center; gap: 2px; }
  .nav .btn.icon:first-child :global(.icon) { transform: rotate(180deg); }
  .today { height: 28px; font-size: 12.5px; }

  .grid { display: flex; flex-direction: column; gap: 4px; outline: none; }
  .weekdays, .week { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 4px; }
  .weekdays span {
    padding: 0 0 2px 4px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .day {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 5px;
    min-height: 74px;
    padding: 7px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface);
    text-align: left;
    transition: border-color 110ms var(--ease), background 110ms var(--ease);
  }
  .day:hover { border-color: var(--border-strong); }
  .day.outside { opacity: 0.45; }
  .day.empty { background: transparent; }
  .day.today .num {
    color: #fff;
    background: linear-gradient(135deg, var(--accent), var(--accent-2));
  }
  .day.selected {
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    box-shadow: 0 0 0 2px var(--accent-soft);
  }

  .num {
    display: grid;
    place-items: center;
    min-width: 21px;
    height: 21px;
    padding: 0 5px;
    border-radius: 99px;
    font-size: 12px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
  }

  /* Counts carry their noun. A bare digit reads as ambiguous -- done? due? --
     and two lone numbers side by side look like a score line. */
  .marks { display: flex; flex-wrap: wrap; gap: 3px; }
  .mark {
    height: 18px;
    padding: 0 7px;
    border-radius: 99px;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    white-space: nowrap;
    font-size: 10.5px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  /* Done is the quieter, settled colour; due is the one that wants attention. */
  .mark.done { color: var(--p-low); background: color-mix(in srgb, var(--p-low) 18%, transparent); }
  .mark.due { color: var(--accent); background: var(--accent-soft); }
  .mark.due.overdue { color: var(--overdue); background: color-mix(in srgb, var(--overdue) 16%, transparent); }

  .detail { display: flex; flex-direction: column; gap: 6px; padding-top: 6px; }
  h3 {
    font-size: 13px;
    font-weight: 650;
    text-transform: capitalize;
    padding-bottom: 4px;
  }
  .nothing { font-size: 13px; color: var(--text-faint); padding: 6px 2px 14px; }

  .band {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 12px 2px 6px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  .badge { display: grid; place-items: center; width: 17px; height: 17px; border-radius: 99px; }
  .badge.done { color: var(--p-low); background: color-mix(in srgb, var(--p-low) 18%, transparent); }
  .badge.due { color: var(--accent); background: var(--accent-soft); }
  .count {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0;
    color: var(--text-faint);
    background: var(--surface-2);
    border-radius: 99px;
    padding: 1px 7px;
  }

  .rows { display: flex; flex-direction: column; }
  .rows :global(.row + .row) { border-top: 1px solid var(--border); }

  @media (max-width: 720px) {
    .day { min-height: 58px; }
    .marks { gap: 2px; }
  }
</style>
