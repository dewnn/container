import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";

const dialogMessagesAllowed = JSON.parse(readFileSync(new URL("../../src-tauri/capabilities/default.json", import.meta.url), "utf8")).permissions.includes("dialog:allow-message");

const sample = {
  path: "C:\\fixtures\\sample.mp4", name: "sample.mp4", kind: "video", duration: 60,
  width: 1920, height: 1080, fps: 30, codec: "h264", audio_codec: "aac",
  audio_tracks: [], pixel_format: "yuv420p", bits_per_raw_sample: 8,
  color_transfer: null, color_primaries: null, color_space: null,
  bitrate: 3_000_000, size: 1_000_000, start_timecode: null,
};

async function mockDesktop(page: Page, options: { invalidSecond?: boolean; interrupted?: boolean; startupPath?: string | null; savedRecovery?: boolean; missingProjectSource?: boolean; fixture?: typeof sample } = {}) {
  await page.addInitScript(({ media, invalidSecond, interrupted, startupPath, savedRecovery, missingProjectSource, dialogMessagesAllowed }) => {
    const callbacks = new Map<number, (...args: any[]) => void>();
    const events = new Map<string, number>();
    let callbackId = 0;
    let fileOpens = 0;
    (window as any).__TEST_CALLS__ = [];
    const mock: Record<string, any> = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
      transformCallback(callback: (...args: any[]) => void) { const id = ++callbackId; callbacks.set(id, callback); return id; },
      unregisterCallback(id: number) { callbacks.delete(id); },
      convertFileSrc(path: string) { return `http://asset.localhost/${encodeURIComponent(path)}`; },
      async invoke(cmd: string, args: Record<string, any> = {}) {
        (window as any).__TEST_CALLS__.push({cmd,args});
        if (cmd === "plugin:dialog|message" && !dialogMessagesAllowed) throw new Error("dialog.message not allowed");
        const override = (window as any).__TEST_HANDLER__?.(cmd,args);
        if (override !== undefined) return await override;
        if (cmd === "plugin:event|listen") { events.set(args.event, args.handler); return ++callbackId; }
        if (cmd === "plugin:event|unlisten") return null;
        if (cmd === "plugin:dialog|open") {
          fileOpens += 1;
          if (args.options?.multiple) return ["C:\\fixtures\\second.mp4"];
          return invalidSecond && fileOpens > 1 ? "C:\\fixtures\\broken.mp4" : media.path;
        }
        if (cmd === "plugin:dialog|save") return "C:\\fixtures\\sample.containerproject";
        if (cmd === "plugin:dialog|message") return "Cancel";
        if (cmd === "write_project") return null;
        if (cmd === "plugin:app|version") return "0.16.0-dev.1";
        if (cmd === "startup_media_path") return startupPath ?? null;
        if (cmd === "previous_session_interrupted") return !!interrupted;
        if (cmd === "ffmpeg_status") return { ready: true, ffmpeg_version: "9.0.1", ffprobe_version: "9.0.1" };
        if (cmd === "ffmpeg_capabilities") return { vidstab: true, subtitles: true, overlay: true, blur: true, concat: true };
        if (cmd === "downloader_status") return { ready: true, version: "test" };
        if (cmd === "ffmpeg_runtime_error") return null;
        if (cmd === "auto_encoder_configured") return true;
        if (cmd === "available_encoders") return ["libx264"];
        if (cmd === "warm_up_auto_encoder") return "libx264";
        if (cmd === "probe_media") {
          if (String(args.path).includes("broken")) throw new Error("Invalid media fixture");
          return { ...media, path: args.path };
        }
        if (cmd === "authorize_media_preview") return null;
        if (cmd === "project_media_available") return !missingProjectSource;
        if (cmd === "compute_video_filmstrip") return "";
        if (cmd === "autocut_presets" || cmd === "compute_autocut_waveform") return [];
        if (cmd === "analyze_autocut") return {cuts:[{start:2,end:8,enabled:true},{start:15,end:25,enabled:true}],waveform:[],duration:60};
        return null;
      },
    };
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: mock });
    (window as any).__TEST_DROP__ = (path: string) => {
      const id = events.get("tauri://drag-drop");
      if (id) callbacks.get(id)?.({ event: "tauri://drag-drop", payload: { paths: [path], position: { x: 0, y: 0 } } });
    };
    Object.defineProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__", { value: { unregisterListener() {} } });
    if (!localStorage.getItem("container-language")) localStorage.setItem("container-language", "en");
    if (savedRecovery) localStorage.setItem("container-recovery-v1", JSON.stringify({version:1,savedAt:Date.now(),mediaPath:media.path,workspaceMode:"toolbox",toolbox:null,autocut:null,batch:null}));
  }, { media: options.fixture??sample, dialogMessagesAllowed, ...options });
  await page.goto("/");
  if (!options.startupPath) await expect(page.locator(".dropzone")).toBeVisible();
}

async function openFixture(page: Page) {
  await page.locator(".dropzone").click();
  await expect(page.locator(".settings")).toBeVisible();
}

test("Toolbox side panels resize, preserve preview space and remember widths",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.setViewportSize({width:1440,height:850});
  const panels=page.locator(".workspace");
  const left=page.locator(".tool-pane"),middle=page.locator(".center-stack"),right=page.locator(".settings");
  const dividers=page.locator(".workspace-resizer");
  await expect(dividers).toHaveCount(2);
  const before={left:(await left.boundingBox())!,middle:(await middle.boundingBox())!,right:(await right.boundingBox())!};
  const first=(await dividers.nth(0).boundingBox())!;
  await page.mouse.move(first.x+first.width/2,first.y+first.height/2);
  await page.mouse.down();await page.mouse.move(first.x+first.width/2+90,first.y+first.height/2,{steps:8});await page.mouse.up();
  const widened={left:(await left.boundingBox())!,middle:(await middle.boundingBox())!,right:(await right.boundingBox())!};
  expect(widened.left.width-before.left.width).toBeGreaterThan(75);
  expect(before.middle.width-widened.middle.width).toBeGreaterThan(75);
  expect(Math.abs(widened.right.width-before.right.width)).toBeLessThan(3);
  const second=(await dividers.nth(1).boundingBox())!;
  await page.mouse.move(second.x+second.width/2,second.y+second.height/2);
  await page.mouse.down();await page.mouse.move(second.x+second.width/2-70,second.y+second.height/2,{steps:8});await page.mouse.up();
  expect((await right.boundingBox())!.width-widened.right.width).toBeGreaterThan(55);
  const rightBeforeKey=(await right.boundingBox())!.width;
  await dividers.nth(1).focus();await page.keyboard.press("ArrowRight");
  expect(rightBeforeKey-(await right.boundingBox())!.width).toBeGreaterThan(15);
  const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem("container-toolbox-panel-widths")??"null"));
  expect(saved.left).toBeGreaterThan(before.left.width+75);
  expect(saved.right).toBeGreaterThan(before.right.width+30);
  for(const width of [1100,900,860]){
    await page.setViewportSize({width,height:700});
    await expect.poll(async()=>((await middle.boundingBox())?.width??0)).toBeGreaterThan(315);
    const bounds=(await panels.boundingBox())!;
    expect((await right.boundingBox())!.x+(await right.boundingBox())!.width).toBeLessThanOrEqual(bounds.x+bounds.width+1);
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);
    expect(await page.locator(".tool-row small").first().evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true);
  }
  expect(await page.locator(".settings .transform-options.four").evaluate(el=>getComputedStyle(el).gridTemplateColumns.split(" ").length)).toBe(2);
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/resizable-toolbox-compact.png"});
  await page.setViewportSize({width:1440,height:850});
  await page.reload();await openFixture(page);
  expect(Math.abs((await left.boundingBox())!.width-saved.left)).toBeLessThan(3);
  expect(Math.abs((await right.boundingBox())!.width-saved.right)).toBeLessThan(3);
  await dividers.nth(0).dblclick();
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem("container-toolbox-panel-widths")??"null"))).toEqual({left:null,right:null});
});

test("Panel reset icon asks before restoring saved widths",async({page})=>{
  await mockDesktop(page);
  await expect(page.getByRole("button",{name:"OPEN PROJECT"})).toBeVisible();
  await openFixture(page);
  await expect(page.getByRole("button",{name:"OPEN PROJECT"})).toHaveCount(0);
  await expect(page.getByRole("button",{name:"SAVE PROJECT"})).toBeVisible();
  const divider=page.locator(".workspace-resizer").first();
  await divider.focus();await page.keyboard.press("ArrowRight");
  const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem("container-toolbox-panel-widths")??"null"));
  expect(saved.left).toBeGreaterThan(0);
  const trigger=page.getByRole("button",{name:"Reset panel widths"});
  await trigger.click();
  const dialog=page.getByRole("dialog",{name:"Reset panel layout"});
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("button",{name:"CANCEL"})).toBeFocused();
  await expect(dialog).toContainText("editing history will stay unchanged");
  if(process.env.UI_AUDIT_SCREENSHOTS){
    await page.evaluate(()=>document.documentElement.dataset.theme="dark");
    await page.screenshot({path:"test-results/panel-reset-dark.png"});
    await page.evaluate(()=>document.documentElement.dataset.theme="light");
    await page.screenshot({path:"test-results/panel-reset-light.png"});
  }
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem("container-toolbox-panel-widths")??"null"))).toEqual(saved);
  await trigger.click();
  await dialog.getByRole("button",{name:"CANCEL"}).click();
  await expect(dialog).toHaveCount(0);
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem("container-toolbox-panel-widths")??"null"))).toEqual(saved);
  await trigger.click();
  await dialog.getByRole("button",{name:"RESET"}).click();
  await expect.poll(()=>page.evaluate(()=>JSON.parse(localStorage.getItem("container-toolbox-panel-widths")??"null"))).toEqual({left:null,right:null});
  await expect(dialog).toHaveCount(0);
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="plugin:dialog|message").length)).toBe(0);
});

