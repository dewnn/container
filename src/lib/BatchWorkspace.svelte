<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { localizedTool, tools, type Field, type Tool } from "./tools";
  import { armCompletionSound, playCompletionSound } from "./completionSound";
  import { reportProblem } from "./toast";

  interface HistorySnapshot{selected:Tool;items:{path:string;status:string;progress:number;output?:string;error?:string}[];recursive:boolean}
  let { initialPath, language, availableEncoders, onhistorychange=()=>{}, onsessionchange=()=>{} }:{initialPath:string;language:"tr"|"en";availableEncoders:string[]|null;onhistorychange?:(undo:boolean,redo:boolean)=>void;onsessionchange?:(value:HistorySnapshot)=>void}=$props();
  const supported=["encode","proxy","remux","audio_convert","extract_audio","remove_audio","fix_timestamps","gif"];
  const batchTools=()=>tools.filter(tool=>supported.includes(tool.id)).map(tool=>{
    const copy=localizedTool(tool,language);
    if(copy.id==="encode"){
      const field=copy.fields.find(item=>item.key==="encoder");
      if(field)field.options=(field.options??[]).filter(option=>(availableEncoders??["libx264"]).includes(option.value));
    }
    const audioMode=copy.fields.find(item=>item.key==="audio_mode");
    if(audioMode)audioMode.options=(audioMode.options??[]).filter(option=>option.value!=="selected");
    return copy;
  });
  let selected:Tool=$state(batchTools()[0]);
  let items:{path:string;status:string;progress:number;output?:string;error?:string}[]=$state([]);
  let running=$state(false),cancelAll=$state(false),recursive=$state(false),aggregate=$state(0);
  let currentIndex=$state(-1);
  let history:HistorySnapshot[]=$state([]),historyIndex=$state(-1);
  let historyApplying=false;
  const name=(path:string)=>path.split(/[\\/]/).pop()??path;
  const clone=<T,>(value:T):T=>JSON.parse(JSON.stringify(value)) as T;
  const snapshot=():HistorySnapshot=>clone({selected,items,recursive});
  const signature=(value:HistorySnapshot)=>JSON.stringify(value);
  function commit(value:HistorySnapshot){if(historyApplying||running)return;if(historyIndex>=0&&signature(history[historyIndex])===signature(value))return;history=[...history.slice(0,historyIndex+1),value].slice(-80);historyIndex=history.length-1}
  function applyHistory(value:HistorySnapshot){historyApplying=true;const restored=clone(value);selected=batchTools().some(tool=>tool.id===restored.selected?.id)?restored.selected:batchTools()[0];items=restored.items;recursive=restored.recursive;aggregate=items.length?items.reduce((sum,item)=>sum+item.progress,0)/items.length:0;currentIndex=-1;requestAnimationFrame(()=>historyApplying=false)}
  export function undo(){if(running)return;commit(snapshot());if(historyIndex<=0)return;historyIndex--;applyHistory(history[historyIndex])}
  export function redo(){if(running||historyIndex>=history.length-1)return;historyIndex++;applyHistory(history[historyIndex])}
  export function exportSession(){return snapshot()}
  export function restoreSession(value:HistorySnapshot){applyHistory(value);requestAnimationFrame(()=>{history=[snapshot()];historyIndex=0})}
  $effect(()=>{const value=snapshot();if(historyApplying||running)return;const key=signature(value);const timer=window.setTimeout(()=>{const current=snapshot();if(!historyApplying&&!running&&key===signature(current))commit(value)},280);return()=>window.clearTimeout(timer)});
  $effect(()=>onhistorychange(!running&&historyIndex>0,!running&&historyIndex>=0&&historyIndex<history.length-1));
  $effect(()=>{const value=snapshot();if(historyApplying||running)return;const timer=window.setTimeout(()=>onsessionchange(value),350);return()=>window.clearTimeout(timer)});
  onMount(()=>{if(initialPath)addPaths([initialPath]);history=[snapshot()];historyIndex=0});
  const params=()=>Object.fromEntries(selected.fields.filter(field=>field.key!=="audio_track").map(field=>[field.key,String(field.value)]));
  function addPaths(paths:string[]){const known=new Set(items.map(item=>item.path.toLowerCase()));for(const path of paths)if(!known.has(path.toLowerCase())){items=[...items,{path,status:"waiting",progress:0}];known.add(path.toLowerCase())}}
  async function addFiles(){const result=await open({multiple:true,filters:[{name:"Media",extensions:["mp4","mkv","mov","avi","webm","m4v","mp3","wav","m4a","aac","flac","opus","ogg","jpg","jpeg","png","webp"]}]});if(Array.isArray(result))addPaths(result)}
  async function addFolder(){const folder=await open({directory:true,multiple:false});if(typeof folder==="string")addPaths(await invoke<string[]>("list_media_files",{folder,recursive}))}
  function chooseTool(event:Event){const id=(event.currentTarget as HTMLSelectElement).value;selected=batchTools().find(tool=>tool.id===id)??batchTools()[0]}
  function visible(field:Field){return field.key!=="audio_track"&&!(selected.id==="cut"&&field.key==="crf"&&String(selected.fields.find(item=>item.key==="cut_mode")?.value)==="lossless")}
  async function start(){
    if(running||!items.length)return;armCompletionSound();running=true;cancelAll=false;aggregate=0;let completed=0;
    let unlisten:UnlistenFn|null=null;
    unlisten=await listen<{percent:number}>("container-progress",event=>{if(currentIndex>=0){items[currentIndex].progress=event.payload.percent;aggregate=(currentIndex+event.payload.percent/100)/items.length*100;items=[...items]}});
    for(let index=0;index<items.length;index++){
      if(cancelAll)break;currentIndex=index;items[index]={...items[index],status:"running",progress:0,error:undefined};items=[...items];
      try{const result=await invoke<{output:string}>("run_operation",{request:{input:items[index].path,operation:selected.id,params:params()}});items[index]={...items[index],status:"complete",progress:100,output:result.output};completed++}
      catch(reason){items[index]={...items[index],status:String(reason).toLowerCase().includes("cancel")?"cancelled":"failed",error:String(reason)};reportProblem(reason)}
      aggregate=(index+1)/items.length*100;items=[...items];
    }
    unlisten?.();running=false;currentIndex=-1;if(!cancelAll&&completed>0)await playCompletionSound();
  }
  async function cancel(){cancelAll=true;await invoke("cancel_job")}
  async function removeOrCancel(index:number){if(running&&index===currentIndex){await invoke("cancel_job");return}if(!running||items[index].status==="waiting")items=items.filter((_,position)=>position!==index)}
