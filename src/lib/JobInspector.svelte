<script lang="ts">
  import {invoke} from "@tauri-apps/api/core";
  let {language,speed="—",elapsed=0,output=""}:{language:"tr"|"en";speed?:string;elapsed?:number;output?:string}=$props();
  let opened=$state(false),attempts=$state<{args:string[];errors:string}[]>([]),error=$state("");
  $effect(()=>{if(!opened)return;let disposed=false;async function refresh(){try{const result=await invoke<typeof attempts>("job_inspector");if(!disposed)attempts=result??[]}catch(reason){if(!disposed)error=String(reason)}}void refresh();const timer=setInterval(refresh,1000);return()=>{disposed=true;clearInterval(timer)}});
  function quote(arg:string){return '"'+arg.replace(/(\\*)"/g,'$1$1\\"').replace(/(\\+)$/,'$1$1')+'"'}
  function safe(value:string){return /(?:[A-Za-z]:[\\/]|https?:\/\/|\\\\)/i.test(value)?"[redacted path or URL]":value}
  async function copy(report:boolean){try{const text=report?JSON.stringify({speed,elapsed,output:safe(output),attempts:attempts.map(a=>({args:a.args.map(safe),errors:a.errors.split(/\r?\n/).map(line=>safe(line)).join("\n")}))},null,2):attempts.map(a=>"ffmpeg "+a.args.map(quote).join(" ")).join("\n\n");await navigator.clipboard.writeText(text);error=""}catch(reason){error=String(reason)}}
</script>
<details ontoggle={(event)=>opened=event.currentTarget.open}>
  <summary>{language==="tr"?"İşlem ayrıntıları":"Job details"}</summary>
  {#if attempts.length}<p>{language==="tr"?"Kodlayıcı":"Encoder"}: {attempts.at(-1)?.args.reduce((found,arg,index,args)=>arg==="-c:v"?args[index+1]??found:found,"—")} · {language==="tr"?"Hız":"Speed"}: {speed} · {language==="tr"?"Geçen":"Elapsed"}: {elapsed.toFixed(1)}s</p>{/if}
  {#if output}<p>{language==="tr"?"Çıktı":"Output"}: {output}</p>{/if}
  <button onclick={()=>copy(false)} disabled={!attempts.length}>{language==="tr"?"Komutu kopyala":"Copy command"}</button>
  <button onclick={()=>copy(true)} disabled={!attempts.length}>{language==="tr"?"Tanılama raporunu kopyala":"Copy diagnostic report"}</button>
  {#each attempts as attempt,index}<p>{language==="tr"?"Deneme":"Attempt"} {index+1}</p><pre>{"ffmpeg "+attempt.args.map(quote).join(" ")}</pre>{#if attempt.errors}<pre>{attempt.errors}</pre>{/if}{/each}
  {#if error}<p role="alert">{error}</p>{/if}
</details>
<style>details{font-size:11px;max-width:100%}summary{cursor:pointer}pre{white-space:pre-wrap;overflow-wrap:anywhere;max-height:180px;overflow:auto;background:var(--panel);padding:8px}button{margin:6px 6px 0 0}</style>