test("Panel resize and reset dialog keyboard input never reaches either player",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  for(const mode of ["TOOLBOX","SMARTCUT"]){
    await page.getByRole("button",{name:mode,exact:true}).click();
    await page.evaluate(()=>{
      (window as any).__PLAYER_TOUCHES__=0;
      for(const video of document.querySelectorAll("video")){
        Object.defineProperty(video,"currentTime",{configurable:true,get:()=>0,set:()=>{(window as any).__PLAYER_TOUCHES__++}});
        video.play=async()=>{(window as any).__PLAYER_TOUCHES__++};
      }
    });
    await page.locator(".workspace-resizer").first().focus();await page.keyboard.press("ArrowRight");
    expect(await page.evaluate(()=>(window as any).__PLAYER_TOUCHES__)).toBe(0);
    await page.getByRole("button",{name:"Reset panel widths"}).click();
    const dialog=page.getByRole("dialog",{name:"Reset panel layout"});
    await expect(dialog.getByRole("button",{name:"CANCEL"})).toBeFocused();
    await page.keyboard.press("ArrowRight");await page.keyboard.press("Space");
    await expect(dialog).toHaveCount(0);
    expect(await page.evaluate(()=>(window as any).__PLAYER_TOUCHES__)).toBe(0);
  }
});

test("SmartCut and Batch panels resize, persist independently, and reset only the active workspace",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.setViewportSize({width:1440,height:850});
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  const smart=page.locator(".ac-layout"),smartLeft=smart.locator(".ac-left"),smartMiddle=smart.locator(".ac-main"),smartRight=smart.locator(".ac-right");
  const smartDividers=smart.locator(".workspace-resizer");
  await expect(smartDividers).toHaveCount(2);
  const leftBefore=(await smartLeft.boundingBox())!.width,rightBefore=(await smartRight.boundingBox())!.width;
  const leftHandle=(await smartDividers.first().boundingBox())!;
  await page.mouse.move(leftHandle.x+4,leftHandle.y+leftHandle.height/2);await page.mouse.down();
  await page.mouse.move(leftHandle.x+94,leftHandle.y+leftHandle.height/2,{steps:8});await page.mouse.up();
  expect((await smartLeft.boundingBox())!.width-leftBefore).toBeGreaterThan(75);
  const rightHandle=(await smartDividers.last().boundingBox())!;
  await page.mouse.move(rightHandle.x+4,rightHandle.y+rightHandle.height/2);await page.mouse.down();
  await page.mouse.move(rightHandle.x-66,rightHandle.y+rightHandle.height/2,{steps:8});await page.mouse.up();
  expect((await smartRight.boundingBox())!.width-rightBefore).toBeGreaterThan(55);
  const smartSaved=await page.evaluate(()=>JSON.parse(localStorage.getItem("container-smartcut-panel-widths")??"null"));
  expect(smartSaved.left).toBeGreaterThan(leftBefore+75);expect(smartSaved.right).toBeGreaterThan(rightBefore+55);
  await smartDividers.first().focus();await page.keyboard.press("ArrowRight");
  expect((await smartLeft.boundingBox())!.width).toBeGreaterThan(smartSaved.left+15);
  const smartFinal=await page.evaluate(()=>JSON.parse(localStorage.getItem("container-smartcut-panel-widths")??"null"));

  await page.getByRole("button",{name:"BATCH",exact:true}).click();
  const batch=page.locator(".batch-workspace"),batchLeft=batch.locator(".batch-control"),batchRight=batch.locator(".batch-list");
  const batchDivider=batch.locator(".workspace-resizer");
  await expect(batchDivider).toHaveCount(1);
  const batchBefore=(await batchLeft.boundingBox())!.width,batchHandle=(await batchDivider.boundingBox())!;
  await page.mouse.move(batchHandle.x+4,batchHandle.y+batchHandle.height/2);await page.mouse.down();
  await page.mouse.move(batchHandle.x+84,batchHandle.y+batchHandle.height/2,{steps:8});await page.mouse.up();
  expect((await batchLeft.boundingBox())!.width-batchBefore).toBeGreaterThan(65);
  const batchSaved=await page.evaluate(()=>JSON.parse(localStorage.getItem("container-batch-panel-widths")??"null"));
  expect(batchSaved.left).toBeGreaterThan(batchBefore+65);
  await batchDivider.focus();await page.keyboard.press("ArrowLeft");
  expect((await batchLeft.boundingBox())!.width).toBeLessThan(batchSaved.left-15);
  const batchFinal=await page.evaluate(()=>JSON.parse(localStorage.getItem("container-batch-panel-widths")??"null"));

  for(const width of [1024,900,860]){
    await page.setViewportSize({width,height:700});
    await expect.poll(async()=>((await batchRight.boundingBox())?.width??0)).toBeGreaterThan(315);
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);
  }
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/batch-resizable-compact.png"});
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  await expect.poll(async()=>((await smartMiddle.boundingBox())?.width??0)).toBeGreaterThan(310);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/smartcut-resizable-compact.png"});
  await page.setViewportSize({width:1440,height:850});
  expect(Math.abs((await smartLeft.boundingBox())!.width-smartFinal.left)).toBeLessThan(3);
  expect(Math.abs((await smartRight.boundingBox())!.width-smartFinal.right)).toBeLessThan(3);
  await page.reload();await openFixture(page);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  expect(Math.abs((await smartLeft.boundingBox())!.width-smartFinal.left)).toBeLessThan(3);
  expect(Math.abs((await smartRight.boundingBox())!.width-smartFinal.right)).toBeLessThan(3);
  const reset=page.getByRole("button",{name:"Reset panel widths"});
  await reset.click();
  const dialog=page.getByRole("dialog",{name:"Reset panel layout"});
  await expect(dialog).toContainText("SmartCut side panels");
  await dialog.getByRole("button",{name:"CANCEL"}).click();
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem("container-smartcut-panel-widths")??"null"))).toEqual(smartFinal);
  await reset.click();await dialog.getByRole("button",{name:"RESET"}).click();
  await expect.poll(()=>page.evaluate(()=>JSON.parse(localStorage.getItem("container-smartcut-panel-widths")??"null"))).toEqual({left:null,right:null});
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem("container-batch-panel-widths")??"null"))).toEqual(batchFinal);

  await page.getByRole("button",{name:"BATCH",exact:true}).click();
  expect(Math.abs((await batchLeft.boundingBox())!.width-batchFinal.left)).toBeLessThan(3);
  await reset.click();await expect(dialog).toContainText("Batch controls panel");
  await dialog.getByRole("button",{name:"RESET"}).click();
  await expect.poll(()=>page.evaluate(()=>JSON.parse(localStorage.getItem("container-batch-panel-widths")??"null"))).toEqual({left:null});
  await page.reload();await openFixture(page);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  expect((await smartLeft.boundingBox())!.width).toBeLessThan(smartFinal.left-40);
  await page.getByRole("button",{name:"BATCH",exact:true}).click();
  expect((await batchLeft.boundingBox())!.width).toBeLessThan(batchFinal.left-40);
});

test("Image favorites count and crop controls stay separate at compact widths",async({page})=>{
  await page.addInitScript(()=>localStorage.setItem("container-favorites",JSON.stringify(["clipper","transform","text","image_compressor","audio_convert"])));
  await mockDesktop(page,{fixture:{...sample,path:"C:\\fixtures\\sample.jpg",name:"sample.jpg",kind:"image",duration:0.04,width:140,height:140,fps:25,codec:"mjpeg"}});
  await openFixture(page);
  await expect(page.locator(".favorites-filter b")).toHaveText("3");
  await expect(page.locator(".chips")).not.toContainText("dur");
  await expect(page.locator(".chips")).not.toContainText("fps");
  await expect(page.locator(".filename")).not.toHaveAttribute("title",/Duration:|FPS:/);
  for(const width of [1440,1024,900]){
    await page.setViewportSize({width,height:720});
    const lastPreset=page.locator(".crop-options button").last();
    const fit=page.locator(".image-fit-options");
    await fit.scrollIntoViewIfNeeded();
    const presetBox=await lastPreset.boundingBox(),fitBox=await fit.boundingBox();
    expect(fitBox!.y-(presetBox!.y+presetBox!.height)).toBeGreaterThanOrEqual(8);
    await fit.locator("summary").click();
    await expect(fit).toHaveAttribute("open","");
    const options=fit.locator(".transform-options");
    const summaryBox=await fit.locator("summary").boundingBox(),optionsBox=await options.boundingBox();
    expect(optionsBox!.y).toBeGreaterThanOrEqual(summaryBox!.y+summaryBox!.height);
    if(width===900){
      await page.screenshot({path:"test-results/image-toolbox-compact.png"});
      await page.evaluate(()=>document.documentElement.dataset.theme="dark");
      await page.screenshot({path:"test-results/image-toolbox-compact-dark.png"});
    }
    await fit.locator("summary").click();
  }
  await page.locator(".favorites-filter").click();
  await expect(page.locator(".tool-row")).toHaveCount(3);
  await page.locator(".favorite-toggle.active").first().click();
  await expect(page.locator(".favorites-filter b")).toHaveText("2");
});

