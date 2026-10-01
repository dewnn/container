// Keep keyboard focus in custom modal layers; return it to the opener on close.
export function containDialog(node:HTMLDialogElement){
  const previous=document.activeElement as HTMLElement|null;
  const targets=()=>Array.from(node.querySelectorAll<HTMLElement>('button,input,select,textarea,a[href],[tabindex]')).filter(item=>!item.matches(':disabled,[tabindex="-1"]')&&item.getClientRects().length>0);
  const timer=window.setTimeout(()=>{if(!node.contains(document.activeElement))targets()[0]?.focus()},0);
  const key=(event:KeyboardEvent)=>{if(event.key!=="Tab")return;const list=targets();if(!list.length){event.preventDefault();node.focus();return}const first=list[0],last=list[list.length-1];if(event.shiftKey&&(document.activeElement===first||!node.contains(document.activeElement))){event.preventDefault();last.focus()}else if(!event.shiftKey&&(document.activeElement===last||!node.contains(document.activeElement))){event.preventDefault();first.focus()}};
  document.addEventListener("keydown",key);
  return {destroy(){window.clearTimeout(timer);document.removeEventListener("keydown",key);if(previous?.isConnected)previous.focus()}};
}
