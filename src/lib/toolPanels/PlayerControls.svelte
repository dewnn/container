<script lang="ts">
  let { language, fullscreenToolboxPlayer, hoverPlayerSeek, playbackDuration, playerSeekHover=$bindable(), playerTime, precisionPlayerSeek, seekToolbox, seekToolboxBy, toggleToolboxPlayer, toolboxCurrent, toolboxPlaying, toolboxVideo, toolboxVolume=$bindable(), wheelPlayerSeek }:{ language:"tr"|"en"; fullscreenToolboxPlayer:()=>unknown; hoverPlayerSeek:(event:PointerEvent)=>void; playbackDuration:number; playerSeekHover:{percent:number;time:number;precision:boolean}|null; playerTime:(value:number)=>string; precisionPlayerSeek:(event:PointerEvent)=>void; seekToolbox:(value:number)=>void; seekToolboxBy:(seconds:number)=>void; toggleToolboxPlayer:()=>void; toolboxCurrent:number; toolboxPlaying:boolean; toolboxVideo:HTMLVideoElement|null; toolboxVolume:number; wheelPlayerSeek:(event:WheelEvent)=>void }=$props();
</script>

<div class="ac-controls">
  {#if playerSeekHover}<span class="player-seek-tooltip mono" class:precision={playerSeekHover.precision} style:left={`${playerSeekHover.percent}%`}>{playerTime(playerSeekHover.time)}</span>{/if}
  <input class="player-seek" style={`--seek-pct:${playbackDuration?Math.min(100,toolboxCurrent/playbackDuration*100):0}%`} aria-label={language==="tr"?"Video konumu":"Video position"} type="range" min="0" max={playbackDuration} step="0.01" value={toolboxCurrent} onpointerdown={precisionPlayerSeek} onpointermove={hoverPlayerSeek} onpointerleave={()=>playerSeekHover=null} onwheel={wheelPlayerSeek} oninput={event=>seekToolbox(Number(event.currentTarget.value))}>
  <button onclick={()=>seekToolboxBy(-5)} title={language==="tr"?"5 saniye geri":"5 seconds back"}>−5</button>
  <button class="play" onclick={toggleToolboxPlayer} title={language==="tr"?"Oynat / Duraklat":"Play / Pause"}>{toolboxPlaying?"Ⅱ":"▶"}</button>
  <button onclick={()=>seekToolboxBy(5)} title={language==="tr"?"5 saniye ileri":"5 seconds forward"}>+5</button>
  <span class="ac-time mono">{playerTime(toolboxCurrent)} <i>/</i> {playerTime(playbackDuration)}</span>
  <input class="volume" aria-label={language==="tr"?"Ses düzeyi":"Volume"} type="range" min="0" max="1" step="0.05" bind:value={toolboxVolume} oninput={()=>{if(toolboxVideo)toolboxVideo.volume=toolboxVolume}}>
  <button onclick={fullscreenToolboxPlayer} title={language==="tr"?"Tam ekran":"Fullscreen"}>⛶</button>
</div>