test("Audio favorites exclude video and image tools",async({page})=>{
  await page.addInitScript(()=>localStorage.setItem("container-favorites",JSON.stringify(["clipper","image_compressor","audio_convert","file_hash"])));
  await mockDesktop(page,{fixture:{...sample,path:"C:\\fixtures\\sample.mp3",name:"sample.mp3",kind:"audio",width:0,height:0,fps:0,codec:"mp3",audio_codec:"mp3"}});
  await openFixture(page);
  await expect(page.locator(".explain")).toContainText("re-encodes audio as FLAC");
  await expect(page.locator(".favorites-filter b")).toHaveText("2");
  await expect(page.locator(".chips")).not.toContainText("res");
  await expect(page.locator(".chips")).not.toContainText("fps");
  await page.locator(".favorites-filter").click();
  await expect(page.locator(".tool-row")).toHaveCount(2);
  await page.setViewportSize({width:900,height:600});
  await expect(page.locator(".run-box .run")).toBeVisible();
  expect(await page.locator(".settings").evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true);
  await page.screenshot({path:"test-results/audio-toolbox-compact.png"});
  await page.evaluate(()=>document.documentElement.dataset.theme="dark");
  await page.screenshot({path:"test-results/audio-toolbox-compact-dark.png"});
});

test("Video and its Audio tab show separate favorite counts",async({page})=>{
  await page.addInitScript(()=>localStorage.setItem("container-favorites",JSON.stringify(["clipper","text","file_hash","remove_audio"])));
  await mockDesktop(page);
  await openFixture(page);
  await expect(page.locator(".favorites-filter b")).toHaveText("3");
  await page.locator(".tabs button").filter({hasText:"AUDIO"}).click();
  await expect(page.locator(".favorites-filter b")).toHaveText("1");
  await page.locator(".favorites-filter").click();
  await expect(page.locator(".tool-row")).toHaveCount(1);
  await expect(page.locator(".tool-row")).toContainText("Remove Audio");
});

for(const language of ["en","tr"]){
  for(const theme of ["dark","light"]){
    test(`UI polish remains readable and reachable: ${language}/${theme}`,async({page})=>{
      await page.addInitScript(language=>localStorage.setItem("container-language",language),language);
      await mockDesktop(page);await openFixture(page);
      await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
      for(const size of [{width:1440,height:900},{width:1280,height:720},{width:1024,height:768},{width:900,height:600}]){
        await page.setViewportSize(size);
        await expect(page.locator(".tool-row small").first()).toBeVisible();
        const categoryColors=await page.locator(".tool-group").evaluateAll(groups=>groups.map(group=>({
          category:group.getAttribute("data-category"),
          colors:Array.from(group.querySelectorAll(".tool-row>i")).map(el=>getComputedStyle(el).backgroundColor)
        })));
        for(const group of categoryColors)expect(new Set(group.colors).size).toBe(1);
        expect(new Set(categoryColors.map(group=>group.colors[0])).size).toBeGreaterThan(4);
        await expect(page.locator(".tool-row small").first()).toHaveText(language==="tr"?"Kırp, döndür ve boyutlandır":"Crop, rotate and resize");
        expect(await page.locator(".tool-row small").first().evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true);
        await expect(page.locator(".tool-row").first()).toHaveAttribute("title",/.+/);
        expect(await page.locator(".transform-controls>section").first().evaluate(el=>getComputedStyle(el).backgroundColor)).toBe("rgba(0, 0, 0, 0)");
        expect(await page.locator(".transform-options button.active").first().evaluate(el=>getComputedStyle(el).boxShadow)).toBe("none");
        if(theme==="light")expect(await page.locator(".transform-options button.active").first().evaluate(el=>getComputedStyle(el).color)).toBe("rgb(37, 99, 184)");
        const geometry=await page.evaluate(()=>{
          const field=document.querySelector(".settings>.field-list")!;
          field.scrollTop=field.scrollHeight;
          const box=(s:string)=>document.querySelector(s)!.getBoundingClientRect();
          return {fields:box(".settings>.field-list").bottom,footer:box(".run-box").top,run:box(".run-box .run").bottom,panel:box(".settings").bottom,last:field.lastElementChild!.getBoundingClientRect().bottom};
        });
        expect(geometry.fields).toBeLessThanOrEqual(geometry.footer+1);
        expect(geometry.run).toBeLessThanOrEqual(geometry.panel);
        expect(geometry.last).toBeLessThanOrEqual(geometry.footer+1);
        await page.locator(".field-list").evaluate(el=>el.scrollTop=0);
        await page.screenshot({path:`test-results/polish-toolbox-${language}-${theme}-${size.width}.png`});
        await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
        const timeline=await page.evaluate(()=>{
          const box=(s:string)=>document.querySelector(s)!.getBoundingClientRect();
          return {title:box(".tl-title"),status:box(".tl-status"),buttons:box(".tl-buttons"),wave:box(".wave"),nav:box(".navigator"),panel:box(".ac-timeline")};
        });
        expect(timeline.buttons.top).toBeGreaterThanOrEqual(Math.max(timeline.title.bottom,timeline.status.bottom)-1);
        expect(timeline.wave.top).toBeGreaterThanOrEqual(timeline.buttons.bottom);
        expect(timeline.nav.bottom).toBeLessThanOrEqual(timeline.panel.bottom);
        expect(timeline.buttons.right).toBeLessThanOrEqual(timeline.panel.right);
        expect(timeline.status.right).toBeLessThanOrEqual(timeline.panel.right);
        await page.screenshot({path:`test-results/polish-smartcut-${language}-${theme}-${size.width}.png`});
        await page.getByRole("button",{name:language==="tr"?"ARAÇ KUTUSU":"TOOLBOX",exact:true}).click();
      }
    });
  }
}

test("header media summary aligns beside Toolbox without crowding actions",async({page})=>{
  await mockDesktop(page);
  await page.evaluate(media=>{
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="probe_media"?{...media,path:args.path,name:"batuhanfurkan5-bizde-bizde-hadiiii-clip-with-a-very-long-filename.mp4"}:undefined;
  },sample);
  await openFixture(page);
  for(const width of [1920,1440,1024,900]){
    await page.setViewportSize({width,height:1080});
    const summary=await page.locator(".file-summary").boundingBox();
    const tabs=await page.locator(".mode-tabs").boundingBox();
    const close=await page.locator(".top-cancel").boundingBox();
    expect(tabs!.x-summary!.x-summary!.width).toBeGreaterThanOrEqual(7);
    expect(tabs!.x-summary!.x-summary!.width).toBeLessThanOrEqual(15);
    expect(close!.x+close!.width).toBeLessThanOrEqual(width);
    await expect(page.locator(".filename")).toBeVisible();
    await expect(page.locator(".filename")).toHaveAttribute("title",/Duration:.*\nResolution:.*\nFPS:.*\nCodec:.*\nSize:/);
    if(summary!.width<=430)await expect(page.locator(".chips")).toBeHidden();
    else{
      await expect(page.locator(".chips")).toBeVisible();
      if(summary!.width<=800)await expect(page.locator(".media-extra").first()).toBeHidden();
      else await expect(page.locator(".media-extra").first()).toBeVisible();
      expect(await page.locator(".chips").evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true);
    }
    await page.screenshot({path:`test-results/header-responsive-${width}.png`});
    if(width===1920){
      const chips=await page.locator(".chips").boundingBox();
      expect(Math.abs(chips!.x+chips!.width-summary!.x-summary!.width)).toBeLessThan(2);
      await page.screenshot({path:"test-results/header-right-aligned.png"});
    }
  }
});

async function stageMocks(page:Page){
  page.on("pageerror",error=>console.error("Stage page error:",error.message));
  await page.evaluate(()=>{
    let count=0;
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
      if(cmd==="run_operation"||cmd==="export_autocut")return {output:`C:\\fixtures\\output-${++count}.mp4`,elapsed:.1};
      if(cmd==="write_project"){(window as any).__SAVED_PROJECT__=args.contents;return null}
      if(cmd==="read_project")return (window as any).__SAVED_PROJECT__;
    };
  });
}
async function selectStageNumber(page:Page,id:number){
  await page.locator(".stage-trigger").click();
  await page.locator(".stage-list button").filter({has:page.locator("b",{hasText:new RegExp(`^${id}$`)})}).click();
  await expect(page.locator(".stage-trigger")).toHaveText("GO BACK ▾");
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage",String(id));
}

