<script lang="ts">
  import {remainingSeconds} from "./renderFeedback";
  import JobInspector from "./JobInspector.svelte";
  let {running,progress,language,scope="render",speed="—",elapsed=0,output=""}:{running:boolean;progress:number;language:"tr"|"en";scope?:string;speed?:string;elapsed?:number;output?:string}=$props();
  let seconds=$state(0);
  $effect(()=>{if(!running){seconds=0;return}const start=performance.now();const timer=window.setInterval(()=>seconds=(performance.now()-start)/1000,1000);return()=>window.clearInterval(timer)});
  const remaining=$derived(remainingSeconds(progress,seconds));
</script>
<div class="render-feedback" data-scope={scope}>
  {#if running}<span>{language==="tr"?"Tahmini kalan":"Estimated remaining"}: {remaining===null?"—":remaining<60?`~${remaining}s`:`~${Math.ceil(remaining/60)} ${language==="tr"?"dk":"min"}`}</span>{/if}
</div>
<JobInspector {language} {speed} {elapsed} {output}/>
<style>.render-feedback{display:flex;flex-wrap:wrap;gap:6px 12px;align-items:center;font-size:10px;color:var(--muted);padding:5px 0}</style>
