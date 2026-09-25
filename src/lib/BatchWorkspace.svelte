<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount, type Snippet } from "svelte";
  import { localizedTool, tools, type Field, type Tool } from "./tools";
  import { armCompletionSound, playCompletionSound } from "./completionSound";
  import { reportProblem } from "./toast";

  interface HistorySnapshot{selected:Tool;items:{path:string;status:string;progress:number;output?:string;error?:string}[];recursive:boolean}
  let { initialPath, language, availableEncoders, historyControl, oncontinue, onhistorychange=()=>{}, onsessionchange=()=>{}, onbusychange=()=>{} }:{initialPath:string;language:"tr"|"en";availableEncoders:string[]|null;historyControl?:Snippet;oncontinue?:(path:string)=>Promise<void>;onhistorychange?:(undo:boolean,redo:boolean)=>void;onsessionchange?:(value:HistorySnapshot)=>void;onbusychange?:(value:boolean)=>void}=$props();
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
  let panelWorkspace:HTMLElement|null=$state(null);
  let panelWorkspaceWidth=$state(0);
  let controlPanelWidth=$state<number|null>(null);
  const panelStorageKey="container-batch-panel-widths";
  function panelSizes(){
    const compact=panelWorkspaceWidth<=950,padding=compact?7:12;
    const available=Math.max(0,panelWorkspaceWidth-padding*2-8);
    const minLeft=compact?235:260,minRight=compact?320:400;
    const left=Math.max(minLeft,Math.min(600,available-minRight,controlPanelWidth??(compact?235:340)));
    return {left,available,minLeft,minRight};
  }
  const sizes=$derived(panelSizes());
  $effect(()=>{
    const element=panelWorkspace;if(!element)return;
    const update=()=>panelWorkspaceWidth=element.clientWidth;
    update();const observer=new ResizeObserver(update);observer.observe(element);
    return()=>observer.disconnect();
  });
  function savePanelWidth(){try{localStorage.setItem(panelStorageKey,JSON.stringify({left:controlPanelWidth}))}catch{}}
  function setPanelWidth(value:number){const next=panelSizes();controlPanelWidth=Math.round(Math.max(next.minLeft,Math.min(600,next.available-next.minRight,value)))}
  function startPanelResize(event:PointerEvent){
    if(event.button!==0||!panelWorkspace)return;
    event.preventDefault();const target=event.currentTarget as HTMLElement,pointerId=event.pointerId;
    const startX=event.clientX,initial=panelSizes().left;
    controlPanelWidth=initial;target.setPointerCapture?.(pointerId);
    const oldCursor=document.body.style.cursor,oldSelection=document.body.style.userSelect;
    document.body.style.cursor="col-resize";document.body.style.userSelect="none";
    const move=(next:PointerEvent)=>{if(next.pointerId===pointerId)setPanelWidth(initial+next.clientX-startX)};
    const stop=(next?:PointerEvent)=>{
      if(next&&next.pointerId!==pointerId)return;
      window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",stop);window.removeEventListener("pointercancel",stop);window.removeEventListener("blur",onBlur);
      if(target.hasPointerCapture?.(pointerId))target.releasePointerCapture(pointerId);
      document.body.style.cursor=oldCursor;document.body.style.userSelect=oldSelection;savePanelWidth();
    };
    const onBlur=()=>stop();
    window.addEventListener("pointermove",move);window.addEventListener("pointerup",stop);window.addEventListener("pointercancel",stop);window.addEventListener("blur",onBlur);
  }
  function panelKey(event:KeyboardEvent){
    const next=panelSizes();let value=next.left;
    if(event.key==="Home")value=next.minLeft;
    else if(event.key==="End")value=Math.min(600,next.available-next.minRight);
    else if(event.key==="ArrowLeft"||event.key==="ArrowRight")value+=(event.key==="ArrowRight"?1:-1)*(event.shiftKey?50:20);
    else return;
    event.preventDefault();event.stopPropagation();setPanelWidth(value);savePanelWidth();
  }
  export function resetPanelWidths(){controlPanelWidth=null;savePanelWidth()}
  $effect(()=>onbusychange(running));
  const name=(path:string)=>path.split(/[\\/]/).pop()??path;
  const statusLabel=(status:string)=>language==="tr"?({waiting:"Bekliyor",running:"İşleniyor",complete:"Tamamlandı",cancelled:"İptal edildi",failed:"Başarısız"}[status]??status):status;
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
  onMount(()=>{
    try{const saved=JSON.parse(localStorage.getItem(panelStorageKey)??"null");if(saved&&typeof saved==="object"&&Number.isFinite(saved.left)&&saved.left>=235&&saved.left<=600)controlPanelWidth=saved.left}catch{}
    if(initialPath)addPaths([initialPath]);history=[snapshot()];historyIndex=0;
  });
  const params=()=>Object.fromEntries(selected.fields.filter(field=>field.key!=="audio_track").map(field=>[field.key,String(field.value)]));
  function addPaths(paths:string[]){if(running)return;const known=new Set(items.map(item=>item.path.toLowerCase()));for(const path of paths)if(!known.has(path.toLowerCase())){items=[...items,{path,status:"waiting",progress:0}];known.add(path.toLowerCase())}}
  async function addFiles(){const result=await open({multiple:true,filters:[{name:"Media",extensions:["mp4","mkv","mov","avi","webm","m4v","mp3","wav","m4a","aac","flac","opus","ogg","jpg","jpeg","png","webp"]}]});if(Array.isArray(result))addPaths(result)}
  async function addFolder(){const folder=await open({directory:true,multiple:false});if(typeof folder==="string")addPaths(await invoke<string[]>("list_media_files",{folder,recursive}))}
  function chooseTool(event:Event){if(running)return;const id=(event.currentTarget as HTMLSelectElement).value;selected=batchTools().find(tool=>tool.id===id)??batchTools()[0]}
  function visible(field:Field){return field.key!=="audio_track"&&!(selected.id==="cut"&&field.key==="crf"&&["lossless","smart"].includes(String(selected.fields.find(item=>item.key==="cut_mode")?.value)))}
  async function start(){
    if(running||!items.length)return;armCompletionSound();running=true;cancelAll=false;aggregate=0;let completed=0;
    const operation=selected.id,operationParams=params();
    items=items.map(item=>({...item,status:"waiting",progress:0,output:undefined,error:undefined}));
    let unlisten:UnlistenFn|null=null;
    try{unlisten=await listen<{percent:number}>("container-progress",event=>{if(currentIndex>=0){items[currentIndex].progress=event.payload.percent;aggregate=(currentIndex+event.payload.percent/100)/items.length*100;items=[...items]}})}catch(reason){running=false;reportProblem(reason);return}
    for(let index=0;index<items.length;index++){
      if(cancelAll)break;currentIndex=index;items[index]={...items[index],status:"running",progress:0,output:undefined,error:undefined};items=[...items];
      try{const result=await invoke<{output:string}>("run_operation",{request:{input:items[index].path,operation,params:operationParams}});items[index]={...items[index],status:"complete",progress:100,output:result.output};completed++}
      catch(reason){items[index]={...items[index],status:String(reason).toLowerCase().includes("cancel")?"cancelled":"failed",error:String(reason)};reportProblem(reason)}
      aggregate=(index+1)/items.length*100;items=[...items];
    }
    unlisten?.();running=false;currentIndex=-1;if(!cancelAll&&completed>0)await playCompletionSound();
  }
  async function cancel(){cancelAll=true;await invoke("cancel_job")}
  async function removeOrCancel(index:number){if(running&&index===currentIndex){await invoke("cancel_job");return}if(!running||items[index].status==="waiting")items=items.filter((_,position)=>position!==index)}