async function historyFixture(page:Page,count=3){
  await mockDesktop(page,{interrupted:true});await openFixture(page);await stageMocks(page);
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).__SAVED_PROJECT__)).toBeTruthy();
  await page.evaluate(count=>{
    const original=JSON.parse((window as any).__SAVED_PROJECT__);
    delete original.stageHistory;
    const entries=Array.from({length:count},(_,index)=>{
      const session=structuredClone(original);
      session.mediaPath=`C:\\fixtures\\çekim-çok-uzun-dosya-adı-${index+1}-final-video.mp4`;
      session.toolbox.media.path=session.mediaPath;
      session.toolbox.media.name=session.mediaPath.split("\\").pop();
      return {id:index+1,parent:index===0?null:index,label:index%2?"SmartCut":"Transform",session};
    });
    (window as any).__SAVED_PROJECT__=JSON.stringify({...entries.at(-1)!.session,stageHistory:{entries,current:count,next:count+1,trimmed:false}});
    (window as any).__TEST_DROP__("C:\\fixtures\\history.containerproject");
  },count);
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage",String(count));
}

test("history keyboard navigation, focus return and dismissal do not activate editor shortcuts",async({page})=>{
  const errors:string[]=[];page.on("pageerror",error=>errors.push(error.message));
  await historyFixture(page);
  const trigger=page.locator(".stage-trigger"),rows=page.locator(".stage-list button");
  await trigger.focus();await page.keyboard.press("Enter");
  await expect(rows.nth(2)).toBeFocused();
  await page.keyboard.press("ArrowUp");await expect(rows.nth(1)).toBeFocused();
  await page.keyboard.press("Home");await expect(rows.first()).toBeFocused();
  await page.keyboard.press("ArrowUp");await expect(rows.first()).toBeFocused();
  await page.keyboard.press("End");await expect(rows.last()).toBeFocused();
  await page.keyboard.press("ArrowDown");await expect(rows.last()).toBeFocused();
  const probes=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((c:any)=>c.cmd==="probe_media").length);
  await page.keyboard.press("Space");
  await expect(rows.last()).toBeFocused();
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((c:any)=>c.cmd==="probe_media").length)).toBe(probes);
  await page.keyboard.press("Home");await page.keyboard.press("Enter");
  await expect(trigger).toHaveAttribute("data-current-stage","1");await expect(trigger).toBeFocused();
  await trigger.press("ArrowDown");await expect(rows.first()).toBeFocused();
  await page.keyboard.press("Escape");await expect(trigger).toBeFocused();await expect(rows).toHaveCount(0);
  await trigger.click();await page.keyboard.press("End");await page.keyboard.press("Tab");
  await expect(page.locator(".stage-menu")).toHaveCount(0);
  await trigger.click();await page.locator(".brand").click();await expect(rows).toHaveCount(0);
  await trigger.click();await page.setViewportSize({width:1000,height:700});await expect(rows).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("long history scrolls to current step and stays readable within dark/light compact windows",async({page})=>{
  await historyFixture(page,30);
  for(const theme of ["dark","light"]){
    await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
    for(const size of [{width:1920,height:1080},{width:1024,height:768},{width:900,height:600}]){
      await page.setViewportSize(size);await page.locator(".stage-trigger").click();
      const menu=page.locator(".stage-menu"),list=page.locator(".stage-list"),current=page.locator('[aria-current="step"]');
      await expect(current).toBeFocused();await expect(page.locator(".stage-list button")).toHaveCount(30);
      const box=(await menu.boundingBox())!,row=(await current.boundingBox())!,scroll=(await list.boundingBox())!;
      expect(box.x).toBeGreaterThanOrEqual(0);expect(box.x+box.width).toBeLessThanOrEqual(size.width);
      expect(box.y+box.height).toBeLessThanOrEqual(size.height);
      expect(row.y).toBeGreaterThanOrEqual(scroll.y);expect(row.y+row.height).toBeLessThanOrEqual(scroll.y+scroll.height+1);
      expect(await menu.evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true);
      await expect(current.locator("small")).toContainText("çok-uzun-dosya");
      await expect(current).toHaveAttribute("title",/çekim-çok-uzun-dosya-adı-30/);
      await page.screenshot({path:`test-results/history-long-${theme}-${size.width}.png`});
      await page.keyboard.press("Escape");
    }
  }
});

test("history restoration locks navigation until source loading completes",async({page})=>{
  await historyFixture(page);
  await page.evaluate(()=>{
    const previous=(window as any).__TEST_HANDLER__;
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="probe_media"?new Promise(resolve=>{(window as any).__RESOLVE_SOURCE__=()=>resolve({...sampleForTest(),path:args.path})}):previous(cmd,args);
    function sampleForTest(){return JSON.parse((window as any).__SAVED_PROJECT__).toolbox.media}
  });
  await page.locator(".stage-trigger").click();await page.locator(".stage-list button").first().click();
  await expect(page.locator(".stage-menu")).toHaveCount(0);await expect(page.locator(".stage-trigger")).toBeDisabled();
  await expect(page.locator("main")).toHaveAttribute("inert","");
  await expect.poll(()=>page.evaluate(()=>typeof (window as any).__RESOLVE_SOURCE__)).toBe("function");
  await page.evaluate(()=>(window as any).__RESOLVE_SOURCE__());
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","1");
  await expect(page.locator(".stage-trigger")).toBeEnabled();await expect(page.locator(".stage-trigger")).toBeFocused();
});

test("latest draft and original branch survive saving, reopening and recovery",async({page})=>{
  await historyFixture(page);
  const rotation=page.locator("header").filter({has:page.locator("b",{hasText:/^ROTATE$/})}).locator("small");
  await selectStageNumber(page,1);await page.getByRole("button",{name:"180°",exact:true}).click();
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>JSON.parse((window as any).__SAVED_PROJECT__).toolbox.selected.fields.find((f:any)=>f.key==="rotate").value)).toBe("180");
  await page.evaluate(()=>(window as any).__TEST_DROP__("C:\\fixtures\\history.containerproject"));
  await expect(rotation).toHaveText("180°");
  await selectStageNumber(page,2);
  await page.locator(".stage-trigger").click();
  await expect(page.locator(".stage-list button")).toHaveCount(4);
  await expect(page.locator(".stage-branch")).toHaveCount(2);
  await page.keyboard.press("Escape");await selectStageNumber(page,4);
  await expect(rotation).toHaveText("180°");
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>JSON.parse((window as any).__SAVED_PROJECT__).stageHistory.current)).toBe(4);
  // Recovery is a separate entry route from explicit project loading.
  await page.reload();await expect(page.getByRole("button",{name:"RESTORE WORK",exact:true})).toBeVisible();
  await page.getByRole("button",{name:"RESTORE WORK",exact:true}).click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","4");
  await expect(rotation).toHaveText("180°");
  await selectStageNumber(page,1);await expect(page.getByRole("button",{name:"0°",exact:true})).toHaveClass(/active/);
});

test("relinking a historical source updates its saved references without losing other steps",async({page})=>{
  await historyFixture(page);
  await page.evaluate(()=>{
    const previous=(window as any).__TEST_HANDLER__;
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
      if(cmd==="project_media_available")return !args.path.includes("adı-1-");
      if(cmd==="plugin:dialog|message")return args.buttons.OkCancelCustom[0];
      if(cmd==="plugin:dialog|open")return "C:\\fixtures\\relocated.mp4";
      return previous(cmd,args);
    };
  });
  await selectStageNumber(page,1);
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>JSON.parse((window as any).__SAVED_PROJECT__).mediaPath)).toBe("C:\\fixtures\\relocated.mp4");
  const saved=await page.evaluate(()=>JSON.parse((window as any).__SAVED_PROJECT__));
  expect(saved.stageHistory.entries).toHaveLength(3);
  expect(saved.stageHistory.entries[0].session.mediaPath).toBe("C:\\fixtures\\relocated.mp4");
  expect(saved.stageHistory.entries[0].session.toolbox.media.path).toBe("C:\\fixtures\\relocated.mp4");
  await selectStageNumber(page,2);await selectStageNumber(page,1);
  const confirmations=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((c:any)=>c.cmd==="plugin:dialog|message").length);
  expect(confirmations).toBe(1);
});

test("history shows the live tool name and Turkish labels without creating spurious steps",async({page})=>{
  await historyFixture(page);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  await page.locator(".stage-trigger").click();
  await expect(page.locator('[aria-current="step"] .stage-label')).toHaveText("SmartCut");
  await page.keyboard.press("Escape");
  await page.evaluate(()=>localStorage.setItem("container-language","tr"));await page.reload();
  await page.getByRole("button",{name:"ÇALIŞMAYI GERİ YÜKLE",exact:true}).click();
  await page.locator(".stage-trigger").click();
  await expect(page.locator(".stage-trigger")).toHaveText("GERİ DÖN ▾");
  await expect(page.locator(".stage-heading strong")).toHaveText("İşlem geçmişi");
  await expect(page.locator(".stage-current")).toHaveText("Şu an");
  await expect(page.locator(".stage-list button")).toHaveCount(3);
  await page.screenshot({path:"test-results/history-turkish.png"});
});

