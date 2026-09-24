export interface ProjectResource { label: string; path: string; relativePath?: string }

export function projectResources(session: unknown, includeHistory=false): ProjectResource[] {
  if (!session || typeof session !== "object") return [];
  const value = session as Record<string, any>;
  const collected: ProjectResource[] = [];
  const add = (label: string, path: unknown) => {
    if (typeof path === "string" && path.trim() && !collected.some(item => item.path === path)) collected.push({ label, path });
  };
  add("Source media", value.mediaPath);
  for (const field of value.toolbox?.selected?.fields ?? []) if (field?.type === "file") add(String(field.label ?? field.key), field.value);
  for (const path of value.toolbox?.mergeInputs ?? []) add("Merge video", path);
  for (const track of value.autocut?.linkedTracks ?? []) add("SmartCut linked track", track?.path);
  add("SmartCut analysis track", value.autocut?.analysisInput);
  for (const item of value.batch?.items ?? []) add("Batch input", item?.path);
  if(includeHistory){
    for(const entry of value.stageHistory?.entries??[])for(const resource of projectResources(entry.session))add(resource.label,resource.path);
    const sessions=[value,...(value.stageHistory?.entries??[]).map((entry:any)=>entry.session)];
    for(const snapshot of sessions){
      add("Stage output",snapshot.toolbox?.output);add("SmartCut output",snapshot.autocut?.output);
      for(const item of snapshot.batch?.items??[])add("Batch output",item?.output);
    }
  }
  return collected;
}

export function replaceProjectResource<T>(session: T, previous: string, replacement: string): T {
  const walk = (value: unknown): unknown => {
    if (value === previous) return replacement;
    if (Array.isArray(value)) return value.map(walk);
    if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, walk(item)]));
    return value;
  };
  return walk(session) as T;
}