</script>

<section class="batch-workspace resizable" bind:this={panelWorkspace} style={`--batch-left:${sizes.left}px`}>
  <aside class="batch-control panel">
    <div class="pane-head"><div><h3>{language==="tr"?"TOPLU İŞLEM":"BATCH QUEUE"}</h3><p>{language==="tr"?"Dosyaların sırayla, güvenle işlenir":"one job at a time · safe default"}</p></div>{@render historyControl?.()}</div>
    <label class="field"><span>{language==="tr"?"İŞLEM":"OPERATION"}</span><select value={selected.id} onchange={chooseTool} disabled={running}>{#each batchTools() as tool}<option value={tool.id}>{tool.title}</option>{/each}</select></label>
    {#each selected.fields as field}
      {#if visible(field)}<label class="field"><span>{field.label}</span>{#if field.type==="select"}<select bind:value={field.value} disabled={running}>{#each field.options??[] as option}<option value={option.value}>{option.label}</option>{/each}</select>{:else}<input type={field.type==="text"?"text":"number"} bind:value={field.value} min={field.min} max={field.max} step={field.step} disabled={running}>{/if}</label>{/if}
    {/each}
    <div class="batch-add"><button class="ghost" onclick={addFiles} disabled={running}>+ {language==="tr"?"DOSYA":"FILES"}</button><button class="ghost" onclick={addFolder} disabled={running}>+ {language==="tr"?"KLASÖR":"FOLDER"}</button></div>
    <label class="batch-check"><input type="checkbox" bind:checked={recursive} disabled={running}> {language==="tr"?"alt klasörleri de tara":"include subfolders"}</label>
    <small>{language==="tr"?"Alt klasörleri istersen dahil et. Bir dosyada hata olursa diğerleri işlenmeye devam eder.":"Subfolders are scanned only when explicitly enabled. A failed file does not stop the queue."}</small>
    {#if running}<button class="run danger" onclick={cancel}>{language==="tr"?"TÜMÜNÜ İPTAL ET":"CANCEL ALL"}</button>{:else}<button class="run" onclick={start} disabled={!items.length}>▶ {language==="tr"?"KUYRUĞU BAŞLAT":"START QUEUE"}</button>{/if}
  </aside>
  <div class="workspace-resizer" role="slider" tabindex="0" aria-label={language==="tr"?"Batch kontrol paneli genişliği":"Batch controls panel width"} aria-orientation="horizontal" aria-valuemin={sizes.minLeft} aria-valuemax={Math.min(600,sizes.available-sizes.minRight)} aria-valuenow={Math.round(sizes.left)} onpointerdown={startPanelResize} onkeydown={panelKey} ondblclick={resetPanelWidths} title={language==="tr"?"Sürükle · sıfırla: çift tık":"Drag to resize · double-click to reset"}></div>
  <section class="batch-list panel">
    <div class="pane-head"><div><h3>{language==="tr"?"KUYRUK":"QUEUE"}</h3><p>{items.length} {language==="tr"?"dosya":"files"}</p></div><b>{aggregate.toFixed(0)}%</b></div>
    <div class="batch-total"><i style={`width:${aggregate}%`}></i></div>
    <div class="batch-items">{#each items as item,index}<article><span class="batch-index">{String(index+1).padStart(2,"0")}</span><div><b>{name(item.path)}</b><small>{item.error??item.output??statusLabel(item.status)}</small><i><em style={`width:${item.progress}%`}></em></i></div><strong class:failed={item.status==="failed"}>{statusLabel(item.status)}</strong><div class="batch-row-actions">{#if item.status==="complete" && item.output && oncontinue}<button disabled={running} title={language==="tr"?"çıktıyı düzenle":"continue editing"} aria-label={language==="tr"?"çıktıyı düzenle":"continue editing"} onclick={()=>oncontinue?.(item.output!)}>↗</button>{/if}<button onclick={()=>removeOrCancel(index)} disabled={running&&index!==currentIndex&&item.status!=="waiting"} aria-label={language==="tr"?"Kuyruktan kaldır":"Remove from queue"}>×</button></div></article>{/each}</div>
  </section>
</section>