test("invalid historical graphs fall back safely to the current project",async({page})=>{
  await historyFixture(page);
  await page.evaluate(()=>{
    const saved=JSON.parse((window as any).__SAVED_PROJECT__);
    saved.stageHistory.entries[0].parent=999;
    (window as any).__SAVED_PROJECT__=JSON.stringify(saved);
    (window as any).__TEST_DROP__("C:\\fixtures\\invalid-history.containerproject");
  });
  await expect(page.locator(".stage-trigger")).toHaveCount(0);
  await expect(page.locator(".settings")).toBeVisible();
  await expect(page.getByRole("button",{name:"▶ render transform",exact:true})).toBeEnabled();
});

test("changed render settings require rerender before continuation, including restored projects",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  const render=page.getByRole("button",{name:"▶ render transform",exact:true});
  const proceed=page.getByRole("button",{name:"continue editing",exact:true});
  await render.click();await expect(proceed).toBeEnabled();
  await page.getByRole("button",{name:"180°",exact:true}).click();
  await expect(proceed).toBeDisabled();
  await expect(proceed).toHaveAttribute("title",/render again/);
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).__SAVED_PROJECT__)).toBeTruthy();
  await page.evaluate(()=>(window as any).__TEST_DROP__("C:\\fixtures\\saved.containerproject"));
  await expect(proceed).toBeDisabled();
  await page.getByRole("button",{name:"0°",exact:true}).click();
  await expect(proceed).toBeEnabled();
  await page.getByRole("button",{name:"180°",exact:true}).click();
  await render.click();await expect(proceed).toBeEnabled();await proceed.click();
  await selectStageNumber(page,1);
  await expect(page.locator("header").filter({has:page.locator("b",{hasText:/^ROTATE$/})}).locator("small")).toHaveText("180°");
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((c:any)=>c.cmd==="run_operation").at(-1).args.request.params.rotate)).toBe("180");
});

test("GO BACK closes with Escape from both trigger and menu",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await page.getByRole("button",{name:"continue editing",exact:true}).click();
  const trigger=page.locator(".stage-trigger");
  await trigger.click();await trigger.press("Escape");
  await expect(page.locator(".stage-menu")).toHaveCount(0);await expect(trigger).toBeFocused();
  await trigger.click();await page.locator(".stage-close").focus();await page.keyboard.press("Escape");
  await expect(page.locator(".stage-menu")).toHaveCount(0);await expect(trigger).toBeFocused();
});

for(const outcome of ["Render failed","cancelled"]){
  test(`Batch ${outcome} rerun cannot continue its previous output`,async({page})=>{
    await mockDesktop(page);await openFixture(page);await stageMocks(page);
    await page.getByRole("button",{name:"BATCH",exact:true}).click();
    const start=page.getByRole("button",{name:"▶ START QUEUE",exact:true});
    await start.click();await expect(page.getByRole("button",{name:"continue editing",exact:true})).toBeEnabled();
    for(const width of [1440,1024]){
      await page.setViewportSize({width,height:900});
      const buttons=page.locator(".batch-row-actions button");
      const a=await buttons.nth(0).boundingBox(),b=await buttons.nth(1).boundingBox();
      expect(Math.abs(a!.y-b!.y)).toBeLessThan(1);
      expect(a!.x+a!.width).toBeLessThanOrEqual(b!.x);
    }
    await page.screenshot({path:`test-results/batch-actions-${outcome.replaceAll(" ","-")}.png`});
    await page.evaluate(message=>{(window as any).__TEST_HANDLER__=(cmd:string)=>{if(cmd==="run_operation")throw new Error(message)}},outcome);
    await start.click();
    await expect(page.locator(".batch-items article strong")).toHaveText(outcome==="cancelled"?"cancelled":"failed");
    await expect(page.getByRole("button",{name:"continue editing",exact:true})).toHaveCount(0);
    await expect(page.locator(".batch-items article small")).not.toContainText("output-1.mp4");
  });
}
test("Continue editing alone creates stages; earlier settings and forward outputs survive",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await expect(page.locator(".stage-trigger")).toHaveCount(0);
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await expect(page.getByRole("button",{name:"continue editing",exact:true})).toBeEnabled();
  await expect(page.locator(".stage-trigger")).toHaveCount(0);
  await page.getByRole("button",{name:"continue editing",exact:true}).click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","2");
  await expect(page.locator(".topbar .stage-trigger")).toHaveCount(1);
  await expect(page.locator(".topbar .slash")).toHaveCount(0);
  const brandBounds=await page.locator(".brand").boundingBox();
  const backBounds=await page.locator(".stage-trigger").boundingBox();
  expect(backBounds!.x-brandBounds!.x-brandBounds!.width).toBeLessThanOrEqual(15);
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await selectStageNumber(page,1);
  await page.getByRole("button",{name:"continue editing",exact:true}).click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","3");
  await page.locator(".stage-trigger").click();
  await expect(page.locator(".stage-list button")).toHaveCount(3);
  await page.locator(".stage-close").click();
  await selectStageNumber(page,2);
  await page.getByRole("button",{name:"show output",exact:true}).click();
  const reveal=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((x:any)=>x.cmd.includes("reveal")).at(-1));
  expect(JSON.stringify(reveal)).toContain("output-2.mp4");
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).__SAVED_PROJECT__)).toBeTruthy();
  const saved=await page.evaluate(()=>JSON.parse((window as any).__SAVED_PROJECT__));
  expect(saved.stageHistory.entries).toHaveLength(3);
  // Open the saved project via the project-aware drop route.
  await page.evaluate(()=>(window as any).__TEST_DROP__("C:\\fixtures\\sample.containerproject"));
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","2");
  await selectStageNumber(page,1);
  await expect(page.getByRole("button",{name:"continue editing",exact:true})).toBeVisible();
});

test("editing an ancestor creates a branch without erasing the original",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await page.getByRole("button",{name:"continue editing",exact:true}).click();
  await selectStageNumber(page,1);
  await page.getByRole("button",{name:"180°",exact:true}).click();
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await page.getByRole("button",{name:"continue editing",exact:true}).click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","4");
  await selectStageNumber(page,1);
  await expect(page.getByRole("button",{name:"0°",exact:true})).toHaveClass(/active/);
  await selectStageNumber(page,3);
  await expect(page.locator("header").filter({has:page.locator("b",{hasText:/^ROTATE$/})}).locator("small")).toHaveText("180°");
});

test("missing historical source and failed continuation leave current stage intact",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await page.getByRole("button",{name:"continue editing",exact:true}).click();
  await page.evaluate(()=>{
    const original=(window as any).__TEST_HANDLER__;
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="project_media_available"?false:original(cmd,args);
  });
  await page.locator(".stage-trigger").click();
  await page.locator(".stage-list button").first().click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","2");
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="run_operation"?{output:"C:\\fixtures\\broken.mp4",elapsed:.1}:undefined});
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await page.getByRole("button",{name:"continue editing",exact:true}).click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","2");
  await expect(page.getByText("Invalid media fixture").first()).toBeVisible();
});

test("SmartCut stages restore cuts and keep history compact in dark and light",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  await page.getByRole("button",{name:"DETECT SILENCE",exact:true}).click();
  await page.getByRole("button",{name:"EXPORT MP4",exact:true}).click();
  await page.getByRole("button",{name:"continue editing",exact:true}).click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","2");
  await selectStageNumber(page,1);
  await expect(page.locator(".ac-layout")).toBeVisible();
  await expect(page.getByRole("button",{name:"continue editing",exact:true})).toBeVisible();
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).__SAVED_PROJECT__)).toBeTruthy();
  expect(await page.evaluate(()=>JSON.parse((window as any).__SAVED_PROJECT__).autocut.cuts)).toHaveLength(2);
  await page.setViewportSize({width:1024,height:768});
  for(const theme of ["dark","light"]){
    await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
    await page.locator(".stage-trigger").click();
    const bounds=await page.locator(".stage-menu").boundingBox();
    expect(bounds!.x).toBeGreaterThanOrEqual(0);expect(bounds!.x+bounds!.width).toBeLessThanOrEqual(1024);
    expect(bounds!.y+bounds!.height).toBeLessThanOrEqual(768);
    await page.screenshot({path:`test-results/stage-history-${theme}.png`});
    await page.locator(".stage-close").click();
  }
});

test("Batch continuation restores the completed queue without rerunning it",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByRole("button",{name:"BATCH",exact:true}).click();
  await page.getByRole("button",{name:"▶ START QUEUE",exact:true}).click();
  await page.getByRole("button",{name:"continue editing",exact:true}).click();
  await selectStageNumber(page,1);
  await expect(page.locator(".batch-items article strong")).toHaveText("complete");
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").length)).toBe(1);
});

