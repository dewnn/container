export interface Stage<T> { id:number; parent:number|null; label:string; session:T }
export interface StageHistory<T> { entries:Stage<T>[]; current:number; next:number; trimmed:boolean }
const copy = <T>(value:T):T => JSON.parse(JSON.stringify(value));
export function startStages<T>(session:T,label:string):StageHistory<T> {
  return {entries:[{id:1,parent:null,label,session:copy(session)}],current:1,next:2,trimmed:false};
}
function signature(value:unknown):string {
  return JSON.stringify(value,(key,item)=>{
    if(["savedAt","mediaUrl","renderedImageUrl","resources","stageHistory"].includes(key))return undefined;
    if(key==="selected"&&item&&Array.isArray(item.fields))return {id:item.id,fields:item.fields.map((field:{key:string;value:unknown})=>({key:field.key,value:field.value}))};
    return item;
  });
}
function limit<T>(history:StageHistory<T>,retainId?:number):StageHistory<T> {
  // Snapshots contain settings, never media bytes. Bound both count and storage.
  while(history.entries.length>2&&(history.entries.length>30||JSON.stringify(history).length>750_000)){
    const index=history.entries.findIndex(entry=>entry.id!==history.current&&entry.id!==retainId);
    history.entries.splice(index,1);history.trimmed=true;
  }
  const ids=new Set(history.entries.map(entry=>entry.id));
  for(const entry of history.entries)if(entry.parent!==null&&!ids.has(entry.parent))entry.parent=null;
  return history;
}
export function checkpointStage<T>(value:StageHistory<T>|null,session:T,label:string,retainId?:number):StageHistory<T> {
  if(!value)return startStages(session,label);
  const history=copy(value),entry=history.entries.find(item=>item.id===history.current)!;
  if(signature(entry.session)===signature(session))return history;
  if(history.entries.some(item=>item.parent===entry.id)){
    const id=history.next++;
    history.entries.push({id,parent:entry.id,label,session:copy(session)});history.current=id;
  }else{entry.session=copy(session);entry.label=label}
  return limit(history,retainId);
}
export function continueStage<T>(value:StageHistory<T>,session:T,label:string):StageHistory<T> {
  const history=copy(value),id=history.next++;
  history.entries.push({id,parent:history.current,label,session:copy(session)});history.current=id;
  return limit(history);
}
export function validStages<T>(value:unknown,validSession:(value:unknown)=>value is T):value is StageHistory<T> {
  if(!value||typeof value!=="object")return false;
  const h=value as StageHistory<T>;
  return Array.isArray(h.entries)&&h.entries.length>0&&h.entries.length<=30&&Number.isSafeInteger(h.next)&&
    h.entries.every(e=>e&&Number.isSafeInteger(e.id)&&e.id>0&&e.id<h.next&&typeof e.label==="string"&&validSession(e.session)&&!(e.session as any).stageHistory)&&
    new Set(h.entries.map(e=>e.id)).size===h.entries.length&&h.entries.some(e=>e.id===h.current)&&
    h.entries.every(e=>e.parent===null||h.entries.some(p=>p.id===e.parent&&p.id<e.id));
}
