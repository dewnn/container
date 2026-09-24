<script lang="ts">
  import { tick } from "svelte";
  import type { StageHistory } from "./stageHistory";

  let { history, language, busy = false, currentLabel, onselect }: {
    history: StageHistory<unknown> | null;
    language: "tr" | "en";
    busy?: boolean;
    currentLabel?: string;
    onselect: (id: number) => void | Promise<void>;
  } = $props();
  const uid = $props.id();
  const tr = $derived(language === "tr");
  let expanded = $state(false);
  let root: HTMLDivElement | undefined = $state();
  let trigger: HTMLButtonElement | undefined = $state();
  let menu: HTMLDivElement | undefined = $state();
  let position = $state({ left: 0, top: 0, height: 400 });

  function close(returnFocus = false) {
    expanded = false;
    if (returnFocus) trigger?.focus();
  }
  $effect(() => { if (busy) close(); });

  async function open() {
    if (busy || !trigger) return;
    const rect = trigger.getBoundingClientRect();
    const width = Math.min(320, window.innerWidth - 24);
    position = { left: Math.max(12, Math.min(rect.left, window.innerWidth - width - 12)), top: rect.bottom + 8, height: window.innerHeight - rect.bottom - 20 };
    expanded = true;
    await tick();
    const current = menu?.querySelector<HTMLButtonElement>('[aria-current="step"]');
    current?.focus({ preventScroll: true });
    current?.scrollIntoView({ block: "nearest" });
  }
  async function choose(id: number) {
    if (busy || id === history?.current) return;
    close();
    await onselect(id);
    await tick();
    trigger?.focus();
  }
  function menuKey(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === "Escape") { event.preventDefault(); close(true); return; }
    const rows = Array.from(menu?.querySelectorAll<HTMLButtonElement>(".stage-list button") ?? []);
    const index = rows.indexOf(document.activeElement as HTMLButtonElement);
    let next = index;
    if (event.key === "ArrowDown") next = Math.min(rows.length - 1, index + 1);
    else if (event.key === "ArrowUp") next = Math.max(0, index - 1);
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = rows.length - 1;
    else return;
    event.preventDefault();
    rows[next]?.focus();
  }
  function fileName(session: unknown) {
    const path = (session as { mediaPath?: string })?.mediaPath ?? "";
    return { path, name: path.split(/[\\/]/).pop() ?? path };
  }
  function branched(parent: number | null) {
    return parent !== null && (history?.entries.filter(entry => entry.parent === parent).length ?? 0) > 1;
  }
</script>