test("color output does not silently replace the source before Continue editing",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByPlaceholder("search tools...").fill("Color Adjustment");
  await page.getByText("Color Adjustment",{exact:true}).last().click();
  await page.getByRole("checkbox",{name:/^Brightness/}).check();
  await page.getByRole("button",{name:/render color adjustment/i}).click();
  await expect(page.getByRole("button",{name:"continue editing",exact:true})).toBeEnabled();
  const inputs=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="probe_media").map((call:any)=>call.args.path));
  expect(inputs).toEqual([sample.path]);
  await page.getByRole("button",{name:"continue editing",exact:true}).click();
  await selectStageNumber(page,1);
  await expect(page.locator(".selected-title h2")).toHaveText("Color Adjustment");
  await expect(page.getByRole("checkbox",{name:/^Brightness/})).toBeChecked();
});

test("text layers remain editable when returning from a continued output",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  const font=readFileSync(new URL("../../node_modules/@fontsource-variable/geist/files/geist-latin-wght-normal.woff2",import.meta.url)).toString("base64");
  await page.evaluate(font=>{
    const previous=(window as any).__TEST_HANDLER__;
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
      if(cmd==="list_system_fonts")return [{name:"Arial",path:"C:\\fixtures\\font.ttf"}];
      if(cmd==="font_preview_data")return `data:font/woff2;base64,${font}`;
      return previous(cmd,args);
    };
  },font);
  await page.getByPlaceholder("search tools...").fill("Text");
  await page.locator(".tool-row").filter({has:page.getByText("Text",{exact:true})}).click();
  await page.getByRole("button",{name:"＋ Add text",exact:true}).click();
  await page.getByRole("textbox",{name:"Text",exact:true}).fill("Keep this editable");
  await page.getByRole("button",{name:"▶ render text",exact:true}).click();
  await expect(page.getByRole("textbox",{name:"Text",exact:true})).toHaveValue("Keep this editable");
  await page.getByRole("button",{name:"continue editing",exact:true}).click();
  await selectStageNumber(page,1);
  await expect(page.getByRole("textbox",{name:"Text",exact:true})).toHaveValue("Keep this editable");
});

test("Text handles resize smoothly and pasted emoji reaches the export raster",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  const font=readFileSync(new URL("../../node_modules/@fontsource-variable/geist/files/geist-latin-wght-normal.woff2",import.meta.url)).toString("base64");
  await page.evaluate(font=>{
    const previous=(window as any).__TEST_HANDLER__;
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
      if(cmd==="list_system_fonts")return [{name:"Arial",path:"C:\\fixtures\\font.ttf"}];
      if(cmd==="font_preview_data")return `data:font/woff2;base64,${font}`;
      return previous(cmd,args);
    };
  },font);
  await page.getByPlaceholder("search tools...").fill("Text");
  await page.locator(".tool-row").filter({has:page.getByText("Text",{exact:true})}).click();
  await page.getByRole("button",{name:"＋ Add text",exact:true}).click();
  await page.getByRole("textbox",{name:"Text",exact:true}).fill("Hi 😀");
  const layer=page.locator(".preview-text.active");
  const before=await layer.boundingBox();
  const right=await layer.locator(".text-size-handle.right").boundingBox();
  await page.mouse.move(right!.x+right!.width/2,right!.y+right!.height/2);
  await page.mouse.down();await page.mouse.move(right!.x+right!.width/2+36,right!.y+right!.height/2,{steps:4});await page.mouse.up();
  const after=await layer.boundingBox();
  expect(after!.width-before!.width).toBeGreaterThan(25);
  expect(Math.abs(after!.x-before!.x)).toBeLessThan(3);
  const left=await layer.locator(".text-size-handle.left").boundingBox();
  await page.mouse.move(left!.x+left!.width/2,left!.y+left!.height/2);
  await page.mouse.down();await page.mouse.move(left!.x+left!.width/2-24,left!.y+left!.height/2,{steps:4});await page.mouse.up();
  const fromLeft=await layer.boundingBox();
  expect(fromLeft!.width-after!.width).toBeGreaterThan(16);
  expect(Math.abs(fromLeft!.x+fromLeft!.width-after!.x-after!.width)).toBeLessThan(3);
  await expect(layer.locator(".text-size-handle.nw,.text-size-handle.ne,.text-size-handle.sw,.text-size-handle.se")).toHaveCount(4);
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/text-transform-handles.png"});
  const emojiPixels=await page.locator(".text-preview-canvas").evaluate(canvas=>{
    const image=(canvas as HTMLCanvasElement).getContext("2d")!.getImageData(0,0,(canvas as HTMLCanvasElement).width,(canvas as HTMLCanvasElement).height).data;
    let yellow=0;for(let index=0;index<image.length;index+=4)if(image[index]>170&&image[index+1]>90&&image[index+1]<245&&image[index+2]<100&&image[index+3]>100)yellow++;
    return yellow;
  });
  expect(emojiPixels).toBeGreaterThan(20);
  await page.getByRole("button",{name:"▶ render text",exact:true}).click();
  const call=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((value:any)=>value.cmd==="run_operation").at(-1));
  expect(call.args.request.params.text_raster_png).toMatch(/^data:image\/png;base64,/);
});

for(const kind of ["video","image"] as const){
  test(`Text corner resize keeps line breaks, Enter adds a line and canvas X removes it in ${kind}`,async({page})=>{
    const fixture=kind==="image"?{...sample,path:"C:\\fixtures\\sample.jpg",name:"sample.jpg",kind:"image",duration:.04,width:640,height:360,fps:25,codec:"mjpeg"}:sample;
    await mockDesktop(page,{fixture});await openFixture(page);await stageMocks(page);
    const font=readFileSync(new URL("../../node_modules/@fontsource-variable/geist/files/geist-latin-wght-normal.woff2",import.meta.url)).toString("base64");
    await page.evaluate(font=>{
      const previous=(window as any).__TEST_HANDLER__;
      (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
        if(cmd==="list_system_fonts")return [{name:"Arial",path:"C:\\fixtures\\font.ttf"}];
        if(cmd==="font_preview_data")return `data:font/woff2;base64,${font}`;
        return previous(cmd,args);
      };
    },font);
    await page.getByPlaceholder("search tools...").fill("Text");
    await page.locator(".tool-row").filter({has:page.getByText("Text",{exact:true})}).click();
    await page.getByRole("button",{name:"＋ Add text",exact:true}).click();
    const editor=page.getByRole("textbox",{name:"Text",exact:true});
    await editor.fill("ONE TWO THREE FOUR FIVE SIX SEVEN EIGHT NINE TEN ELEVEN TWELVE");
    const layer=page.locator(".preview-text.active");
    const before=await layer.locator("span").textContent()??"";
    expect(before.split("\n").length).toBeGreaterThan(1);
    const corner=await layer.locator(".text-size-handle.ne").boundingBox();
    await page.mouse.move(corner!.x+corner!.width/2,corner!.y+corner!.height/2);
    await page.mouse.down();await page.mouse.move(corner!.x+corner!.width/2-30,corner!.y+corner!.height/2,{steps:4});await page.mouse.up();
    await expect(layer.locator("span")).toHaveText(before);
    await editor.fill("FIRST");await editor.press("End");await editor.press("Enter");await editor.type("SECOND");
    await expect(editor).toHaveValue("FIRST\nSECOND");
    await expect(layer.locator("span")).toHaveText("FIRST\nSECOND");
    if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:`test-results/text-multiline-delete-${kind}.png`});
    await page.getByRole("button",{name:"▶ render text",exact:true}).click();
    const call=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((value:any)=>value.cmd==="run_operation").at(-1));
    expect(JSON.parse(call.args.request.params.layers)[0].text).toBe("FIRST\nSECOND");
    await expect(layer.getByRole("button",{name:/Remove text/})).toHaveCount(0);
    await page.locator(".text-tab.active .text-tab-remove").click();
    await expect(page.locator(".preview-text")).toHaveCount(0);
  });
}

for(const kind of ["video","image"] as const){
  test(`Image overlay handle keeps its opposite edge fixed in ${kind}`,async({page})=>{
    const fixture=kind==="image"?{...sample,path:"C:\\fixtures\\sample.jpg",name:"sample.jpg",kind:"image",duration:.04,width:640,height:360,fps:25,codec:"mjpeg"}:sample;
    await mockDesktop(page,{fixture});await openFixture(page);
    const logo=readFileSync(new URL("../../public/logo-dark.png",import.meta.url));
    await page.route("http://asset.localhost/**",route=>decodeURIComponent(route.request().url()).endsWith("overlay.png")?route.fulfill({contentType:"image/png",body:logo}):route.continue());
    await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="plugin:dialog|open"?"C:\\fixtures\\overlay.png":undefined});
    await page.getByPlaceholder("search tools...").fill("Image / Logo Overlay");
    await page.getByText("Image / Logo Overlay",{exact:true}).last().click();
    await page.locator(".file-field").click();
    await expect(page.locator(".overlay-preview-box")).toBeVisible();
    const overlay=page.locator(".overlay-preview-box");
    const before=await overlay.boundingBox();
    const handle=await overlay.locator(".text-size-handle.right").boundingBox();
    await page.mouse.move(handle!.x+handle!.width/2,handle!.y+handle!.height/2);
    await page.mouse.down();await page.mouse.move(handle!.x+handle!.width/2+30,handle!.y+handle!.height/2,{steps:4});await page.mouse.up();
    const after=await overlay.boundingBox();
    expect(after!.width-before!.width).toBeGreaterThan(20);
    expect(Math.abs(after!.x-before!.x)).toBeLessThan(3);
    const corner=await overlay.locator(".text-size-handle.se").boundingBox();
    await page.mouse.move(corner!.x+corner!.width/2,corner!.y+corner!.height/2);
    await page.mouse.down();await page.mouse.move(corner!.x+corner!.width/2+16,corner!.y+corner!.height/2+12,{steps:4});await page.mouse.up();
    const diagonal=await overlay.boundingBox();
    expect(diagonal!.width-after!.width).toBeGreaterThan(12);
    expect(Math.abs(diagonal!.x-after!.x)).toBeLessThan(3);
    if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:`test-results/overlay-transform-handles-${kind}.png`});
  });
}