</script>

<section class="batch-workspace">
  <aside class="batch-control panel">
    <div class="pane-head"><div><h3>{language==="tr"?"TOPLU İŞLEM":"BATCH QUEUE"}</h3><p>{language==="tr"?"tek seferde bir iş · güvenli varsayılan":"one job at a time · safe default"}</p></div></div>
    <label class="field"><span>{language==="tr"?"İŞLEM":"OPERATION"}</span><select value={selected.id} onchange={chooseTool}>{#each batchTools() as tool}<option value={tool.id}>{tool.title}</option>{/each}</select></label>
    {#each selected.fields as field}
      {#if visible(field)}<label class="field"><span>{field.label}</span>{#if field.type==="select"}<select bind:value={field.value}>{#each field.options??[] as option}<option value={option.value}>{option.label}</option>{/each}</select>{:else}<input type={field.type==="text"?"text":"number"} bind:value={field.value} min={field.min} max={field.max} step={field.step}>{/if}</label>{/if}
    {/each}
    <div class="batch-add"><button class="ghost" onclick={addFiles}>+ {language==="tr"?"DOSYA":"FILES"}</button><button class="ghost" onclick={addFolder}>+ {language==="tr"?"KLASÖR":"FOLDER"}</button></div>
    <label class="batch-check"><input type="checkbox" bind:checked={recursive}> {language==="tr"?"alt klasörleri de tara":"include subfolders"}</label>
    <small>{language==="tr"?"Alt klasör taraması yalnızca açıkça işaretlendiğinde çalışır. Hata alan dosya kuyruğu durdurmaz.":"Subfolders are scanned only when explicitly enabled. A failed file does not stop the queue."}</small>
    {#if running}<button class="run danger" onclick={cancel}>{language==="tr"?"TÜMÜNÜ İPTAL ET":"CANCEL ALL"}</button>{:else}<button class="run" onclick={start} disabled={!items.length}>▶ {language==="tr"?"KUYRUĞU BAŞLAT":"START QUEUE"}</button>{/if}
  </aside>
  <section class="batch-list panel">
    <div class="pane-head"><div><h3>{language==="tr"?"KUYRUK":"QUEUE"}</h3><p>{items.length} {language==="tr"?"dosya":"files"}</p></div><b>{aggregate.toFixed(0)}%</b></div>
    <div class="batch-total"><i style={`width:${aggregate}%`}></i></div>
    <div class="batch-items">{#each items as item,index}<article><span class="batch-index">{String(index+1).padStart(2,"0")}</span><div><b>{name(item.path)}</b><small>{item.error??item.output??item.status}</small><i><em style={`width:${item.progress}%`}></em></i></div><strong class:failed={item.status==="failed"}>{item.status}</strong><button onclick={()=>removeOrCancel(index)} disabled={running&&index!==currentIndex&&item.status!=="waiting"}>×</button></article>{/each}</div>
  </section>
</section>