<svelte:window onresize={() => close()} onclick={(event) => {
  if (event.target instanceof Node && !root?.contains(event.target)) close();
}} />
{#if history && history.entries.length > 1}
  <div class="stage-history" bind:this={root} onfocusout={(event) => {
    if (event.relatedTarget instanceof Node && !root?.contains(event.relatedTarget)) close();
  }}>
    <button bind:this={trigger} class="stage-trigger" data-current-stage={history.current}
      disabled={busy} aria-expanded={expanded} aria-haspopup="dialog" aria-controls={expanded ? uid : undefined}
      onclick={() => expanded ? close(true) : open()}
      onkeydown={(event) => {
        if (["ArrowDown", "ArrowUp", " ", "Enter", "Escape"].includes(event.key)) event.stopPropagation();
        if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); void open(); }
        if (event.key === "Escape") { event.preventDefault(); close(true); }
      }}>{tr ? "GERİ DÖN" : "GO BACK"} ▾</button>
    {#if expanded}
      <div bind:this={menu} id={uid} class="stage-menu" style:left={position.left + "px"}
        style:top={position.top + "px"} style:max-height={position.height + "px"}
        role="dialog" tabindex="-1" aria-labelledby={uid + "-title"} aria-describedby={uid + "-help"} onkeydown={menuKey}>
        <div class="stage-heading">
          <strong id={uid + "-title"}>{tr ? "İşlem geçmişi" : "History"}</strong>
          <span>{history.entries.length} {tr ? "adım" : "steps"}</span>
          <button class="stage-close" aria-label={tr ? "Kapat" : "Close history"} onclick={() => close(true)}>×</button>
        </div>
        <div class="stage-list">
          {#each history.entries as entry (entry.id)}
            {@const current = entry.id === history.current}
            {@const file = fileName(entry.session)}
            <button aria-current={current ? "step" : undefined} aria-disabled={busy || current}
              title={file.path} onclick={() => choose(entry.id)}>
              <b>{entry.id}</b>
              <span class="stage-details">
                <span class="stage-label">{current ? currentLabel || entry.label : entry.label}</span>
                <small>{file.name || (tr ? "Medya" : "Media")}</small>
              </span>
              {#if branched(entry.parent)}<span class="stage-branch" title={tr ? entry.parent + ". adımdan devam edildi" : "Continued from step " + entry.parent}>↳ {entry.parent}</span>{/if}
              {#if current}<span class="stage-current">{tr ? "Şu an" : "Current"}</span>{/if}
            </button>
          {/each}
        </div>
        <p id={uid + "-help"}>{tr ? "Geri dönmek çıktılarını silmez." : "Going back keeps your output files."}
          {#if history.trimmed}<span>{tr ? "Yalnızca son adımlar tutuluyor." : "Only recent steps are retained."}</span>{/if}
        </p>
      </div>
    {/if}
  </div>
{/if}

<style>
  .stage-history{position:relative;flex-shrink:0}
  .stage-trigger{font:10px var(--mono);padding:6px 8px;white-space:nowrap;border:1px solid var(--border);background:var(--surface);color:var(--text);border-radius:5px;cursor:pointer}
  .stage-trigger[aria-expanded="true"]{border-color:var(--border-strong);background:var(--surface-2)}
  .stage-menu{position:fixed;box-sizing:border-box;width:min(320px,calc(100vw - 24px));z-index:50;display:flex;flex-direction:column;background:var(--panel);border:1px solid var(--border-strong);border-radius:9px;padding:5px;box-shadow:0 12px 35px #0003;color:var(--text);font-family:var(--sans)}
  .stage-heading{display:flex;align-items:center;gap:8px;padding:5px 5px 9px;flex-shrink:0}
  .stage-heading strong{font:600 12px var(--sans)}
  .stage-heading>span{color:var(--muted);font:10px var(--sans);margin-left:auto}
  .stage-close{display:grid;place-items:center;width:25px;height:25px;border:0;border-radius:4px;background:transparent;color:var(--muted);font:20px/1 var(--sans);cursor:pointer}
  .stage-close:hover{background:var(--surface-2);color:var(--text)}
  .stage-list{min-height:0;max-height:300px;overflow:auto;overscroll-behavior:contain}
  .stage-list button{display:flex;align-items:center;gap:9px;width:100%;text-align:left;padding:9px 7px;border:0;border-radius:5px;background:transparent;color:var(--text);cursor:pointer}
  .stage-list button:hover,.stage-list button[aria-current]{background:var(--surface-2)}
  .stage-list button[aria-current]{opacity:1;cursor:default}
  button:focus-visible{outline:2px solid var(--blue);outline-offset:-2px}
  .stage-list b{display:grid;place-items:center;flex-shrink:0;width:25px;height:25px;border:1px solid var(--border);border-radius:5px;color:var(--muted);font:11px var(--mono)}
  .stage-list button[aria-current] b{color:var(--blue);border-color:var(--blue)}
  .stage-details{flex:1;min-width:0}
  .stage-label,.stage-details small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .stage-label{font:500 12px/1.4 var(--sans)}
  .stage-details small{color:var(--muted);font:10px/1.4 var(--sans);margin-top:2px}
  .stage-current{font:10px var(--sans);color:var(--blue);white-space:nowrap}
  .stage-branch{font:10px var(--mono);color:var(--muted);white-space:nowrap}
  .stage-menu p{flex-shrink:0;border-top:1px solid var(--border);padding:10px 7px 5px;margin:5px 0 0;color:var(--muted);font:10px/1.5 var(--sans)}
  .stage-menu p span{display:block;margin-top:2px}
</style>