test("failed second file does not destroy the open tool settings", async ({ page }) => {
  await mockDesktop(page, { invalidSecond: true });
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("GIF Maker");
  await page.getByText("GIF Maker", { exact: true }).last().click();
  await expect(page.locator(".selected-title h2")).toHaveText("GIF Maker");
  await page.evaluate(() => (window as any).__TEST_DROP__("C:\\fixtures\\broken.mp4"));
  await expect(page.getByText("Invalid media fixture").first()).toBeVisible();
  await expect(page.locator(".selected-title h2")).toHaveText("GIF Maker");
});

test("text undo stays in the Social Tag input", async ({ page }) => {
  await mockDesktop(page);
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper", { exact: true }).last().click();
  await page.getByText("Social Tag", { exact: true }).click();
  const username = page.getByPlaceholder("kanaladi");
  await username.fill("creator");
  await username.press("ControlOrMeta+z");
  await expect(username).toHaveValue("");
});

test("Social Tag positions move across the camera and retain per-style choices", async ({ page }) => {
  await mockDesktop(page);
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper", { exact: true }).last().click();
  await page.getByText("Social Tag", { exact: true }).click();
  await page.getByPlaceholder("kanaladi").fill("Example");
  const style = page.getByRole("combobox", { name: "Style" });
  const position = page.getByRole("combobox", { name: "Position" });
  const tag = page.locator(".clipper-social-tag");
  const x = async () => (await tag.boundingBox())?.x ?? -1;

  await expect(position).toHaveValue("left");
  await expect(tag).toBeVisible();
  const boxedLeft = await x();
  const camera = await page.locator(".region-a").boundingBox();
  const boxedBounds = await tag.boundingBox();
  expect(camera).not.toBeNull();
  expect(boxedBounds).not.toBeNull();
  expect(Math.abs(boxedBounds!.x - camera!.x)).toBeLessThan(3);
  expect(Math.abs(boxedBounds!.y + boxedBounds!.height - camera!.y - camera!.height)).toBeLessThan(3);
  await position.selectOption("center");
  const boxedCenter = await x();
  const centeredBounds = await tag.boundingBox();
  expect(Math.abs(centeredBounds!.x + centeredBounds!.width / 2 - camera!.x - camera!.width / 2)).toBeLessThan(3);
  await position.selectOption("right");
  const boxedRight = await x();
  const rightBounds = await tag.boundingBox();
  expect(Math.abs(rightBounds!.x + rightBounds!.width - camera!.x - camera!.width)).toBeLessThan(3);
  expect(boxedLeft).toBeLessThan(boxedCenter);
  expect(boxedCenter).toBeLessThan(boxedRight);
  if (process.env.UI_AUDIT_SCREENSHOTS) await page.screenshot({ path: "test-results/social-tag-boxed-right.png" });

  await style.selectOption("plain");
  await expect(position).toHaveValue("center");
  const plainCenter = await x();
  const plainBounds = await tag.boundingBox();
  expect(Math.abs(plainBounds!.x + plainBounds!.width / 2 - camera!.x - camera!.width / 2)).toBeLessThan(3);
  expect(Math.abs(plainBounds!.y + plainBounds!.height / 2 - camera!.y - camera!.height)).toBeLessThan(3);
  await position.selectOption("left");
  const plainLeft = await x();
  await position.selectOption("right");
  const plainRight = await x();
  expect(plainLeft).toBeLessThan(plainCenter);
  expect(plainCenter).toBeLessThan(plainRight);
  if (process.env.UI_AUDIT_SCREENSHOTS) await page.screenshot({ path: "test-results/social-tag-plain-right.png" });
  await style.selectOption("boxed");
  await expect(position).toHaveValue("right");
});

test("Social Tag preview keeps text and icon aligned at small sizes",async({page})=>{
  await mockDesktop(page);
  await openFixture(page);
  await page.setViewportSize({width:1920,height:1080});
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper",{exact:true}).last().click();
  await page.getByText("Social Tag",{exact:true}).click();
  await page.getByPlaceholder("kanaladi").fill("eray");
  const style=page.getByRole("combobox",{name:"Style"});
  const platform=page.getByRole("combobox",{name:"Platform"});
  const size=page.locator(".clipper-watermark-controls input[type=range]");
  const tag=page.locator(".clipper-social-tag");
  for(const platformValue of ["kick","twitch"]){
    await platform.selectOption(platformValue);
    for(const styleValue of ["plain","boxed"]){
      await style.selectOption(styleValue);
      for(const fontSize of ["20","36"]){
        await size.fill(fontSize);
        const icon=await tag.locator(".clipper-social-icon").boundingBox();
        const text=await tag.locator(".clipper-social-name").boundingBox();
        expect(icon).not.toBeNull();expect(text).not.toBeNull();
        expect(Math.abs(icon!.y+icon!.height/2-text!.y-text!.height/2)).toBeLessThan(1);
        expect(await tag.evaluate(el=>getComputedStyle(el).transform)).not.toBe("none");
      }
    }
  }
  await style.selectOption("plain");
  await platform.selectOption("kick");
  await size.fill("36");
  const centered=await tag.evaluate(el=>{
    const box=el.getBoundingClientRect(),parent=el.parentElement!.getBoundingClientRect();
    return {visual:box.x+box.width/2,anchor:parent.x+parseFloat(el.style.left)};
  });
  expect(Math.abs(centered.visual-centered.anchor)).toBeLessThan(2);
  await page.screenshot({path:"test-results/social-tag-eray-preview.png"});
  await stageMocks(page);
  await page.getByRole("button",{name:"▶ render clipper",exact:true}).click();
  await expect(page.getByRole("button",{name:"show output",exact:true})).toBeVisible();
  await page.getByRole("combobox",{name:"Position"}).selectOption("left");
  await expect(page.locator(".job-head p")).toHaveText("settings changed · render again");
  await expect(page.getByRole("button",{name:"previous output",exact:true})).toBeVisible();
});

test("GIF IN and OUT respond to the player position", async ({ page }) => {
  await mockDesktop(page);
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("GIF Maker");
  await page.getByText("GIF Maker", { exact: true }).last().click();
  await page.getByRole("slider", { name: "Video position" }).fill("10");
  await page.getByRole("button", { name: /IN I/ }).click();
  await page.getByRole("slider", { name: "Video position" }).fill("20");
  await page.getByRole("button", { name: /OUT O/ }).click();
  await expect(page.getByRole("textbox", { name: "Start time" })).toHaveValue("0:00:10");
  await expect(page.getByRole("textbox", { name: "End time" })).toHaveValue("0:00:20");
});

test("an explicit startup file takes precedence over stale recovery", async ({ page }) => {
  await mockDesktop(page, { interrupted: true, savedRecovery: true, startupPath: sample.path });
  await expect(page.locator(".settings")).toBeVisible();
  await expect(page.locator(".recovery-card")).toHaveCount(0);
});

test("interrupted work restores the preview and editor", async ({ page }) => {
  await mockDesktop(page, { interrupted: true, savedRecovery: true });
  await expect(page.locator(".recovery-card")).toBeVisible();
  await page.getByRole("button", { name: "RESTORE WORK" }).click();
  await expect(page.locator(".settings")).toBeVisible();
  await expect(page.locator(".recovery-card")).toHaveCount(0);
});

test("language switch changes and remembers the landing UI", async ({ page }) => {
  await mockDesktop(page);
  await page.locator(".landing-language").getByRole("button", { name: "TR" }).click();
  await expect(page.locator(".landing-project-actions")).toHaveText("PROJE AÇ");
  await page.reload();
  await expect(page.locator(".landing-project-actions")).toHaveText("PROJE AÇ");
  await page.locator(".landing-language").getByRole("button", { name: "EN" }).click();
  await expect(page.locator(".landing-project-actions")).toHaveText("OPEN PROJECT");
});

test("save warns about missing project files without a header check button", async ({ page }) => {
  await mockDesktop(page, { missingProjectSource: true });
  await openFixture(page);
  await expect(page.getByRole("button", { name: "CHECK FILES" })).toHaveCount(0);
  await page.getByRole("button", { name: "SAVE PROJECT" }).click();
  await expect(page.getByRole("dialog", { name: "PROJECT FILES" })).toBeVisible();
  await expect(page.locator(".project-file-list")).toContainText("Source media");
  await expect(page.locator(".project-file-list")).toContainText(sample.path);
  await expect(page.locator(".project-files-warning")).toBeVisible();
});

test("Toolbox and Batch keep the quick interface without saved presets", async ({ page }) => {
  await mockDesktop(page);
  await openFixture(page);
  await expect(page.locator(".tool-presets")).toHaveCount(0);
  await page.getByRole("button", { name: "BATCH" }).click();
  await expect(page.locator(".batch-workspace")).toBeVisible();
  await expect(page.locator(".tool-presets")).toHaveCount(0);
});

test("workspace roundtrips preserve Clipper edits, SmartCut cuts and Batch inputs", async ({ page }) => {
  await mockDesktop(page);
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper", {exact:true}).last().click();
  await page.getByText("Social Tag", {exact:true}).click();
  await page.getByPlaceholder("kanaladi").fill("keep_this_name");
  await page.getByRole("button", {name:"SMARTCUT",exact:true}).click();
  await page.getByRole("button", {name:"DETECT SILENCE",exact:true}).click();
  await expect(page.locator(".cut-list article")).toHaveCount(2);
  const cutStart=page.locator(".cut-list article").first().locator('input').first();
  await cutStart.fill("3");
  await cutStart.press("Tab");
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(async()=>page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="write_project").length)).toBe(1);
  expect(await page.evaluate(()=>JSON.parse((window as any).__TEST_CALLS__.find((call:any)=>call.cmd==="write_project").args.contents).autocut.cuts[0].start)).toBe(3);
  // No wait for the debounced session callback: the switch must capture now.
  await page.getByRole("button", {name:"BATCH",exact:true}).click();
  await page.getByRole("button", {name:"+ FILES",exact:true}).click();
  await page.locator(".batch-control select").first().selectOption("remove_audio");
  await page.getByRole("button", {name:"TOOLBOX",exact:true}).click();
  await expect(page.getByPlaceholder("kanaladi")).toHaveValue("keep_this_name");
  await page.getByRole("button", {name:"SMARTCUT",exact:true}).click();
  await expect(page.locator(".cut-list article")).toHaveCount(2);
  await expect(cutStart).toHaveValue("3.000");
  await page.getByRole("button", {name:"BATCH",exact:true}).click();
  await expect(page.locator(".batch-items article")).toHaveCount(2);
  await expect(page.locator(".batch-control select").first()).toHaveValue("remove_audio");
});

test("Batch locks its controls and uses one captured operation for the queue", async ({ page }) => {
  await mockDesktop(page);
  await openFixture(page);
  await page.evaluate(()=>{
    (window as any).__TEST_PENDING__=[];
    (window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="run_operation"
      ? new Promise(resolve=>(window as any).__TEST_PENDING__.push(()=>resolve({output:"C:\\fixtures\\out.mp4",elapsed:1}))) : undefined;
  });
  await page.getByRole("button", {name:"BATCH",exact:true}).click();
  await page.getByRole("button", {name:"+ FILES",exact:true}).click();
  await page.getByRole("button", {name:/START QUEUE/}).click();
  await page.waitForFunction(()=>(window as any).__TEST_PENDING__.length===1);
  const operation=page.locator(".batch-control select").first();
  await expect(operation).toBeDisabled();
  await expect(page.getByRole("button",{name:"+ FILES",exact:true})).toBeDisabled();
  // Even a synthetic change cannot mutate the captured queue operation.
  await operation.evaluate((element:HTMLSelectElement)=>{element.value="remove_audio";element.dispatchEvent(new Event("change",{bubbles:true}))});
  await page.evaluate(()=>(window as any).__TEST_PENDING__.shift()());
  await page.waitForFunction(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").length===2);
  const requests=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").map((call:any)=>call.args.request));
  expect(requests.map((request:any)=>request.operation)).toEqual(["encode","encode"]);
  expect(requests[1].params).toEqual(requests[0].params);
  await page.evaluate(()=>(window as any).__TEST_PENDING__.shift()());
  await expect(operation).toBeEnabled();
  await expect(page.locator(".batch-items article > strong")).toHaveText(["complete","complete"]);
});

test("SmartCut reanalyzes in-flight edits and only exports current results", async ({ page }) => {
  await mockDesktop(page);
  await openFixture(page);
  await page.evaluate(()=>{
    (window as any).__TEST_PENDING__=[];
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="analyze_autocut"
      ? new Promise(resolve=>(window as any).__TEST_PENDING__.push(()=>resolve({cuts:[{start:args.request.minimum_pause,end:8,enabled:true}],waveform:[],duration:60}))) : undefined;
  });
  await page.getByRole("button", {name:"SMARTCUT",exact:true}).click();
  await page.getByRole("button", {name:"DETECT SILENCE",exact:true}).click();
  await page.waitForFunction(()=>(window as any).__TEST_PENDING__.length===1);
  await page.locator('.ac-fields input[type="range"]').first().fill("0.75");
  await page.evaluate(()=>(window as any).__TEST_PENDING__.shift()());
  await page.waitForFunction(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="analyze_autocut").length===2);
  await expect(page.getByRole("button",{name:"EXPORT MP4",exact:true})).toBeDisabled();
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="analyze_autocut").map((call:any)=>call.args.request.minimum_pause))).toEqual([0.35,0.75]);
  await page.evaluate(()=>(window as any).__TEST_PENDING__.shift()());
  await expect(page.getByRole("button",{name:"EXPORT MP4",exact:true})).toBeEnabled();
  await expect(page.locator(".cut-list article input").first()).toHaveValue("0.750");
});

test("missing project source can be relinked through the native confirmation IPC", async ({ page }) => {
  await mockDesktop(page, {interrupted:true,savedRecovery:true});
  await page.evaluate(()=>{
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
      if(cmd==="project_media_available")return args.path==="C:\\fixtures\\relocated.mp4";
      if(cmd==="plugin:dialog|message")return args.buttons.OkCancelCustom[0];
      if(cmd==="plugin:dialog|open")return "C:\\fixtures\\relocated.mp4";
    };
  });
  await page.getByRole("button",{name:"RESTORE WORK",exact:true}).click();
  await expect(page.locator(".settings")).toBeVisible();
  await expect(page.locator(".recovery-card")).toHaveCount(0);
  const calls=await page.evaluate(()=>(window as any).__TEST_CALLS__);
  expect(calls.some((call:any)=>call.cmd==="plugin:dialog|message")).toBe(true);
  expect(calls.some((call:any)=>call.cmd==="authorize_media_preview"&&call.args.path==="C:\\fixtures\\relocated.mp4")).toBe(true);
});

test("compact SmartCut controls, GIF ruler and project files remain readable", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 768 });
  await page.emulateMedia({ colorScheme: "dark" });
  await mockDesktop(page, { missingProjectSource: true });
  await openFixture(page);
  await page.getByRole("button", { name: "SMARTCUT" }).click();
  await expect(page.locator(".ac-layout")).toBeVisible();
  const smartCut = await page.evaluate(() => {
    const box = (selector: string) => document.querySelector(selector)!.getBoundingClientRect();
    return { title: box(".tl-title").bottom, status: box(".tl-status").bottom, buttons: box(".tl-buttons").top, panel: box(".ac-timeline").bottom, navigator: box(".navigator").bottom };
  });
  expect(smartCut.buttons).toBeGreaterThanOrEqual(Math.max(smartCut.title, smartCut.status) - 1);
  expect(smartCut.navigator).toBeLessThanOrEqual(smartCut.panel + 1);
  if (process.env.UI_AUDIT_SCREENSHOTS) await page.screenshot({ path: "test-results/fixed-smartcut-compact.png" });

  await page.getByRole("button", { name: "TOOLBOX" }).click();
  await page.getByPlaceholder("search tools...").fill("GIF Maker");
  await page.getByText("GIF Maker", { exact: true }).last().click();
  const gif = await page.evaluate(() => ({ ruler: document.querySelector(".tool-ruler")!.getBoundingClientRect().bottom, panel: document.querySelector(".tool-timeline")!.getBoundingClientRect().bottom }));
  expect(gif.ruler).toBeLessThanOrEqual(gif.panel + 1);
  if (process.env.UI_AUDIT_SCREENSHOTS) await page.screenshot({ path: "test-results/fixed-gif-compact.png" });

  await page.getByRole("button", { name: "SAVE PROJECT" }).click();
  await expect(page.locator(".project-file-list strong")).toBeVisible();
  const files = await page.evaluate(() => ({ label: document.querySelector(".project-file-list strong")!.getBoundingClientRect().bottom, path: document.querySelector(".project-file-list small")!.getBoundingClientRect().top }));
  expect(files.path).toBeGreaterThanOrEqual(files.label);
  if (process.env.UI_AUDIT_SCREENSHOTS) await page.screenshot({ path: "test-results/fixed-project-files-compact.png" });
});
