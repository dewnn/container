import { expect, test, type Page } from "@playwright/test";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const dialogMessagesAllowed = JSON.parse(readFileSync(new URL("../../../src-tauri/capabilities/default.json", import.meta.url), "utf8")).permissions.includes("dialog:allow-message");

const sample = {
  path: "C:\\fixtures\\sample.mp4", name: "sample.mp4", kind: "video", duration: 60,
  width: 1920, height: 1080, fps: 30, codec: "h264", audio_codec: "aac",
  audio_tracks: [], pixel_format: "yuv420p", bits_per_raw_sample: 8,
  color_transfer: null, color_primaries: null, color_space: null,
  bitrate: 3_000_000, size: 1_000_000, start_timecode: null,
};

async function mockDesktop(page: Page, options: { invalidSecond?: boolean; interrupted?: boolean; startupPath?: string | null; savedRecovery?: boolean; missingProjectSource?: boolean; fixture?: typeof sample; version?: string } = {}) {
  await page.addInitScript(({ media, invalidSecond, interrupted, startupPath, savedRecovery, missingProjectSource, dialogMessagesAllowed, version }) => {
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
          if (args.options?.multiple) return args.options.filters?.[0]?.extensions?.includes("bmp") ? [media.path] : ["C:\\fixtures\\second.mp4"];
          return invalidSecond && fileOpens > 1 ? "C:\\fixtures\\broken.mp4" : media.path;
        }
        if (cmd === "plugin:dialog|save") return "C:\\fixtures\\sample.cproj";
        if (cmd === "plugin:dialog|message") return "Cancel";
        if (cmd === "write_project") return null;
        if (cmd === "plugin:app|version") return version ?? "0.16.0-dev.1";
        if (cmd === "startup_media_path") return startupPath ?? null;
        if (cmd === "previous_session_interrupted") return !!interrupted;
        if (cmd === "ffmpeg_status") return { ready: true, ffmpeg_version: "9.0.1", ffprobe_version: "9.0.1" };
        if (cmd === "ffmpeg_capabilities") return { vidstab: true, subtitles: true, overlay: true, blur: true, concat: true };
        if (cmd === "downloader_status") return { ready: true, version: "test" };
        if (cmd === "list_download_history") return [];
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
        if (cmd === "recommend_autocut_settings") return {threshold:0.57,min_silence:0.12,min_speech:0.16,minimum_pause:0.41,keep_before_speech:0.11,keep_after_speech:0.19,noise_floor_db:-82.4,speech_level_db:-30.2};
        if (cmd === "analyze_autocut") return {cuts:[{start:2,end:8,enabled:true},{start:15,end:25,enabled:true}],waveform:[],duration:60};
        return null;
      },
    };
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: mock });
    (window as any).__TEST_DROP__ = (path: string | string[]) => {
      const id = events.get("tauri://drag-drop");
      if (id) callbacks.get(id)?.({ event: "tauri://drag-drop", payload: { paths: Array.isArray(path) ? path : [path], position: { x: 0, y: 0 } } });
    };
    (window as any).__TEST_EVENT__ = (name: string, payload: unknown = null) => {
      const id = events.get(name);
      if (id) callbacks.get(id)?.({ event: name, payload });
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

test("Processing Stack preserves order, toggles steps and renders one request", async ({page}) => {
  await mockDesktop(page); await openFixture(page); await stageMocks(page);
  await page.getByPlaceholder("search tools...").fill("Cut Video");
  await page.locator(".tool-row").filter({has:page.getByText("Cut Video",{exact:true})}).click();
  await page.getByRole("button",{name:"+ Add to stack"}).click();
  await page.getByPlaceholder("search tools...").fill("Color Adjustment");
  await page.locator(".tool-row").filter({has:page.getByText("Color Adjustment",{exact:true})}).click();
  await page.getByRole("slider",{name:"Brightness"}).fill("10");
  await page.getByRole("button",{name:"+ Add to stack"}).click();
  await expect(page.locator(".processing-stack-step")).toHaveCount(2);
  await page.locator(".processing-stack-step").nth(1).getByRole("button",{name:"Move up"}).click();
  await page.locator(".processing-stack-step").nth(1).getByRole("checkbox").uncheck();
  await page.getByRole("button",{name:"Render processing stack"}).click();
  const request=await page.evaluate(()=>(window as any).__TEST_CALLS__.find((call:any)=>call.cmd==="run_operation"));
  expect(request.args.request.operation).toBe("processing_stack");
  const steps=JSON.parse(request.args.request.params.steps);
  expect(steps.map((step:any)=>step.operation)).toEqual(["color","cut"]);
  expect(steps.map((step:any)=>step.enabled)).toEqual([true,false]);
  await expect(page.locator(".job-status-line")).toContainText("complete");
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>JSON.parse((window as any).__SAVED_PROJECT__).toolbox.processingStack.length)).toBe(2);
  await page.evaluate(()=>(window as any).__TEST_DROP__("C:\\fixtures\\stack.cproj"));
  await expect(page.locator(".processing-stack-step")).toHaveCount(2);
  await expect(page.locator(".processing-stack-step").first()).toContainText("Color Adjustment");
});

test("Stack project and source history restore the selected step without duplicates",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await expect(page.locator(".stage-trigger")).toBeVisible();
  await page.locator(".stage-trigger").click();
  await expect(page.locator(".stage-list button")).toHaveCount(1);
  await expect(page.locator(".stage-menu")).toContainText("No other sources yet.");
  await page.keyboard.press("Escape");
  await page.getByRole("button",{name:"+ Add to stack",exact:true}).click();
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>JSON.parse((window as any).__SAVED_PROJECT__).toolbox.editingStackStepId)).toBe(1);
  await page.evaluate(()=>(window as any).__TEST_DROP__("C:\\fixtures\\stack.cproj"));
  await expect(page.getByRole("button",{name:"Update selected step",exact:true})).toBeVisible();
  await page.getByRole("button",{name:"Update selected step",exact:true}).click();
  await expect(page.locator(".processing-stack-step")).toHaveCount(1);
  await page.getByRole("button",{name:"Render processing stack"}).click();
  await page.getByRole("button",{name:"Open as source",exact:true}).click();
  await page.locator(".stage-trigger").click();
  await expect(page.locator(".stage-trigger")).toContainText("SOURCE HISTORY");
  await expect(page.locator(".stage-menu")).not.toContainText("No other sources yet.");
  await expect(page.locator(".stage-list button").first()).toContainText("Processing Stack");
  await page.locator(".stage-list button").first().click();
  await expect(page.getByRole("button",{name:"Update selected step",exact:true})).toBeVisible();
  await page.getByRole("button",{name:"Update selected step",exact:true}).click();
  await expect(page.locator(".processing-stack-step")).toHaveCount(1);
});

test("Processing Stack carries Clipper into SmartCut and renders saved cuts with it",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.locator(".tool-row").filter({has:page.getByText("Clipper",{exact:true})}).click();
  await page.getByRole("button",{name:"+ Add to stack"}).click();
  await expect(page.locator(".processing-stack-step")).toHaveCount(1);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  await expect(page.locator(".ac-layout")).toBeVisible();
  await page.getByRole("button",{name:"DETECT SILENCES",exact:true}).click();
  await page.getByRole("button",{name:"+ Add cuts to stack",exact:true}).click();
  await expect(page.locator(".processing-stack-step")).toHaveCount(2);
  await expect(page.locator(".processing-stack-step").first()).toContainText("SmartCut");
  await expect(page.locator(".processing-stack-step").first().getByRole("button",{name:"Move down"})).toBeDisabled();
  await expect(page.locator(".processing-stack-step").nth(1).getByRole("button",{name:"Move up"})).toBeDisabled();
  await page.getByRole("button",{name:"Render processing stack"}).click();
  const request=await page.evaluate(()=>(window as any).__TEST_CALLS__.find((call:any)=>call.cmd==="run_operation"));
  const steps=JSON.parse(request.args.request.params.steps);
  expect(steps.map((step:any)=>step.operation)).toEqual(["smartcut","clipper"]);
  expect(JSON.parse(steps[0].params.cuts)).toEqual([{start:2,end:8,enabled:true},{start:15,end:25,enabled:true}]);
  await page.locator(".processing-stack-name").first().click();
  await expect(page.locator(".ac-layout")).toBeVisible();
  await expect(page.getByRole("button",{name:"Update cuts in stack",exact:true})).toBeEnabled();
  await page.getByRole("button",{name:"Update cuts in stack",exact:true}).click();
  await expect(page.locator(".processing-stack-step")).toHaveCount(2);
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>JSON.parse((window as any).__SAVED_PROJECT__).toolbox.processingStack[0].smartcutSession.cuts.length)).toBe(2);
  await page.evaluate(()=>(window as any).__TEST_DROP__("C:\\fixtures\\stack.cproj"));
  await expect(page.locator(".processing-stack-step")).toHaveCount(2);
  await page.locator(".processing-stack-name").first().click();
  await expect(page.getByRole("button",{name:"Update cuts in stack",exact:true})).toBeEnabled();
});

test("Stack result preview shows the completed file without reapplying Clipper overlays",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.locator(".tool-row").filter({has:page.getByText("Clipper",{exact:true})}).click();
  await page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true}).click();
  await page.getByRole("button",{name:"+ Add to stack",exact:true}).click();
  await expect(page.getByRole("button",{name:"Stack result",exact:true})).toBeDisabled();
  await expect(page.locator(".preview-head")).toContainText("TOOL PREVIEW");
  for(const [width,height] of [[1024,700],[900,600]]){
    await page.setViewportSize({width,height});
    const fits=await page.locator(".stack-preview-toggle").evaluate(el=>{
      const box=el.getBoundingClientRect(),head=el.closest(".preview-head")!.getBoundingClientRect();
      return box.left>=head.left&&box.right<=head.right+1&&box.bottom<=head.bottom+1;
    });
    expect(fits).toBe(true);
  }
  await page.screenshot({path:"test-results/stack-preview-compact.png"});
  await page.setViewportSize({width:1280,height:720});
  await page.getByRole("button",{name:"Render processing stack"}).click();
  await page.getByRole("button",{name:"Stack result",exact:true}).click();
  await expect(page.locator(".stack-result-video")).toHaveAttribute("src",/output-1/);
  await expect(page.locator(".stack-result-video")).not.toHaveAttribute("controls","");
  await page.locator(".stack-result-video").evaluate(video=>{
    Object.defineProperty(video,"duration",{configurable:true,value:12});
    video.dispatchEvent(new Event("loadedmetadata"));
  });
  await expect(page.getByRole("slider",{name:"Video position",exact:true})).toHaveAttribute("max","12");
  await expect(page.locator(".ac-time")).toContainText("00:12.000");
  await expect(page.locator(".job-actions .play-render")).toBeVisible();
  await expect(page.locator(".clipper-output-canvas")).toHaveCount(0);
  await expect(page.locator(".preview-head")).toContainText("STACK RESULT");
  await page.getByRole("button",{name:"Back to tool",exact:true}).click();
  await expect(page.locator(".stack-result-video")).toHaveCount(0);
  await expect(page.locator(".clipper-output-canvas")).toBeVisible();
  await page.getByRole("button",{name:"Medium",exact:true}).click();
  await expect(page.getByRole("button",{name:"Stack result",exact:true})).toBeDisabled();
});

test("Stack scales text pixels after Clipper without mutating editable layers",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  // This test checks raster scaling, not system-font fidelity; use a portable fixture.
  const font=readFileSync(new URL("../../../src-tauri/resources/fonts/Montserrat-ExtraBoldItalic.ttf",import.meta.url)).toString("base64");
  await page.evaluate(font=>{
    const previous=(window as any).__TEST_HANDLER__;
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
      if(cmd==="list_system_fonts")return [{name:"Arial",path:"C:\\Windows\\Fonts\\arial.ttf"}];
      if(cmd==="font_preview_data")return `data:font/ttf;base64,${font}`;
      return previous(cmd,args);
    };
  },font);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.locator(".tool-row").filter({has:page.getByText("Clipper",{exact:true})}).click();
  await page.getByRole("button",{name:"+ Add to stack",exact:true}).click();
  await page.getByPlaceholder("search tools...").fill("Text");
  await page.locator(".tool-row").filter({has:page.getByText("Text",{exact:true})}).click();
  await page.getByRole("button",{name:"＋ Add text",exact:true}).click();
  await page.getByRole("textbox",{name:"Text",exact:true}).fill("watermark 🎬");
  await page.getByRole("button",{name:"+ Add to stack",exact:true}).click();
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  const saved=await page.evaluate(()=>JSON.parse((window as any).__SAVED_PROJECT__).toolbox.processingStack[1]);
  await page.getByRole("button",{name:"Render processing stack"}).click();
  await expect(page.locator(".job-status")).toContainText("complete");
  const layers=await page.evaluate(()=>{
    const call=(window as any).__TEST_CALLS__.find((c:any)=>c.cmd==="run_operation");
    return JSON.parse(JSON.parse(call.args.request.params.steps)[1].params.layers);
  });
  expect(layers[0].size).toBeCloseTo(saved.textLayers[0].size*1080/1920);
  expect(layers[0].x).toBe(saved.textLayers[0].x);expect(layers[0].y).toBe(saved.textLayers[0].y);
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  expect(await page.evaluate(()=>JSON.parse((window as any).__SAVED_PROJECT__).toolbox.processingStack[1].textLayers[0].size)).toBe(saved.textLayers[0].size);
});

test("Clipper output canvas restores decoded frames after SmartCut workspace switching",async({page})=>{
  const root=resolve("test-results","clipper-workspace-frames");mkdirSync(root,{recursive:true});
  const source=resolve(root,"source.mp4");
  expect(spawnSync("ffmpeg",["-v","error","-y","-f","lavfi","-i","color=red:s=640x360:r=30:d=1","-c:v","libx264","-pix_fmt","yuv420p",source]).status).toBe(0);
  await page.route("http://asset.localhost/**",route=>route.fulfill({body:readFileSync(source),contentType:"video/mp4",headers:{"Access-Control-Allow-Origin":"*"}}));
  await mockDesktop(page,{fixture:{...sample,width:640,height:360,duration:1}});await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.locator(".tool-row").filter({has:page.getByText("Clipper",{exact:true})}).click();
  await page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true}).click();
  const colouredPixels=async()=>{const screenshot=await page.locator(".clipper-output-canvas").screenshot();const pixels=spawnSync("ffmpeg",["-v","error","-i","pipe:0","-vf","scale=32:32","-f","rawvideo","-pix_fmt","rgb24","pipe:1"],{input:screenshot}).stdout;let coloured=0;for(let i=0;i<pixels.length;i+=3)if(pixels[i]>100&&pixels[i+1]<80)coloured++;return coloured};
  await expect.poll(colouredPixels).toBeGreaterThan(20);
  for(let i=0;i<3;i++){
    await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
    await page.getByRole("button",{name:"TOOLBOX",exact:true}).click();
    await expect(page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true})).toHaveAttribute("aria-pressed","true");
    await expect.poll(colouredPixels).toBeGreaterThan(20);
  }
  await page.getByRole("button",{name:"BLUR",exact:true}).click();
  const blurPixels=async()=>{const screenshot=await page.locator(".blur-backdrop-canvas").screenshot();const pixels=spawnSync("ffmpeg",["-v","error","-i","pipe:0","-vf","scale=32:32","-f","rawvideo","-pix_fmt","rgb24","pipe:1"],{input:screenshot}).stdout;let coloured=0;for(let i=0;i<pixels.length;i+=3)if(pixels[i]>100&&pixels[i+1]<80)coloured++;return coloured};
  await expect.poll(blurPixels).toBeGreaterThan(20);
  await expect(page.locator(".video-canvas video")).toHaveCount(1);
  for(let i=0;i<3;i++){
    await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
    await page.getByRole("button",{name:"TOOLBOX",exact:true}).click();
    await expect.poll(blurPixels).toBeGreaterThan(20);
  }
});

test("Processing Stack cancellation releases the editor for another render",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Cut Video");
  await page.locator(".tool-row").filter({has:page.getByText("Cut Video",{exact:true})}).click();
  await page.getByRole("button",{name:"+ Add to stack"}).click();
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="run_operation"?new Promise((_resolve,reject)=>(window as any).__FAIL_STACK__=()=>reject(new Error("Job cancelled."))):undefined});
  await page.getByRole("button",{name:"Render processing stack"}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").length)).toBe(1);
  await page.getByRole("button",{name:"cancel job",exact:true}).click();
  await page.evaluate(()=>(window as any).__FAIL_STACK__());
  await expect(page.locator(".job-status-line")).toContainText("cancelled");
  await expect(page.getByRole("button",{name:"Render processing stack"})).toBeEnabled();
});

test("SmartCut export and Stack actions remain reachable in compact windows",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  await page.getByRole("button",{name:"DETECT SILENCES",exact:true}).click();
  for(const size of [{width:1920,height:1000},{width:1440,height:850},{width:1024,height:700},{width:900,height:600}]){
    await page.setViewportSize(size);
    const exportButton=page.getByRole("button",{name:"EXPORT MP4",exact:true});
    const addButton=page.getByRole("button",{name:"+ Add cuts to stack",exact:true});
    await expect(exportButton).toBeInViewport();await expect(addButton).toBeInViewport();
    if(size.height===1000){
      expect(await page.locator(".ac-left-scroll").evaluate(el=>el.scrollHeight-el.clientHeight)).toBeLessThanOrEqual(1);
    }
    await expect.poll(()=>page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);
    await page.locator(".ac-left-scroll").evaluate(el=>el.scrollTop=el.scrollHeight);
    await expect(exportButton).toBeInViewport();await expect(addButton).toBeInViewport();
    await page.locator(".ac-left-scroll").evaluate(el=>el.scrollTop=0);
    if(size.width===900)await page.screenshot({path:"test-results/smartcut-compact-actions.png"});
  }
  await page.getByRole("button",{name:"+ Add cuts to stack",exact:true}).click();
  await expect(page.getByRole("button",{name:"Render processing stack"})).toBeInViewport();
});

test("Adding SmartCut cuts cannot silently discard an unapplied Clipper edit",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.locator(".tool-row").filter({has:page.getByText("Clipper",{exact:true})}).click();
  await page.getByRole("button",{name:"+ Add to stack"}).click();
  await page.getByRole("button",{name:"SQUARES",exact:true}).click();
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  await page.getByRole("button",{name:"DETECT SILENCES",exact:true}).click();
  await page.getByRole("button",{name:"+ Add cuts to stack",exact:true}).click();
  await expect(page.locator(".app-toast")).toContainText("unapplied changes");
  await expect(page.locator(".ac-layout")).toBeVisible();
  await page.getByRole("button",{name:"TOOLBOX",exact:true}).click();
  await expect(page.getByRole("button",{name:"Render processing stack"})).toBeDisabled();
  await page.getByRole("button",{name:"Update selected step",exact:true}).click();
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  await page.getByRole("button",{name:"+ Add cuts to stack",exact:true}).click();
  await expect(page.locator(".processing-stack-step")).toHaveCount(2);
});

test("Frame Extractor simple mode starts at the playhead with count and millisecond spacing",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByPlaceholder("search tools...").fill("Frame Extractor");
  await page.locator(".tool-row").filter({has:page.getByText("Frame Extractor",{exact:true})}).click();
  await expect(page.getByRole("combobox",{name:"Mode"})).toHaveCount(0);
  await page.getByRole("spinbutton",{name:"Number of frames to extract"}).fill("5");
  await page.getByRole("spinbutton",{name:"Distance between frames"}).fill("200");
  await page.locator(".video-canvas video").evaluate(el=>{Object.defineProperty(el,"currentTime",{value:7.25,configurable:true});el.dispatchEvent(new Event("timeupdate"))});
  await expect(page.locator(".frame-extractor-simple")).toContainText("00:07.250");
  await page.getByRole("button",{name:/render frame extractor/i}).click();
  const request=await page.evaluate(()=>(window as any).__TEST_CALLS__.find((call:any)=>call.cmd==="run_operation")?.args.request);
  expect(request.params).toMatchObject({mode:"burst",start:"7.25",end:"60",count:"5",interval:"0.2"});
});

test("Frame Extractor sends the selected range and contact-sheet settings",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByPlaceholder("search tools...").fill("Frame Extractor");
  await page.locator(".tool-row").filter({has:page.getByText("Frame Extractor",{exact:true})}).click();
  await page.getByRole("button",{name:"Advanced options",exact:true}).click();
  await expect(page.getByRole("spinbutton",{name:"End"})).toHaveValue("60");
  await page.getByRole("combobox",{name:"Mode"}).selectOption("sheet");
  await page.getByRole("spinbutton",{name:"Frame count"}).fill("20");
  await page.getByRole("spinbutton",{name:"Columns"}).fill("5");
  await page.getByRole("button",{name:/render frame extractor/i}).click();
  const request=await page.evaluate(()=>(window as any).__TEST_CALLS__.find((call:any)=>call.cmd==="run_operation")?.args.request);
  expect(request.operation).toBe("frame_extractor");
  expect(request.params).toMatchObject({mode:"sheet",end:"60",count:"20",columns:"5"});
});

test("Audio Lab keeps video-only format controls hidden and sends loudness preset",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByRole("button",{name:"audio",exact:true}).click();
  await page.getByPlaceholder("search tools...").fill("Audio Lab");
  await page.locator(".tool-row").filter({has:page.getByText("Audio Lab",{exact:true})}).click();
  await expect(page.getByRole("combobox",{name:"Audio output"})).toHaveCount(0);
  await page.getByRole("combobox",{name:"Preset"}).selectOption("youtube");
  await page.getByRole("button",{name:/render audio lab/i}).click();
  const request=await page.evaluate(()=>(window as any).__TEST_CALLS__.find((call:any)=>call.cmd==="run_operation")?.args.request);
  expect(request.operation).toBe("audio_lab");
  expect(request.params.preset).toBe("youtube");
});

test("late project read cannot replace a newer media selection", async ({page}) => {
  await mockDesktop(page); await openFixture(page); await stageMocks(page);
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).__SAVED_PROJECT__)).toBeTruthy();
  await page.evaluate(()=>{
    const previous=(window as any).__TEST_HANDLER__;
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
      if(cmd==="probe_media"&&args.path.endsWith("newer.mp4"))return {...JSON.parse((window as any).__SAVED_PROJECT__).toolbox.media,path:args.path,name:"newer.mp4"};
      if(cmd==="read_project")return new Promise(resolve=>(window as any).__LATE_PROJECT__=()=>resolve((window as any).__SAVED_PROJECT__));
      return previous?.(cmd,args);
    };
    (window as any).__TEST_DROP__("C:\\fixtures\\old.cproj");
  });
  await expect.poll(()=>page.evaluate(()=>!!(window as any).__LATE_PROJECT__)).toBe(true);
  await page.evaluate(()=>(window as any).__TEST_DROP__("C:\\fixtures\\newer.mp4"));
  await expect(page.locator(".filename")).toContainText("newer.mp4");
  await page.evaluate(()=>(window as any).__LATE_PROJECT__());
  await expect.poll(()=>page.evaluate(()=>(window as any).__TEST_CALLS__.filter((c:any)=>c.cmd==="read_project").length)).toBe(1);
  await page.waitForTimeout(300);
  await expect(page.locator(".filename")).toContainText("newer.mp4");
});

test("late cancel response does not overwrite a completed render", async ({page}) => {
  await mockDesktop(page); await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Cut Video");
  await page.locator(".tool-row").filter({has:page.getByText("Cut Video",{exact:true})}).click();
  await page.evaluate(()=>{
    (window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="run_operation"
      ?new Promise(resolve=>(window as any).__FINISH_JOB__=()=>resolve({output:"C:\\fixtures\\finished.mp4",elapsed:.1}))
      :cmd==="cancel_job"?new Promise(resolve=>(window as any).__FINISH_CANCEL__=()=>resolve(null)):undefined;
  });
  await page.getByRole("button",{name:/render cut video/i}).click();
  await page.getByRole("button",{name:"cancel job",exact:true}).click();
  await page.evaluate(()=>(window as any).__FINISH_JOB__());
  await expect(page.locator(".job-status-line")).toContainText("complete");
  await page.evaluate(()=>(window as any).__FINISH_CANCEL__());
  await expect(page.locator(".job-status-line")).toContainText("complete");
});

test("active render ignores stale job events and cancels only its own identity",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Cut Video");
  await page.locator(".tool-row").filter({has:page.getByText("Cut Video",{exact:true})}).click();
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="run_operation"?new Promise(()=>{}):undefined});
  await page.getByRole("button",{name:/render cut video/i}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).__TEST_CALLS__.find((c:any)=>c.cmd==="run_operation")?.args.jobId)).toBeTruthy();
  await page.evaluate(()=>{
    const id=(window as any).__TEST_CALLS__.find((c:any)=>c.cmd==="run_operation").args.jobId;
    (window as any).__TEST_EVENT__("container-progress",{job_id:id,percent:20,time:1,speed:"1x",frame:"20",status:"own job running"});
  });
  await expect(page.locator(".job-status-line")).toContainText("own job running");
  await page.evaluate(()=>(window as any).__TEST_EVENT__("container-progress",{job_id:"previous-job",percent:90,time:9,speed:"9x",frame:"90",status:"stale job"}));
  await expect(page.locator(".job-status-line")).toContainText("own job running");
  await page.getByRole("button",{name:"cancel job",exact:true}).click();
  expect(await page.evaluate(()=>{
    const calls=(window as any).__TEST_CALLS__;
    return calls.find((c:any)=>c.cmd==="cancel_job").args.jobId===calls.find((c:any)=>c.cmd==="run_operation").args.jobId;
  })).toBe(true);
});

test("idle Toolbox ignores another workspace's progress events",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await expect(page.locator(".job-status-line")).toContainText("ready");
  await page.evaluate(()=>(window as any).__TEST_EVENT__("container-progress",{percent:62,time:4,speed:"2x",frame:"90",status:"running"}));
  await expect(page.locator(".job-status-line")).toContainText("ready");
});

test("Toolbox quality analysis still receives its own progress",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Quality / Compression");
  await page.getByText("Quality / Compression",{exact:true}).last().click();
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="analyze_quality"?new Promise(()=>{}):undefined});
  await page.getByRole("button",{name:"Analyze Video",exact:true}).click();
  await page.evaluate(()=>(window as any).__TEST_EVENT__("container-progress",{percent:42,time:4,speed:"1x",frame:"15",status:"analyzing quality sample"}));
  await expect(page.locator(".job-status-line")).toContainText("analyzing quality sample");
});

test("late downloader cancel cannot replace download success",async({page})=>{
  await mockDesktop(page);
  await page.evaluate(()=>{
    (window as any).__TEST_HANDLER__=(cmd:string)=>{
      if(cmd==="analyze_download_url")return {title:"Example",uploader:null,duration:12,thumbnail_path:null,formats:[]};
      if(cmd==="list_download_history")return [{path:"C:\\fixtures\\downloaded.mp4",name:"downloaded.mp4"}];
      if(cmd==="download_media")return new Promise(resolve=>(window as any).__FINISH_DOWNLOAD__=()=>resolve({output_dir:"C:\\fixtures",output_file:"C:\\fixtures\\downloaded.mp4",details:""}));
      if(cmd==="cancel_job")return new Promise(resolve=>(window as any).__FINISH_CANCEL__=()=>resolve(null));
    };
  });
  await page.locator(".downloader-quick-trigger").click();
  await page.getByPlaceholder("https://…").fill("https://example.com/video");
  await page.getByRole("button",{name:"ANALYZE LINK",exact:true}).click();
  await page.getByRole("button",{name:/^↓ DOWNLOAD$/}).click();
  await page.getByRole("button",{name:"CANCEL",exact:true}).click();
  await page.evaluate(()=>(window as any).__FINISH_DOWNLOAD__());
  await expect(page.getByText("Download finished.",{exact:true})).toBeVisible();
  await page.evaluate(()=>(window as any).__FINISH_CANCEL__());
  await expect(page.getByText("Download finished.",{exact:true})).toBeVisible();
});

test("Batch stress preserves 55 unique rows through failure and retry",async({page})=>{
  test.setTimeout(60_000);
  await mockDesktop(page);
  await page.evaluate(()=>{
    const paths=Array.from({length:55},(_,index)=>`C:\\fixtures\\klip (ğ & ${index}).mp4`);
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
      if(cmd==="run_operation"){
        if(args.request.input.includes("& 27)")&&!(window as any).__RETRY_OK__)throw new Error("controlled item failure");
        return {output:`${args.request.input}.out.mp4`,elapsed:.1};
      }
    };
    (window as any).__TEST_DROP__([...paths,paths[0],paths[54]]);
  });
  await expect(page.locator(".batch-items article")).toHaveCount(55);
  await page.getByRole("button",{name:/START QUEUE/}).click();
  await expect(page.locator(".batch-items article>strong").filter({hasText:"complete"})).toHaveCount(54);
  await expect(page.locator(".batch-items article>strong").filter({hasText:"failed"})).toHaveCount(1);
  await page.evaluate(()=>(window as any).__RETRY_OK__=true);
  await page.getByRole("button",{name:/RETRY FAILED/}).click();
  await expect(page.locator(".batch-items article>strong").filter({hasText:"complete"})).toHaveCount(55);
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").length)).toBe(56);
});

test("late async listeners are cleaned after twenty downloader mount cycles",async({page})=>{
  await mockDesktop(page);
  await page.evaluate(()=>{
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
      if(cmd==="plugin:event|listen"&&args.event==="downloader-progress")return new Promise(resolve=>(window as any).__LATE_LISTENER__=resolve);
    };
  });
  for(let index=0;index<20;index++){
    await page.locator(".downloader-quick-trigger").click();
    await expect.poll(()=>page.evaluate(()=>!!(window as any).__LATE_LISTENER__)).toBe(true);
    await page.getByRole("button",{name:"BACK",exact:true}).click();
    await expect(page.locator(".dropzone")).toBeVisible();
    const eventId=8000+index;
    await page.evaluate(id=>{(window as any).__LATE_LISTENER__(id);(window as any).__LATE_LISTENER__=null},eventId);
    await expect.poll(()=>page.evaluate(id=>(window as any).__TEST_CALLS__.filter((c:any)=>c.cmd==="plugin:event|unlisten"&&c.args.eventId===id).length,eventId)).toBe(1);
  }
});

test("Watermark finishes loading after a language change during font loading",async({page})=>{
  // Font-loading races must also run on the Linux frontend runner.
  const font=readFileSync(new URL("../../../src-tauri/resources/fonts/Montserrat-ExtraBoldItalic.ttf",import.meta.url)).toString("base64");
  await mockDesktop(page);await openFixture(page);
  await page.evaluate(encoded=>{
    (window as any).__TEST_HANDLER__=(cmd:string)=>{
      if(cmd==="list_system_fonts")return [{name:"Arial",path:"C:\\Windows\\Fonts\\arial.ttf"}];
      if(cmd==="font_preview_data")return new Promise(resolve=>(window as any).__FINISH_FONT__=()=>resolve(`data:font/ttf;base64,${encoded}`));
    };
  },font);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper",{exact:true}).last().click();
  await page.getByRole("checkbox",{name:"Watermark",exact:true}).check();
  await expect.poll(()=>page.evaluate(()=>!!(window as any).__FINISH_FONT__)).toBe(true);
  await page.getByRole("button",{name:"General settings",exact:true}).click();
  await page.getByRole("combobox",{name:"Language",exact:true}).selectOption("tr");
  await page.getByRole("button",{name:"Tamam",exact:true}).click();
  await page.evaluate(()=>(window as any).__FINISH_FONT__());
  await expect(page.locator(".clipper-watermark-controls textarea")).toBeVisible();
  await expect(page.getByText("Font hazırlanıyor…",{exact:true})).toHaveCount(0);
});

test("Render ETA, optional completion alert and actionable failures",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByPlaceholder("search tools...").fill("Cut Video");
  await page.locator(".tool-row").filter({has:page.getByText("Cut Video",{exact:true})}).click();
  await page.getByRole("button",{name:"General settings",exact:true}).click();
  await page.getByRole("checkbox",{name:"Notify when finished"}).check();
  await page.getByRole("button",{name:"Done",exact:true}).click();
  expect(await page.evaluate(()=>localStorage.getItem("container-completion-alert"))).toBe("true");
  await page.clock.install();
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="run_operation"?new Promise(resolve=>(window as any).__FINISH_RENDER__=resolve):undefined});
  await page.getByRole("button",{name:/render cut video/i}).click();
  await expect(page.locator(".render-feedback")).toContainText("Estimated remaining: —");
  await page.evaluate(()=>(window as any).__TEST_EVENT__("container-progress",{percent:50,time:6,speed:"1x",frame:"100",status:"running"}));
  await page.clock.runFor(6000);
  await expect(page.locator(".render-feedback")).toContainText("~6s");
  await page.evaluate(()=>(window as any).__FINISH_RENDER__({output:"C:\\fixtures\\ready.mp4",elapsed:6}));
  await expect(page.locator(".app-toast")).toContainText("Render complete");
  for(const size of [{width:1440,height:850},{width:900,height:600}]){
    await page.setViewportSize(size);
    const toastBox=(await page.locator(".app-toast").boundingBox())!;
    const playBox=(await page.locator(".play-render").boundingBox())!;
    expect(toastBox.y+toastBox.height).toBeLessThan(playBox.y);
    await page.locator(".play-render").click();
  }
  await expect.poll(()=>page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="plugin:opener|open_path").length)).toBe(2);
  await expect(page.locator(".render-feedback")).not.toContainText("Estimated remaining");
  await page.getByRole("button",{name:"General settings",exact:true}).click();
  await page.getByRole("checkbox",{name:"Notify when finished"}).uncheck();
  await page.getByRole("button",{name:"Done",exact:true}).click();
  await page.clock.runFor(6000);
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="run_operation"?Promise.reject(new Error("No space left on device")):undefined});
  await page.getByRole("button",{name:/render cut video/i}).click();
  await expect(page.locator(".error-box")).toContainText("Free up space and retry");
  await page.locator(".error-box summary").click();
  await expect(page.locator(".error-box pre")).toContainText("No space left on device");
  await expect(page.locator(".app-toast")).not.toContainText("Render complete");
});

test("Cut Video exports the full duration without manually setting OUT",async({page})=>{
  await mockDesktop(page,{fixture:{...sample,duration:100.05988}});
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Cut Video");
  await page.locator(".tool-row").filter({has:page.getByText("Cut Video",{exact:true})}).click();
  await expect(page.getByRole("textbox",{name:"End time"})).toHaveValue("0:01:40.060");
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="run_operation"
    ? Number(args.request.params.end)>100.05988
      ? Promise.reject(new Error("End exceeds source duration"))
      : {output:"C:\\fixtures\\cut.mp4",elapsed:.1}
    : undefined});
  await page.getByRole("button",{name:/render cut video/i}).click();
  await expect(page.locator(".job-head p")).toHaveText("complete");
  const request=await page.evaluate(()=>(window as any).__TEST_CALLS__.find((call:any)=>call.cmd==="run_operation").args.request);
  expect(request.params.start).toBe("0");
  expect(Number(request.params.end)).toBe(100.05988);
});

test("downloaded media opens in the timeline and failed opening keeps the download action",async({page})=>{
  await mockDesktop(page);
  await page.evaluate((fixture)=>{(window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
    if(cmd==="analyze_download_url")return {title:"Example",uploader:"Creator",duration:12,thumbnail_path:null,formats:[{id:"best",label:"Best",detail:"",kind:"video",codec:"h264",rank:1,height:1080}]};
    if(cmd==="download_media"){
      (window as any).__DOWNLOADED__=true;
      return {output_dir:"C:\\fixtures",output_file:"C:\\fixtures\\downloaded.mp4",details:""};
    }
    if(cmd==="list_download_history")return (window as any).__DOWNLOADED__?[{path:"C:\\fixtures\\downloaded.mp4",name:"downloaded.mp4"}]:[];
    if(cmd==="probe_media"&&args.path==="C:\\fixtures\\downloaded.mp4"){
      if((window as any).__FAIL_DOWNLOADED__)throw new Error("Unable to open download");
      return {...fixture,path:args.path,name:"downloaded.mp4"};
    }
    return undefined;
  }},sample);
  await page.locator(".downloader-quick-trigger").click();
  await page.getByPlaceholder("https://…").fill("https://example.com/video");
  await page.getByRole("button",{name:"ANALYZE LINK"}).click();
  await page.getByRole("button",{name:/DOWNLOAD/}).click();
  const openButton=page.getByRole("button",{name:"Open in editor",exact:true});
  await expect(openButton).toBeVisible();
  await page.evaluate(()=>(window as any).__FAIL_DOWNLOADED__=true);
  await openButton.click();
  await expect(openButton).toBeVisible();
  await page.evaluate(()=>(window as any).__FAIL_DOWNLOADED__=false);
  await openButton.click();
  await expect(page.locator(".downloader-workspace")).toHaveCount(0);
  await expect(page.locator(".filename")).toHaveText("downloaded.mp4");
  await expect(page.locator(".tool-timeline")).toBeVisible();
  await expect(page.getByRole("button",{name:/render cut video/i})).toBeVisible();
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="probe_media").at(-1).args.path)).toBe("C:\\fixtures\\downloaded.mp4");
});

test("DWNLDR lists existing downloads, prunes missing files and confirms before recycling",async({page})=>{
  await mockDesktop(page);
  await page.evaluate(()=>{
    (window as any).__DOWNLOAD_FILES__=[
      {path:"C:\\fixtures\\first.mp4",name:"first.mp4"},
      {path:"C:\\fixtures\\second.m4a",name:"second.m4a"},
    ];
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
      if(cmd==="list_download_history")return (window as any).__DOWNLOAD_FILES__;
      if(cmd==="delete_download_history_entry"){
        (window as any).__DOWNLOAD_FILES__=(window as any).__DOWNLOAD_FILES__.filter((entry:any)=>entry.path!==args.path);
        return null;
      }
      return undefined;
    };
  });
  await page.locator(".downloader-quick-trigger").click();
  await expect(page.locator(".download-history li")).toHaveCount(2);
  await page.evaluate(()=>(window as any).__DOWNLOAD_FILES__=[{path:"C:\\fixtures\\first.mp4",name:"first.mp4"}]);
  await page.getByRole("button",{name:"Refresh downloads"}).click();
  await expect(page.locator(".download-history li")).toHaveCount(1);
  await page.getByRole("button",{name:"Delete first.mp4"}).click();
  const dialog=page.getByRole("dialog",{name:"Delete download"});
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("button",{name:"CANCEL"})).toBeFocused();
  await expect(dialog).toContainText("first.mp4");
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/download-delete-dialog.png"});
  await dialog.getByRole("button",{name:"CANCEL"}).click();
  await expect(page.locator(".download-history li")).toHaveCount(1);
  await page.getByRole("button",{name:"Delete first.mp4"}).click();
  await dialog.getByRole("button",{name:"DELETE"}).click();
  await expect(page.locator(".download-history li")).toHaveCount(0);
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.some((call:any)=>call.cmd==="delete_download_history_entry"&&call.args.path==="C:\\fixtures\\first.mp4"))).toBe(true);
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.some((call:any)=>call.cmd==="plugin:dialog|message"))).toBe(false);
});

test("a late history response cannot hide a newly completed download",async({page})=>{
  await mockDesktop(page);
  await page.evaluate(()=>{
    let historyReads=0;
    (window as any).__TEST_HANDLER__=(cmd:string)=>{
      if(cmd==="list_download_history"){
        if(++historyReads===1)return new Promise(resolve=>(window as any).__RESOLVE_OLD_HISTORY__=resolve);
        return [{path:"C:\\fixtures\\downloaded.mp4",name:"downloaded.mp4"}];
      }
      if(cmd==="analyze_download_url")return {title:"Example",uploader:null,duration:12,thumbnail_path:null,formats:[]};
      if(cmd==="download_media")return {output_dir:"C:\\fixtures",output_file:"C:\\fixtures\\downloaded.mp4",details:""};
      return undefined;
    };
  });
  await page.locator(".downloader-quick-trigger").click();
  await page.getByPlaceholder("https://…").fill("https://example.com/video");
  await page.getByRole("button",{name:"ANALYZE LINK",exact:true}).click();
  await page.getByRole("button",{name:/^↓ DOWNLOAD$/}).click();
  await expect(page.getByText("Download finished.",{exact:true})).toBeVisible();
  await page.evaluate(()=>(window as any).__RESOLVE_OLD_HISTORY__([]));
  await expect(page.getByRole("button",{name:"Open in editor",exact:true})).toBeVisible();
  await expect(page.locator(".download-history li")).toHaveCount(1);
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="list_download_history").length)).toBeGreaterThanOrEqual(2);
});

for(const language of ["en","tr"] as const)for(const theme of ["light","dark"] as const){
  test(`download deletion respects ${language}/${theme}, Escape and failed recycling`,async({page})=>{
    await page.addInitScript(({language,theme})=>{localStorage.setItem("container-language",language);localStorage.setItem("container-theme",theme)},{language,theme});
    await mockDesktop(page);
    await page.setViewportSize({width:880,height:700});
    const name="örnek-"+"long-name-".repeat(24)+".mp4";
    await page.evaluate((name)=>{
      (window as any).__TEST_HANDLER__=(cmd:string)=>{
        if(cmd==="list_download_history")return [{path:"C:\\fixtures\\"+name,name}];
        if(cmd==="delete_download_history_entry")throw new Error("File is in use");
        return undefined;
      };
    },name);
    await page.locator(".downloader-quick-trigger").click();
    await page.locator(".history-delete").click();
    const dialog=page.getByRole("dialog",{name:language==="tr"?"İndirmeyi sil":"Delete download"});
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("button",{name:language==="tr"?"İPTAL":"CANCEL",exact:true})).toBeFocused();
    expect(await dialog.evaluate(node=>node.scrollWidth<=node.clientWidth+1)).toBe(true);
    await expect(page.locator("html")).toHaveAttribute("data-theme",theme);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.some((call:any)=>call.cmd==="delete_download_history_entry"))).toBe(false);
    await page.locator(".history-delete").click();
    await dialog.getByRole("button",{name:language==="tr"?"SİL":"DELETE",exact:true}).click();
    await expect(page.locator(".app-toast")).toContainText("File is in use");
    await expect(page.locator(".download-history li")).toHaveCount(1);
    await expect(page.locator(".history-delete")).toBeEnabled();
    expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.some((call:any)=>call.cmd==="plugin:dialog|message"))).toBe(false);
  });
}

test("old project extension reports unsupported format without probing it as media",async({page})=>{
  await mockDesktop(page);
  await page.evaluate(()=>(window as any).__TEST_DROP__("C:\\fixtures\\legacy.containerproject"));
  await expect(page.locator(".app-toast")).toContainText("old project format is no longer supported");
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.some((call:any)=>call.cmd==="probe_media"||call.cmd==="read_project"))).toBe(false);
});

test("downloader back returns to an existing media workspace without losing it",async({page})=>{
  await mockDesktop(page);
  await page.locator(".downloader-quick-trigger").click();
  await page.evaluate(()=>(window as any).__TEST_DROP__("C:\\fixtures\\sample.mp4"));
  await expect(page.locator(".downloader-workspace")).toBeVisible();
  await expect(page.locator(".topbar .downloader-back")).toHaveCount(0);
  const back=page.locator(".download-back-actions .downloader-back");
  await expect(back).toHaveText("← BACK TO EDITOR");
  await back.click();
  await expect(page.locator(".downloader-workspace")).toHaveCount(0);
  await expect(page.locator(".filename")).toHaveText("sample.mp4");
  await expect(page.locator(".settings")).toBeVisible();
});

test("DWLNDR back sits below downloads and returns to the landing page",async({page})=>{
  await mockDesktop(page);
  await page.setViewportSize({width:880,height:700});
  await page.locator(".downloader-quick-trigger").click();
  await expect(page.locator(".topbar .downloader-back")).toHaveCount(0);
  const back=page.locator(".download-back-actions .downloader-back");
  await expect(back).toHaveText("← BACK");
  const history=await page.locator(".download-history").boundingBox();
  const backBox=await back.boundingBox();
  const arrow=await back.locator(".download-back-arrow").boundingBox();
  const label=await back.locator(".download-back-label").boundingBox();
  const note=await page.locator(".download-note").boundingBox();
  expect(history&&backBox&&arrow&&label&&note).toBeTruthy();
  expect(backBox!.y).toBeGreaterThanOrEqual(history!.y+history!.height);
  expect(note!.y).toBeGreaterThanOrEqual(backBox!.y+backBox!.height);
  expect(Math.abs(backBox!.x+backBox!.width/2-(history!.x+history!.width/2))).toBeLessThan(2);
  expect(Math.abs((arrow!.x-backBox!.x)-(backBox!.x+backBox!.width-label!.x-label!.width))).toBeLessThan(1);
  if(process.env.UI_AUDIT_SCREENSHOTS){
    await page.getByRole("button",{name:"Dark theme"}).click();
    await page.screenshot({path:"test-results/downloader-back.png"});
  }
  await back.click();
  await expect(page.locator(".downloader-workspace")).toHaveCount(0);
  await expect(page.locator(".dropzone")).toBeVisible();
});

test("only DEV shows its runtime version in the landing header",async({page})=>{
  await page.setViewportSize({width:1440,height:900});
  await mockDesktop(page,{version:"0.18.2-dev.1"});
  await expect(page.locator(".dev-version")).toHaveText("DEV BUILDv0.18.2-dev.1");
  const badge=await page.locator(".dev-version").boundingBox();
  expect(badge).not.toBeNull();
  expect(Math.abs(badge!.x+badge!.width/2-720)).toBeLessThan(2);
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/dev-version-landing.png"});
  await page.setViewportSize({width:860,height:700});
  const compactBadge=await page.locator(".dev-version").boundingBox();
  const actions=await page.locator(".landing-header-actions").boundingBox();
  expect(compactBadge!.x+compactBadge!.width).toBeLessThan(actions!.x);
  await openFixture(page);
  await expect(page.locator(".dev-version")).toHaveCount(0);
});

test("a second file launch opens in the already-running workspace",async({page})=>{
  await page.addInitScript(()=>{(window as any).isTauri=true});
  await mockDesktop(page);
  await expect.poll(async()=>page.evaluate(()=>(window as any).__TEST_CALLS__.some((call:any)=>call.cmd==="plugin:event|listen"&&call.args.event==="container-open-path"))).toBe(true);
  await page.evaluate(()=>(window as any).__TEST_EVENT__("container-open-path","C:\\fixtures\\second.mp4"));
  await expect(page.locator(".settings")).toBeVisible();
  await expect.poll(async()=>page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="probe_media").at(-1)?.args.path)).toBe("C:\\fixtures\\second.mp4");
});

test("output cleanup keeps the dialog on partial recycle failure and succeeds on retry",async({page})=>{
  await mockDesktop(page);
  await page.evaluate(()=>{
    let attempts=0;
    (window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="clean_output_folder"
      ? ++attempts===1
        ? Promise.reject(new Error("open.mp4 is still in use"))
        : {cleaned:true,path:"C:\\Users\\Test\\Downloads\\CONTAINER Output"}
      : undefined;
  });
  await page.locator(".output-clean-trigger").click();
  await page.locator(".clean-confirm").click();
  await expect(page.locator(".output-clean-layer")).toBeVisible();
  await expect(page.locator("body")).toContainText("open.mp4 is still in use");
  await page.locator(".clean-confirm").click();
  await expect(page.locator(".output-clean-layer")).toHaveCount(0);
  await expect(page.locator(".output-clean-toast")).toContainText("Recycle Bin");
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="clean_output_folder").length)).toBe(2);
});

test("stable releases never show the DEV version badge",async({page})=>{
  await mockDesktop(page,{version:"0.18.2"});
  await expect(page.locator(".dev-version")).toHaveCount(0);
});

test("Geist typography loads throughout Container at full and compact widths",async({page})=>{
  await mockDesktop(page);
  for(const width of [1440,900]){
    await page.setViewportSize({width,height:800});
    await page.evaluate(()=>document.fonts.ready);
    const landing=await page.evaluate(async()=>{
      const [sans,mono]=await Promise.all([document.fonts.load('400 14px "Geist Variable"'),document.fonts.load('500 12px "Geist Mono Variable"')]);
      return {sans:sans.some(face=>face.status==="loaded"),mono:mono.some(face=>face.status==="loaded"),brand:getComputedStyle(document.querySelector(".brand")!).fontFamily,headline:getComputedStyle(document.querySelector(".landing-copy")!).fontFamily,size:parseFloat(getComputedStyle(document.body).fontSize)};
    });
    expect(landing.sans).toBe(true);expect(landing.mono).toBe(true);
    expect(landing.brand).toContain("Geist Variable");expect(landing.headline).toContain("Geist Variable");expect(landing.size).toBe(14);
    if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:`test-results/typography-landing-${width}.png`});
  }
  await openFixture(page);
  const font=(selector:string)=>page.locator(selector).first().evaluate(element=>getComputedStyle(element).fontFamily);
  expect(await font(".tool-row b")).toContain("Geist Variable");
  expect(await font(".chips")).toContain("Geist Mono Variable");
  expect(await font(".settings .tool-settings-head h2")).toContain("Geist Variable");
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/typography-toolbox-900.png"});
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  expect(await font(".ac-card header h3")).toContain("Geist Variable");
  expect(await font(".ac-time")).toContain("Geist Mono Variable");
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/typography-smartcut-900.png"});
  await page.getByRole("button",{name:"BATCH",exact:true}).click();
  expect(await font(".batch-control")).toContain("Geist Variable");
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/typography-batch-900.png"});
});

test("visible workspaces use Container branding rather than the inspiration project name",async({page})=>{
  await mockDesktop(page);
  await expect(page.locator("body")).not.toContainText(/autocut/i);
  await openFixture(page);
  for(const mode of ["TOOLBOX","SMARTCUT","BATCH"]){
    await page.getByRole("button",{name:mode,exact:true}).click();
    await expect(page.locator("body")).not.toContainText(/autocut/i);
    await expect(page.locator(".topbar .brand-logo-dark")).toHaveAttribute("src","/mark-dark.svg");
    await expect(page.locator(".topbar .brand-logo-light")).toHaveAttribute("src","/mark-light.svg");
  }
  await page.evaluate(()=>document.documentElement.dataset.theme="light");
  await expect(page.locator(".topbar .brand-logo-light")).toBeVisible();
  await expect(page.locator(".topbar .brand-logo-dark")).toBeHidden();
});

test("tray hide pauses only media previews, keeps the editing session, and follows language",async({page})=>{
  await page.addInitScript(()=>{(window as any).isTauri=true});
  await mockDesktop(page);
  await expect.poll(()=>page.evaluate(()=>(window as any).__TEST_CALLS__.some((call:any)=>call.cmd==="set_tray_language"&&call.args.language==="en"))).toBe(true);
  await page.locator(".landing-language").getByRole("button",{name:"TR"}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).__TEST_CALLS__.some((call:any)=>call.cmd==="set_tray_language"&&call.args.language==="tr"))).toBe(true);
  await openFixture(page);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  await page.evaluate(()=>{
    const video=document.querySelector(".ac-player video") as HTMLVideoElement;
    (window as any).__TRAY_PAUSED__=false;
    video.pause=()=>{(window as any).__TRAY_PAUSED__=true};
    (window as any).__TEST_EVENT__("container-tray-hidden");
  });
  expect(await page.evaluate(()=>(window as any).__TRAY_PAUSED__)).toBe(true);
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem("container-recovery-v1")??"null")?.workspaceMode)).toBe("autocut");
  await expect(page.getByRole("button",{name:"SMARTCUT",exact:true})).toBeVisible();
  await page.evaluate(()=>(window as any).__TEST_EVENT__("tray-check-updates"));
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="plugin:updater|check").length)).toBe(0);
});

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

async function openPanelReset(page:Page){
  await page.getByRole("button",{name:"General settings",exact:true}).click();
  await page.getByRole("button",{name:"Reset panel layout",exact:true}).click();
}

test("Panel reset in settings asks before restoring saved widths",async({page})=>{
  await mockDesktop(page);
  await expect(page.getByRole("button",{name:"OPEN PROJECT"})).toBeVisible();
  await openFixture(page);
  await expect(page.getByRole("button",{name:"OPEN PROJECT"})).toHaveCount(0);
  await expect(page.getByRole("button",{name:"SAVE PROJECT"})).toBeVisible();
  const divider=page.locator(".workspace-resizer").first();
  await divider.focus();await page.keyboard.press("ArrowRight");
  const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem("container-toolbox-panel-widths")??"null"));
  expect(saved.left).toBeGreaterThan(0);
  await expect(page.locator(".topbar .panel-reset-trigger")).toHaveCount(0);
  await openPanelReset(page);
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
  await openPanelReset(page);
  await dialog.getByRole("button",{name:"CANCEL"}).click();
  await expect(dialog).toHaveCount(0);
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem("container-toolbox-panel-widths")??"null"))).toEqual(saved);
  await openPanelReset(page);
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
    await openPanelReset(page);
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
  const smart=page.locator(".ac-layout"),smartLeft=smart.locator(".ac-left"),smartMiddle=smart.locator(".ac-player"),smartRight=smart.locator(".ac-right");
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
  await expect.poll(async()=>Math.abs((await smartLeft.boundingBox())!.width-smartFinal.left)).toBeLessThan(3);
  await expect.poll(async()=>Math.abs((await smartRight.boundingBox())!.width-smartFinal.right)).toBeLessThan(3);
  await page.reload();await openFixture(page);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  expect(Math.abs((await smartLeft.boundingBox())!.width-smartFinal.left)).toBeLessThan(3);
  expect(Math.abs((await smartRight.boundingBox())!.width-smartFinal.right)).toBeLessThan(3);
  await openPanelReset(page);
  const dialog=page.getByRole("dialog",{name:"Reset panel layout"});
  await expect(dialog).toContainText("SmartCut side panels");
  await dialog.getByRole("button",{name:"CANCEL"}).click();
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem("container-smartcut-panel-widths")??"null"))).toEqual(smartFinal);
  await openPanelReset(page);await dialog.getByRole("button",{name:"RESET"}).click();
  await expect.poll(()=>page.evaluate(()=>JSON.parse(localStorage.getItem("container-smartcut-panel-widths")??"null"))).toEqual({left:null,right:null,timeline:null});
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem("container-batch-panel-widths")??"null"))).toEqual(batchFinal);

  await page.getByRole("button",{name:"BATCH",exact:true}).click();
  expect(Math.abs((await batchLeft.boundingBox())!.width-batchFinal.left)).toBeLessThan(3);
  await openPanelReset(page);await expect(dialog).toContainText("Batch controls panel");
  await dialog.getByRole("button",{name:"RESET"}).click();
  await expect.poll(()=>page.evaluate(()=>JSON.parse(localStorage.getItem("container-batch-panel-widths")??"null"))).toEqual({left:null});
  await page.reload();await openFixture(page);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  expect((await smartLeft.boundingBox())!.width).toBeLessThan(smartFinal.left-40);
  await page.getByRole("button",{name:"BATCH",exact:true}).click();
  expect((await batchLeft.boundingBox())!.width).toBeLessThan(batchFinal.left-40);
});

test("SmartCut timeline spans preview and Cuts, resizes upward and remembers its height",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.setViewportSize({width:1440,height:850});
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  const layout=page.locator(".ac-layout"),player=layout.locator(".ac-player"),cuts=layout.locator(".ac-right"),timeline=layout.locator(".ac-timeline"),divider=layout.getByRole("slider",{name:"SmartCut timeline height"});
  const before={player:(await player.boundingBox())!,cuts:(await cuts.boundingBox())!,timeline:(await timeline.boundingBox())!,divider:(await divider.boundingBox())!};
  expect(Math.abs(before.timeline.x-before.player.x)).toBeLessThan(2);
  expect(Math.abs(before.timeline.x+before.timeline.width-before.cuts.x-before.cuts.width)).toBeLessThan(2);
  expect(before.timeline.y).toBeGreaterThanOrEqual(before.player.y+before.player.height+7);
  expect(before.timeline.y).toBeGreaterThanOrEqual(before.cuts.y+before.cuts.height+7);
  await page.mouse.move(before.divider.x+before.divider.width/2,before.divider.y+4);await page.mouse.down();
  await page.mouse.move(before.divider.x+before.divider.width/2,before.divider.y-95,{steps:8});await page.mouse.up();
  const after={player:(await player.boundingBox())!,cuts:(await cuts.boundingBox())!,timeline:(await timeline.boundingBox())!};
  expect(after.timeline.height-before.timeline.height).toBeGreaterThan(80);
  expect(before.player.height-after.player.height).toBeGreaterThan(80);
  expect(before.cuts.height-after.cuts.height).toBeGreaterThan(80);
  expect(Math.abs(after.timeline.x-before.timeline.x)).toBeLessThan(2);
  const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem("container-smartcut-panel-widths")??"null"));
  expect(saved.timeline).toBeGreaterThan(before.timeline.height+80);
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/smartcut-wide-timeline.png"});
  await page.reload();await openFixture(page);await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  expect(Math.abs((await timeline.boundingBox())!.height-saved.timeline)).toBeLessThan(3);
  await divider.focus();await page.keyboard.press("ArrowDown");
  expect((await timeline.boundingBox())!.height).toBeLessThan(saved.timeline-15);
  await openPanelReset(page);
  await page.getByRole("dialog",{name:"Reset panel layout"}).getByRole("button",{name:"RESET"}).click();
  expect((await timeline.boundingBox())!.height).toBeLessThan(saved.timeline-60);
  for(const width of [1024,900,860]){
    await page.setViewportSize({width,height:700});
    await expect.poll(()=>page.evaluate(()=>{
      const player=document.querySelector(".ac-player")!.getBoundingClientRect(),cuts=document.querySelector(".ac-right")!.getBoundingClientRect(),timeline=document.querySelector(".ac-timeline")!.getBoundingClientRect();
      return {left:Math.round(Math.abs(timeline.left-player.left)),right:Math.round(Math.abs(timeline.right-cuts.right)),overflow:document.documentElement.scrollWidth>innerWidth+1};
    })).toEqual({left:0,right:0,overflow:false});
  }
});

test("SmartCut keeps Auto's recommendation and analysis path in the simplified panel",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  const detection=page.locator(".ac-left-scroll>.ac-card").first();
  await expect(detection.locator(".ac-presets")).toHaveCount(0);
  await expect(detection.locator(".ac-parameter")).toHaveCount(4);
  await expect(detection.getByRole("button",{name:"DETECT SILENCES",exact:true})).toBeVisible();
  if(process.env.UI_AUDIT_SCREENSHOTS){
    await page.screenshot({path:"test-results/smartcut-auto-before.png"});
    await page.evaluate(()=>document.documentElement.dataset.theme="dark");
    await page.screenshot({path:"test-results/smartcut-auto-before-dark.png"});
    await page.evaluate(()=>document.documentElement.dataset.theme="light");
  }
  await detection.getByRole("button",{name:"DETECT SILENCES",exact:true}).click();
  await expect(detection.getByRole("button",{name:"RE-DETECT",exact:true})).toBeVisible();
  await expect(detection.getByRole("slider",{name:"THRESHOLD"})).toHaveValue("0.57");
  await expect(detection.getByRole("slider",{name:"PAD"})).toHaveValue("0.3");
  await expect(detection.locator(".ac-parameter").nth(1).locator("b")).toHaveText("0.30s");
  await expect(detection.getByRole("slider",{name:"MIN SILENCE"})).toHaveValue("0.12");
  await expect(detection.getByRole("slider",{name:"MIN SPEECH"})).toHaveValue("0.16");
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/smartcut-auto-simplified.png"});
  await detection.getByText("OTHER SETTINGS").click();
  await expect(detection.getByRole("slider",{name:"BEFORE"})).toHaveValue("0.11");
  await expect(detection.getByRole("slider",{name:"AFTER"})).toHaveValue("0.19");
  await expect(detection.getByRole("slider",{name:"MINIMUM PAUSE"})).toHaveValue("0.41");
  const calls=()=>page.evaluate(()=>(window as any).__TEST_CALLS__);
  await expect.poll(async()=>((await calls()) as any[]).filter(call=>call.cmd==="analyze_autocut").length).toBe(1);
  const initial=(await calls()) as any[];
  expect(initial.filter(call=>call.cmd==="recommend_autocut_settings")).toHaveLength(1);
  expect(initial.find(call=>call.cmd==="analyze_autocut").args.request).toMatchObject({threshold:0.57,min_silence:0.12,min_speech:0.16,minimum_pause:0.41,keep_before_speech:0.11,keep_after_speech:0.19,boundary_refinement:true});
  await detection.getByRole("slider",{name:"THRESHOLD"}).focus();
  await page.keyboard.press("Shift+ArrowRight");
  await expect(detection.getByRole("slider",{name:"THRESHOLD"})).toHaveValue("0.571");
  await expect.poll(async()=>((await calls()) as any[]).filter(call=>call.cmd==="analyze_autocut").length).toBe(2);
  await detection.getByRole("button",{name:"RE-DETECT",exact:true}).click();
  await expect.poll(async()=>((await calls()) as any[]).filter(call=>call.cmd==="analyze_autocut").length).toBe(3);
  expect(((await calls()) as any[]).filter(call=>call.cmd==="recommend_autocut_settings")).toHaveLength(2);
  expect(((await calls()) as any[]).filter(call=>call.cmd==="analyze_autocut").at(-1)?.args.request.threshold).toBe(0.57);
});

test("SmartCut fine tuning updates cuts and re-detect restores Auto recommendations",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.evaluate(()=>{
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="analyze_autocut"
      ? {cuts:[{start:args.request.min_silence*10,end:8,enabled:true}],waveform:[],duration:60}
      : undefined;
  });
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  const detection=page.locator(".ac-detect-panel");
  await detection.getByRole("button",{name:"DETECT SILENCES",exact:true}).click();
  const cutStart=page.locator(".cut-list article input").first();
  await expect(cutStart).toHaveValue("1.200");
  await detection.getByRole("slider",{name:"MIN SILENCE"}).fill("0.18");
  await expect(cutStart).toHaveValue("1.800");
  await detection.getByRole("slider",{name:"MIN SPEECH"}).fill("0.2");
  await detection.getByRole("slider",{name:"PAD"}).fill("0.4");
  const pad=detection.getByRole("slider",{name:"PAD"});
  await expect(pad).toHaveValue("0.4");
  await pad.focus();await page.keyboard.press("Shift+ArrowRight");
  await expect(pad).toHaveValue("0.401");
  await expect.poll(()=>page.evaluate(()=>{
    const calls=(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="analyze_autocut");
    return calls.at(-1)?.args.request;
  })).toMatchObject({min_silence:0.18,min_speech:0.2,keep_before_speech:0.147,keep_after_speech:0.254});
  await detection.getByRole("button",{name:"RE-DETECT",exact:true}).click();
  await expect(cutStart).toHaveValue("1.200");
  await expect(detection.getByRole("slider",{name:"MIN SILENCE"})).toHaveValue("0.12");
  await expect(detection.getByRole("slider",{name:"MIN SPEECH"})).toHaveValue("0.16");
  await expect(detection.getByRole("slider",{name:"PAD"})).toHaveValue("0.3");
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="recommend_autocut_settings").length)).toBe(2);
});

test("Image favorites count and crop controls stay separate at compact widths",async({page})=>{
  await page.addInitScript(()=>localStorage.setItem("container-favorites",JSON.stringify(["clipper","transform","text","image_compressor","audio_convert"])));
  await mockDesktop(page,{fixture:{...sample,path:"C:\\fixtures\\sample.jpg",name:"sample.jpg",kind:"image",duration:0.04,width:140,height:140,fps:25,codec:"mjpeg"}});
  await openFixture(page);
  await page.evaluate(()=>document.documentElement.dataset.theme="light");
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

test("light theme favorites and category accents remain distinct and readable",async({page})=>{
  await page.addInitScript(()=>localStorage.setItem("container-favorites",JSON.stringify(["clipper","cut"])));
  await mockDesktop(page);await openFixture(page);
  await page.evaluate(()=>document.documentElement.dataset.theme="light");
  const favorites=page.locator(".favorites-filter");
  await favorites.click();
  await expect(favorites).toHaveClass(/active/);
  const palette=await page.evaluate(()=>{
    const css=(selector:string)=>getComputedStyle(document.querySelector(selector)!);
    return {
      filterBackground:css(".favorites-filter").backgroundColor,
      filterStar:css(".favorites-filter span").color,
      activeStar:css(".favorite-toggle.active").color,
      clipper:css('.tool-group[data-category="Clipper"] .tool-row>i').backgroundColor,
      cut:css('.tool-group[data-category="Export"] .tool-row>i').backgroundColor,
      idleHeight:css('.tool-group[data-category="Export"] .tool-row>i').height,
    };
  });
  expect(palette).toMatchObject({filterBackground:"rgb(255, 248, 227)",filterStar:"rgb(194, 129, 0)",activeStar:"rgb(194, 129, 0)",idleHeight:"16px"});
  expect(palette.clipper).not.toBe(palette.cut);
  await page.locator('.tool-group[data-category="Clipper"] .tool-row').click();
  expect(await page.locator('.tool-group[data-category="Clipper"] .tool-row>i').evaluate(el=>getComputedStyle(el).height)).toBe("24px");
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  const smartCutContrast=await page.evaluate(()=>{
    const ratio=(color:string)=>{
      const channels=color.match(/\d+/g)!.slice(0,3).map(value=>Number(value)/255).map(value=>value<=.04045?value/12.92:((value+.055)/1.055)**2.4);
      const luminance=.2126*channels[0]+.7152*channels[1]+.0722*channels[2];
      return 1.05/(luminance+.05);
    };
    return [".ac-card header p",".ac-right header p",".cut-summary",".cut-empty",".tl-status>b"].map(selector=>ratio(getComputedStyle(document.querySelector(selector)!).color));
  });
  for(const ratio of smartCutContrast)expect(ratio).toBeGreaterThan(4.5);
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

test("Turkish tool descriptions and editing controls stay localized",async({page})=>{
  await page.addInitScript(()=>localStorage.setItem("container-language","tr"));
  await mockDesktop(page);await openFixture(page);
  await expect(page.locator(".tool-settings-head p")).toHaveText("Kırpma, döndürme, çevirme ve boyutlandırmayı tek yerde yapar.");
  await expect(page.locator(".transform-controls header b")).toContainText(["KIRP","DÖNDÜR","ÇEVİR","ÇIKTI BOYUTU"]);
  await expect(page.locator(".crop-options button").first()).toHaveText("KAPALI");
  await expect(page.locator(".crop-options button").nth(1)).toHaveText("SERBEST");
  await expect(page.locator(".tool-group h4")).toContainText(["Dönüştürme","Dikey Klipler","Çözünürlük"]);
  await page.locator(".search").fill("Clipper");
  await page.locator(".tool-row").click();
  await expect(page.locator(".tool-settings-head p")).toHaveText("Yatay videoyu paylaşmaya hazır 9:16 düzene dönüştürür.");
  await expect(page.locator(".transform-options button").first()).toHaveText("BÖL");
  await expect(page.locator(".transform-options button").nth(3)).toHaveText("ORİJİNAL BOYUT");
  await expect(page.locator(".auto-camera em")).toHaveText("DENEYSEL");
  await page.locator(".search").fill("Upscale");
  await page.locator(".tool-row").click();
  await expect(page.locator(".tool-settings-head p")).toHaveText("Video çözünürlüğünü HD, 2K, 4K veya 8K'ya yükseltir.");
});

test("Turkish image transform labels preserve format choices",async({page})=>{
  await page.addInitScript(()=>localStorage.setItem("container-language","tr"));
  await mockDesktop(page,{fixture:{...sample,path:"C:\\fixtures\\sample.jpg",name:"sample.jpg",kind:"image",duration:0.04,width:140,height:140,fps:25,codec:"mjpeg"}});
  await openFixture(page);
  await expect(page.locator(".crop-options button").first()).toHaveText("ORİJİNAL");
  await expect(page.locator(".transform-options button").filter({hasText:"PNG · KAYIPSIZ"})).toHaveCount(1);
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
        await expect.poll(()=>page.locator(".tool-group").count()).toBeGreaterThan(4);
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
        if(theme==="light")expect(await page.locator(".transform-options button.active").first().evaluate(el=>{
          const probe=document.createElement("span");probe.style.color="var(--blue)";document.body.append(probe);
          const accent=getComputedStyle(probe).color;probe.remove();return getComputedStyle(el).color===accent;
        })).toBe(true);
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
        if(theme==="light"&&[1440,900].includes(size.width)){
          await page.getByRole("button",{name:language==="tr"?"TOPLU":"BATCH",exact:true}).click();
          await page.screenshot({path:`test-results/polish-batch-${language}-${size.width}.png`});
        }
        await page.getByRole("button",{name:language==="tr"?"ARAÇ KUTUSU":"TOOLBOX",exact:true}).click();
        if(theme==="light"&&[1440,900].includes(size.width)){
          await page.getByRole("button",{name:language==="tr"?"ses":"audio",exact:true}).click();
          await page.screenshot({path:`test-results/polish-audio-${language}-${size.width}.png`});
          await page.getByRole("button",{name:"video",exact:true}).click();
        }
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
    if(width<1900)expect(tabs!.x-summary!.x-summary!.width).toBeLessThanOrEqual(15);
    else expect(Math.abs(summary!.x+summary!.width/2-width/2)).toBeLessThan(2);
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
      await page.screenshot({path:"test-results/header-centered.png"});
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
  await expect(page.locator(".stage-trigger")).toHaveText("SOURCE HISTORY ▾");
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
    (window as any).__TEST_DROP__("C:\\fixtures\\history.cproj");
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
  await page.evaluate(()=>(window as any).__TEST_DROP__("C:\\fixtures\\history.cproj"));
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
  await expect(page.locator(".stage-trigger")).toHaveText("KAYNAK GEÇMİŞİ ▾");
  await expect(page.locator(".stage-heading strong")).toHaveText("Kaynak geçmişi");
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
    (window as any).__TEST_DROP__("C:\\fixtures\\invalid-history.cproj");
  });
  await expect(page.locator(".stage-trigger")).toHaveCount(1);
  await page.locator(".stage-trigger").click();
  await expect(page.locator(".stage-list button")).toHaveCount(1);
  await expect(page.locator(".stage-menu")).toContainText("No other sources yet.");
  await page.keyboard.press("Escape");
  await expect(page.locator(".settings")).toBeVisible();
  await expect(page.getByRole("button",{name:"▶ render transform",exact:true})).toBeEnabled();
});

test("changed render settings require rerender before continuation, including restored projects",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  const render=page.getByRole("button",{name:"▶ render transform",exact:true});
  const proceed=page.getByRole("button",{name:"Open as source",exact:true});
  await render.click();await expect(proceed).toBeEnabled();
  await page.getByRole("button",{name:"180°",exact:true}).click();
  await expect(proceed).toBeDisabled();
  await expect(proceed).toHaveAttribute("title",/render again/);
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).__SAVED_PROJECT__)).toBeTruthy();
  await page.evaluate(()=>(window as any).__TEST_DROP__("C:\\fixtures\\saved.cproj"));
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
  await page.getByRole("button",{name:"Open as source",exact:true}).click();
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
    await start.click();await expect(page.getByRole("button",{name:"Open in Toolbox",exact:true})).toBeEnabled();
    for(const width of [1440,1024]){
      await page.setViewportSize({width,height:900});
      const buttons=page.locator(".batch-row-actions button");
      const a=await buttons.nth(0).boundingBox(),b=await buttons.nth(1).boundingBox();
      expect(Math.abs(a!.y-b!.y)).toBeLessThan(1);
      expect(a!.x+a!.width).toBeLessThanOrEqual(b!.x);
      const actions=await page.locator(".batch-row-actions").boundingBox(),panel=await page.locator(".batch-list").boundingBox();
      expect(actions!.x+actions!.width).toBeLessThanOrEqual(panel!.x+panel!.width);
    }
    await page.screenshot({path:`test-results/batch-actions-${outcome.replaceAll(" ","-")}.png`});
    await page.evaluate(message=>{(window as any).__TEST_HANDLER__=(cmd:string)=>{if(cmd==="run_operation")throw new Error(message)}},outcome);
    await start.click();
    await expect(page.locator(".batch-items article strong")).toHaveText(outcome==="cancelled"?"cancelled":"failed");
    await expect(page.getByRole("button",{name:"Open in Toolbox",exact:true})).toHaveCount(0);
    await expect(page.locator(".batch-items article")).not.toContainText("output-1.mp4");
    await page.locator(".batch-items article summary").click();
    await expect(page.locator(".batch-items article pre")).toContainText(outcome);
  });
}
test("Source history reloads Cut Video filmstrip on every return",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.evaluate(()=>{const previous=(window as any).__TEST_HANDLER__;(window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="compute_video_filmstrip"?"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='200' height='40'%3E%3Crect width='200' height='40' fill='red'/%3E%3C/svg%3E":previous(cmd,args)});
  await page.getByPlaceholder("search tools...").fill("Cut Video");
  await page.locator(".tool-row").filter({has:page.getByText("Cut Video",{exact:true})}).click();
  await expect(page.locator(".filmstrip")).toBeVisible();
  await page.getByRole("button",{name:/render cut video/i}).click();
  await page.getByRole("button",{name:"Open as source",exact:true}).click();
  for(let i=0;i<3;i++){
    await selectStageNumber(page,1);
    await expect(page.locator(".filmstrip")).toBeVisible();
    await selectStageNumber(page,2);
  }
});

test("Continue editing alone creates stages; earlier settings and forward outputs survive",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","1");
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await expect(page.getByRole("button",{name:"Open as source",exact:true})).toBeEnabled();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","1");
  await page.locator(".stage-trigger").click();
  await expect(page.locator(".stage-list button")).toHaveCount(1);
  await page.keyboard.press("Escape");
  await page.getByRole("button",{name:"Open as source",exact:true}).click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","2");
  await expect(page.locator(".topbar .stage-trigger")).toHaveCount(1);
  await expect(page.locator(".topbar .slash")).toHaveCount(0);
  const brandBounds=await page.locator(".brand").boundingBox();
  const backBounds=await page.locator(".stage-trigger").boundingBox();
  expect(backBounds!.x-brandBounds!.x-brandBounds!.width).toBeLessThanOrEqual(15);
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await selectStageNumber(page,1);
  await page.getByRole("button",{name:"Open as source",exact:true}).click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","3");
  await page.locator(".stage-trigger").click();
  await expect(page.locator(".stage-list button")).toHaveCount(3);
  await page.locator(".stage-close").click();
  await selectStageNumber(page,2);
  await page.getByRole("button",{name:"show in folder",exact:true}).click();
  const reveal=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((x:any)=>x.cmd.includes("reveal")).at(-1));
  expect(JSON.stringify(reveal)).toContain("output-2.mp4");
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).__SAVED_PROJECT__)).toBeTruthy();
  const saved=await page.evaluate(()=>JSON.parse((window as any).__SAVED_PROJECT__));
  expect(saved.stageHistory.entries).toHaveLength(3);
  // Open the saved project via the project-aware drop route.
  await page.evaluate(()=>(window as any).__TEST_DROP__("C:\\fixtures\\sample.cproj"));
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","2");
  await selectStageNumber(page,1);
  await expect(page.getByRole("button",{name:"Open as source",exact:true})).toBeVisible();
});

test("editing an ancestor creates a branch without erasing the original",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await page.getByRole("button",{name:"Open as source",exact:true}).click();
  await selectStageNumber(page,1);
  await page.getByRole("button",{name:"180°",exact:true}).click();
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await page.getByRole("button",{name:"Open as source",exact:true}).click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","4");
  await selectStageNumber(page,1);
  await expect(page.getByRole("button",{name:"0°",exact:true})).toHaveClass(/active/);
  await selectStageNumber(page,3);
  await expect(page.locator("header").filter({has:page.locator("b",{hasText:/^ROTATE$/})}).locator("small")).toHaveText("180°");
});

test("missing historical source and failed continuation leave current stage intact",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await page.getByRole("button",{name:"Open as source",exact:true}).click();
  await page.evaluate(()=>{
    const original=(window as any).__TEST_HANDLER__;
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="project_media_available"?false:original(cmd,args);
  });
  await page.locator(".stage-trigger").click();
  await page.locator(".stage-list button").first().click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","2");
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="run_operation"?{output:"C:\\fixtures\\broken.mp4",elapsed:.1}:undefined});
  await page.getByRole("button",{name:"▶ render transform",exact:true}).click();
  await page.getByRole("button",{name:"Open as source",exact:true}).click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","2");
  await page.locator(".error-box summary").click();
  await expect(page.locator(".error-box pre")).toContainText("Invalid media fixture");
});

test("SmartCut stages restore cuts and keep history compact in dark and light",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  await page.getByRole("button",{name:"DETECT SILENCES",exact:true}).click();
  await page.getByRole("button",{name:"EXPORT MP4",exact:true}).click();
  await page.getByRole("button",{name:"Open in editor",exact:true}).click();
  await expect(page.locator(".stage-trigger")).toHaveAttribute("data-current-stage","2");
  await selectStageNumber(page,1);
  await expect(page.locator(".ac-layout")).toBeVisible();
  await expect(page.getByRole("button",{name:"Open in editor",exact:true})).toBeVisible();
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
  await page.getByRole("button",{name:"Open in Toolbox",exact:true}).click();
  await selectStageNumber(page,1);
  await expect(page.locator(".batch-items article strong")).toHaveText("complete");
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").length)).toBe(1);
});

test("color output does not silently replace the source before Continue editing",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByPlaceholder("search tools...").fill("Color Adjustment");
  await page.getByText("Color Adjustment",{exact:true}).last().click();
  await page.getByRole("slider",{name:"Brightness",exact:true}).fill("15");
  await page.getByRole("button",{name:/render color adjustment/i}).click();
  await expect(page.getByRole("button",{name:"Open as source",exact:true})).toBeEnabled();
  const inputs=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="probe_media").map((call:any)=>call.args.path));
  expect(inputs).toEqual([sample.path]);
  await page.getByRole("button",{name:"Open as source",exact:true}).click();
  await selectStageNumber(page,1);
  await expect(page.locator(".tool-settings-head h2")).toHaveText("Color Adjustment");
  await expect(page.getByRole("slider",{name:"Brightness",exact:true})).toHaveValue("15");
});

test("text layers remain editable when returning from a continued output",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  const font=readFileSync(new URL("../../../node_modules/@fontsource-variable/geist/files/geist-latin-wght-normal.woff2",import.meta.url)).toString("base64");
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
  await page.getByRole("button",{name:"Open as source",exact:true}).click();
  await selectStageNumber(page,1);
  await expect(page.getByRole("textbox",{name:"Text",exact:true})).toHaveValue("Keep this editable");
});

for(const family of ["Impact","Arial","Georgia"])for(const width of [1080,1920])test(`Installed ${family} preview preserves glyphs at ${width}px`,async({page})=>{
  test.skip(process.platform!=="win32","Requires installed Windows fonts");
  const fontPath=`C:/Windows/Fonts/${family.toLowerCase()}.ttf`;
  const font=readFileSync(fontPath).toString("base64");
  await mockDesktop(page,{fixture:{...sample,width,height:width===1080?1920:1080}});await openFixture(page);await stageMocks(page);
  await page.evaluate(({font,family,fontPath})=>{
    const previous=(window as any).__TEST_HANDLER__;
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
      if(cmd==="list_system_fonts")return [{name:family,path:fontPath}];
      if(cmd==="font_preview_data")return `data:font/ttf;base64,${font}`;
      return previous(cmd,args);
    };
  },{font,family,fontPath});
  await page.getByPlaceholder("search tools...").fill("Text");
  await page.locator(".tool-row").filter({has:page.getByText("Text",{exact:true})}).click();
  await page.getByRole("button",{name:"＋ Add text",exact:true}).click();
  await page.getByRole("textbox",{name:"Text",exact:true}).fill("clippingfornoreason");
  const result=await page.locator(".text-preview-canvas").evaluate((canvas:HTMLCanvasElement,family)=>{
    const ctx=canvas.getContext("2d")!;
    const actual=ctx.getImageData(0,0,canvas.width,canvas.height).data;
    const reference=document.createElement("canvas");reference.width=canvas.width;reference.height=canvas.height;
    const ref=reference.getContext("2d")!;ref.font=ctx.font;ref.textAlign="center";ref.textBaseline="middle";ref.fillStyle="#ffffff";
    ref.fillText("clippingfornoreason",canvas.width/2,canvas.height/2);
    const expected=ref.getImageData(0,0,canvas.width,canvas.height).data;
    const samePixels=actual.every((value,index)=>value===expected[index]);
    const font=ctx.font;
    const measure=(family:string)=>{ctx.font=`400 64px ${family}`;return ctx.measureText("clippingfornoreason").width};
    const alias=measure(font.slice(font.indexOf("px")+3));
    const native=measure(JSON.stringify(family));
    const clean=document.createElement("canvas").getContext("2d")!;
    clean.font=`400 64px ${JSON.stringify(family)}`;
    return {font,alias,native,samePixels,clean:clean.measureText("clippingfornoreason").width};
  },family);
  expect(result.alias).toBeCloseTo(result.native,1);
  expect(result.alias).toBeCloseTo(result.clean,1);
  expect(result.samePixels).toBe(true);
});

test("Text presets persist and restore selected layer appearance",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  const font=readFileSync(new URL("../../../node_modules/@fontsource-variable/geist/files/geist-latin-wght-normal.woff2",import.meta.url)).toString("base64");
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
  await page.getByRole("textbox",{name:"Text",exact:true}).fill("Saved caption");
  await page.getByRole("slider",{name:"Font size",exact:false}).fill("92");
  await page.getByRole("combobox",{name:"Alignment",exact:true}).selectOption("right");
  await page.getByRole("textbox",{name:"Preset name",exact:true}).fill("My caption");
  await page.getByRole("button",{name:"Save selected text",exact:true}).click();
  const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem("container-text-presets-v1")!)[0]);
  expect(saved.layer.size).toBe(92);expect(saved.layer.align).toBe("right");expect(saved.sourceWidth).toBe(1920);
  const originalBox=await page.locator(".preview-text.active").boundingBox();
  await page.getByRole("textbox",{name:"Text",exact:true}).fill("clippingfornoreason");
  await page.getByRole("slider",{name:"Font size",exact:false}).fill("200");
  await expect(page.locator(".preview-text.active > span")).toHaveText("clippingfornoreason");
  await page.getByRole("textbox",{name:"Text",exact:true}).fill("Other text");
  await page.getByRole("button",{name:"Add from preset",exact:true}).click();
  await expect(page.locator(".text-tab")).toHaveCount(2);
  await expect(page.getByRole("textbox",{name:"Text",exact:true})).toHaveValue("Saved caption");
  await expect(page.getByRole("combobox",{name:"Alignment",exact:true})).toHaveValue("right");
  await expect(page.getByRole("slider",{name:"Font size"})).toHaveValue("92");
  const restoredBox=await page.locator(".preview-text.active").boundingBox();
  expect(restoredBox!.width).toBeCloseTo(originalBox!.width,0);
  expect(restoredBox!.height).toBeCloseTo(originalBox!.height,0);
  await page.reload();await openFixture(page);
  await stageMocks(page);
  await page.evaluate(font=>{const previous=(window as any).__TEST_HANDLER__;(window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="list_system_fonts"?[{name:"Arial",path:"C:\\fixtures\\font.ttf"}]:cmd==="font_preview_data"?`data:font/woff2;base64,${font}`:previous(cmd,args)},font);
  await page.getByPlaceholder("search tools...").fill("Text");
  await page.locator(".tool-row").filter({has:page.getByText("Text",{exact:true})}).click();
  await expect(page.getByRole("combobox",{name:"Saved preset",exact:true}).locator("option")).toHaveText(["Choose a preset","My caption"]);
  await page.getByRole("combobox",{name:"Saved preset",exact:true}).selectOption(saved.id);
  await stageMocks(page);
  await page.evaluate(font=>{const previous=(window as any).__TEST_HANDLER__;(window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="list_system_fonts"?[{name:"Arial",path:"C:\\fixtures\\font.ttf"}]:cmd==="font_preview_data"?`data:font/woff2;base64,${font}`:previous(cmd,args)},font);
  await page.getByRole("button",{name:"Add from preset",exact:true}).click();
  await page.getByRole("slider",{name:"Font size"}).fill("120");
  await page.getByText("Edit selected preset",{exact:true}).click();
  await page.getByRole("button",{name:"Update from selected text",exact:true}).click();
  await page.getByRole("textbox",{name:"Preset name",exact:true}).fill("Renamed caption");
  await page.getByRole("button",{name:"Rename",exact:true}).click();
  const updated=await page.evaluate(()=>JSON.parse(localStorage.getItem("container-text-presets-v1")!));
  expect(updated).toHaveLength(1);expect(updated[0].id).toBe(saved.id);expect(updated[0].name).toBe("Renamed caption");expect(updated[0].layer.size).toBe(120);
  await page.screenshot({path:"test-results/text-preset-management.png"});
  await page.getByRole("button",{name:"Delete preset",exact:true}).click();
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem("container-text-presets-v1")!))).toEqual([]);
});

for(const width of [960,3840])test(`Text preset scales saved pixel dimensions to ${width}px media`,async({page})=>{
  const font=readFileSync(new URL("../../../node_modules/@fontsource-variable/geist/files/geist-latin-wght-normal.woff2",import.meta.url)).toString("base64");
  await page.addInitScript(()=>localStorage.setItem("container-text-presets-v1",JSON.stringify([{
    id:"scaled",name:"Scaled caption",sourceWidth:1920,
    layer:{id:1,text:"Saved caption",x:30,y:70,size:92,wrap_width:12,color:"#ffffff",opacity:80,align:"left",fontName:"Arial",font_path:"C:\\fixtures\\font.ttf",outline:2,outline_color:"#000000",shadow:4,shadow_color:"#000000",background:true,background_color:"#000000",background_opacity:65,background_padding:12}
  }])));
  await mockDesktop(page,{fixture:{...sample,width,height:width*9/16}});await openFixture(page);await stageMocks(page);
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
  await page.getByRole("combobox",{name:"Saved preset",exact:true}).selectOption("scaled");
  await page.getByRole("button",{name:"Add from preset",exact:true}).click();
  await expect(page.getByRole("slider",{name:"Font size"})).toHaveValue(String(92*width/1920));
  await page.getByRole("textbox",{name:"Preset name",exact:true}).fill("Resaved");
  await page.getByRole("button",{name:"Save selected text",exact:true}).click();
  const result=await page.evaluate(()=>JSON.parse(localStorage.getItem("container-text-presets-v1")!).at(-1));
  expect(result.sourceWidth).toBe(width);
  expect(result.layer).toMatchObject({size:92*width/1920,outline:2*width/1920,shadow:4*width/1920,background_padding:12*width/1920,x:30,y:70,wrap_width:12,opacity:80});
});

test("Text handles resize smoothly and pasted emoji reaches the export raster",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  const font=readFileSync(new URL("../../../node_modules/@fontsource-variable/geist/files/geist-latin-wght-normal.woff2",import.meta.url)).toString("base64");
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
  // Color emoji font strikes and glyph hinting vary between Windows and Linux.
  // Allow their small raster-size rounding while still catching anchor jumps.
  expect(Math.abs(after!.x-before!.x)).toBeLessThanOrEqual(4);
  const left=await layer.locator(".text-size-handle.left").boundingBox();
  await page.mouse.move(left!.x+left!.width/2,left!.y+left!.height/2);
  await page.mouse.down();await page.mouse.move(left!.x+left!.width/2-24,left!.y+left!.height/2,{steps:4});await page.mouse.up();
  const fromLeft=await layer.boundingBox();
  expect(fromLeft!.width-after!.width).toBeGreaterThan(16);
  expect(Math.abs(fromLeft!.x+fromLeft!.width-after!.x-after!.width)).toBeLessThanOrEqual(4);
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
    const font=readFileSync(new URL("../../../node_modules/@fontsource-variable/geist/files/geist-latin-wght-normal.woff2",import.meta.url)).toString("base64");
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
    const logo=readFileSync(new URL("../../../src/public/logo-dark.png",import.meta.url));
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
  await expect(page.locator(".tool-settings-head h2")).toHaveText("GIF Maker");
  await page.evaluate(() => (window as any).__TEST_DROP__("C:\\fixtures\\broken.mp4"));
  await page.locator(".error-box summary").click();
  await expect(page.locator(".error-box pre")).toContainText("Invalid media fixture");
  await expect(page.locator(".tool-settings-head h2")).toHaveText("GIF Maker");
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

test("Boxed Social Tag stays centered while plain positions move across the camera", async ({ page }) => {
  await mockDesktop(page);
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper", { exact: true }).last().click();
  await page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true}).click();
  await page.getByText("Social Tag", { exact: true }).click();
  await page.getByPlaceholder("kanaladi").fill("Example");
  const style = page.getByRole("combobox", { name: "Style" });
  const position = page.getByRole("combobox", { name: "Position" });
  const tag = page.locator(".clipper-social-tag");
  const x = async () => (await tag.boundingBox())?.x ?? -1;

  await expect(position).toHaveCount(0);
  await expect(tag).toBeVisible();
  const outputBox = await page.locator(".clipper-output-canvas").boundingBox();
  const camera = outputBox&&{...outputBox,height:outputBox.height*.3};
  expect(camera).not.toBeNull();
  const centeredBounds = await tag.boundingBox();
  expect(centeredBounds).not.toBeNull();
  expect(Math.abs(centeredBounds!.x + centeredBounds!.width / 2 - camera!.x - camera!.width / 2)).toBeLessThan(3);
  expect(Math.abs(centeredBounds!.y + centeredBounds!.height - camera!.y - camera!.height)).toBeLessThan(3);

  await style.selectOption("plain");
  await expect(position).toHaveValue("center");
  const plainCenter = await x();
  const plainBounds = await tag.boundingBox();
  expect(Math.abs(plainBounds!.x + plainBounds!.width / 2 - camera!.x - camera!.width / 2)).toBeLessThan(3);
  expect(Math.abs(plainBounds!.y + plainBounds!.height / 2 - camera!.y - camera!.height)).toBeLessThan(3);
  await position.selectOption("left");
  const plainLeft = await x();
  expect(Math.abs(plainLeft - camera!.x)).toBeLessThan(3);
  await position.selectOption("right");
  const plainRight = await x();
  const plainRightBounds = await tag.boundingBox();
  expect(Math.abs(plainRightBounds!.x + plainRightBounds!.width - camera!.x - camera!.width)).toBeLessThan(3);
  expect(plainLeft).toBeLessThan(plainCenter);
  expect(plainCenter).toBeLessThan(plainRight);
  if (process.env.UI_AUDIT_SCREENSHOTS) await page.screenshot({ path: "test-results/social-tag-plain-right.png" });
  await style.selectOption("boxed");
  await expect(position).toHaveCount(0);
  const boxedAgain = await tag.boundingBox();
  expect(Math.abs(boxedAgain!.x + boxedAgain!.width / 2 - camera!.x - camera!.width / 2)).toBeLessThan(3);
});

test("Kick.com banner uses the supplied art and Gotham font at the camera seam",async({page})=>{
  await mockDesktop(page);
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper",{exact:true}).last().click();
  await page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true}).click();
  await page.getByText("Social Tag",{exact:true}).click();
  await page.getByPlaceholder("kanaladi").fill("adinross");
  const style=page.getByRole("combobox",{name:"Style"});
  const platform=page.getByRole("combobox",{name:"Platform"});
  await platform.selectOption("twitch");
  await style.selectOption("kick_banner");
  await expect(platform).toHaveValue("kick");
  await expect(page.getByRole("combobox",{name:"Position"})).toHaveCount(0);
  const size=page.locator(".clipper-watermark-controls input[type=range]").first();
  await expect(size).toHaveValue("54");
  await size.fill("36");
  const banner=page.locator(".clipper-social-banner");
  await expect(banner).toBeVisible();
  await expect(banner.locator(".clipper-social-banner-name")).toHaveText("ADINROSS");
  await expect.poll(()=>banner.locator("img").evaluateAll(images=>images.map(image=>(image as HTMLImageElement).naturalWidth))).toEqual([1080,1080]);
  await expect.poll(()=>page.evaluate(()=>document.fonts.check('32px "Gotham XNarrow Black"'))).toBe(true);
  const outputBox=await page.locator(".clipper-output-canvas").boundingBox();
  const camera=outputBox&&{...outputBox,height:outputBox.height*.3};
  const small=await banner.boundingBox();
  expect(camera).not.toBeNull();expect(small).not.toBeNull();
  expect(Math.abs(small!.x-camera!.x)).toBeLessThan(3);
  expect(Math.abs(small!.width-camera!.width)).toBeLessThan(3);
  expect(Math.abs(small!.y+small!.height-camera!.y-camera!.height)).toBeLessThan(3);
  const name=await banner.locator(".clipper-social-banner-name").boundingBox();
  expect(name).not.toBeNull();
  expect(Math.abs(name!.y+name!.height/2-small!.y-small!.height*(1-35*1.18/(113*1.035)))).toBeLessThan(2);
  const smallArt=await banner.locator(".clipper-social-banner-logo-art").boundingBox();
  expect(smallArt).not.toBeNull();
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/kick-banner-preview-36.png"});
  await size.fill("54");
  const large=await banner.boundingBox();
  expect(large).not.toBeNull();
  expect(large!.height).toBeGreaterThan(small!.height*1.4);
  expect(Math.abs(large!.y+large!.height-camera!.y-camera!.height)).toBeLessThan(3);
  const largeName=await banner.locator(".clipper-social-banner-name").boundingBox();
  expect(largeName).not.toBeNull();
  expect(Math.abs(largeName!.y+largeName!.height/2-large!.y-large!.height*(1-35*1.18/(113*1.035)))).toBeLessThan(2);
  const referenceFontSize=await banner.locator(".clipper-social-banner-name").evaluate(el=>getComputedStyle(el).fontSize);
  const referencePrefix=await banner.locator(".clipper-social-banner-prefix").boundingBox();
  await page.getByPlaceholder("kanaladi").fill("batuhanfurkan5");
  await expect(banner.locator(".clipper-social-banner-name")).toHaveCSS("font-size",referenceFontSize);
  const shiftedPrefix=await banner.locator(".clipper-social-banner-prefix").boundingBox();
  expect(shiftedPrefix!.x).toBeLessThan(referencePrefix!.x-5);
  const safeName=await banner.locator(".clipper-social-banner-name").boundingBox();
  expect(safeName!.x+safeName!.width).toBeLessThanOrEqual(large!.x+large!.width*.8+1);
  await page.getByPlaceholder("kanaladi").fill("adinross");
  const art=await banner.locator(".clipper-social-banner-logo-art").boundingBox();
  const prefix=await banner.locator(".clipper-social-banner-prefix-art").boundingBox();
  const background=await banner.locator(".clipper-social-banner-bg").boundingBox();
  expect(art).not.toBeNull();expect(prefix).not.toBeNull();expect(background).not.toBeNull();
  expect(art!.width/smallArt!.width).toBeGreaterThan(1.4);
  expect(art!.width/smallArt!.width).toBeLessThan(1.6);
  expect(art!.width).toBeGreaterThan(large!.width);
  expect(art!.height).toBeGreaterThan(large!.height*9);
  expect(background!.height).toBeGreaterThan(large!.height*.6);
  expect(background!.height).toBeLessThan(large!.height);
  expect(prefix!.height).toBeGreaterThan(large!.height*9);
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/kick-banner-preview.png"});
  await page.getByPlaceholder("kanaladi").fill("averylongkickusername1234567890");
  const longBanner=await banner.boundingBox();
  const longName=await banner.locator(".clipper-social-banner-name").boundingBox();
  expect(longBanner).not.toBeNull();expect(longName).not.toBeNull();
  expect(longName!.x+longName!.width).toBeLessThanOrEqual(longBanner!.x+longBanner!.width*.8+1);
  const bannerDistance=page.getByRole("slider",{name:/Move banner away from camera/i});
  const defaultPosition=await banner.boundingBox();
  await bannerDistance.fill("100");
  const movedPosition=await banner.boundingBox();
  expect(movedPosition!.y-defaultPosition!.y).toBeGreaterThan(defaultPosition!.height*.9);
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/kick-banner-long-safe.png"});
  await page.getByPlaceholder("kanaladi").fill("adinross");
  for(const layout of ["SQUARES","FREECAM"]){
    await page.getByRole("button",{name:layout,exact:true}).click();
    const distance=page.getByRole("slider",{name:/Move banner away from camera/i});
    await expect(distance).toBeVisible();
    await distance.fill("0");
    const atCamera=await banner.boundingBox();
    await distance.fill("50");
    const awayFromCamera=await banner.boundingBox();
    expect(awayFromCamera!.y-atCamera!.y,layout).toBeGreaterThan(5);
  }
});

test("Blur foreground framing and Social Tags work in Original, Blur and Fill",async({page})=>{
  await mockDesktop(page);
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper",{exact:true}).last().click();
  await page.getByText("Social Tag",{exact:true}).click();
  await page.getByPlaceholder("kanaladi").fill("batuhanfurkan5");
  await page.getByRole("combobox",{name:"Style"}).selectOption("kick_banner");
  if(process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/kick-banner-batuhan-safe.png"});
  for(const layout of ["ORIGINAL SIZE","BLUR","FILL"]){
    await page.getByRole("button",{name:layout,exact:true}).click();
    const banner=page.locator(".clipper-social-banner");
    await expect(banner).toBeVisible();
    const bounds=await banner.boundingBox();
    const name=await banner.locator(".clipper-social-banner-name").boundingBox();
    expect(bounds).not.toBeNull();expect(name).not.toBeNull();
    expect(name!.x+name!.width).toBeLessThanOrEqual(bounds!.x+bounds!.width*.8+1);
    expect(bounds!.y).toBeGreaterThan(0);
    const zoom=page.getByRole("slider",{name:/Video zoom/i});
    if(layout==="FILL")await expect(zoom).toBeVisible();else await expect(zoom).toHaveCount(0);
    await expect(page.getByRole("slider",{name:/Horizontal framing|Vertical framing/i})).toHaveCount(0);
    await expect(page.getByRole("button",{name:"RESET ZOOM"})).toBeVisible();
    const distance=page.getByRole("slider",{name:/Move banner away from camera/i});
    await expect(distance).toBeVisible();
    await distance.fill("0");
    const initialBanner=await banner.boundingBox();
    await distance.fill("30");
    await expect(distance).toHaveValue("30");
    const movedBanner=await banner.boundingBox();
    expect(movedBanner!.y).toBeGreaterThan(initialBanner!.y+3);
    await page.getByRole("button",{name:"Reset tag distance"}).click();
    await expect(distance).toHaveValue("0");
    if(layout==="BLUR"){
      await expect(page.locator(".blur-backdrop-canvas")).toBeVisible();
      await expect(page.locator(".video-canvas video")).toHaveCount(1);
      const backing=await page.locator(".blur-backdrop-canvas").evaluate((node)=>({width:(node as HTMLCanvasElement).width,height:(node as HTMLCanvasElement).height}));
      expect(backing.width).toBeGreaterThan(0);expect(backing.width).toBeLessThanOrEqual(480);expect(backing.height).toBeGreaterThan(0);
      const backgroundBlur=page.getByRole("slider",{name:/Background blur/i});
      await backgroundBlur.fill("12");
      await expect(backgroundBlur).toHaveValue("12");
      const before=await page.locator(".video-canvas video").last().locator("xpath=..").boundingBox();
      await expect(page.locator(".fill-pan-layer")).toBeVisible();
      await page.locator(".fill-pan-layer").hover();
      for(let i=0;i<8;i++)await page.mouse.wheel(0,-100);
      const after=await page.locator(".video-canvas video").last().locator("xpath=..").boundingBox();
      expect(Math.abs(after!.height-before!.height)).toBeLessThan(1);
      await page.getByRole("button",{name:"RESET ZOOM"}).click();
    }else{
      if(layout==="FILL")await zoom.fill("225");
      else{await page.locator(".fill-pan-layer").hover();for(let i=0;i<13;i++)await page.mouse.wheel(0,-100)}
      const foreground=await page.locator(".video-canvas video").last().boundingBox();
      const frame=await page.locator(".video-canvas video").last().locator("xpath=..").boundingBox();
      expect(foreground!.width).toBeGreaterThan(frame!.width*2.2);
      expect(foreground!.x).toBeLessThanOrEqual(frame!.x+1);
      expect(foreground!.x+foreground!.width).toBeGreaterThanOrEqual(frame!.x+frame!.width-1);
      const pan=await page.locator(".fill-pan-layer").boundingBox();
      await page.mouse.move(pan!.x+pan!.width/2,pan!.y+pan!.height/2);
      await page.mouse.down();
      await expect(page.locator(".clipper-center-guides .vertical")).toBeVisible();
      await expect(page.locator(".clipper-center-guides .horizontal")).toBeVisible();
      await page.mouse.move(pan!.x+pan!.width/2+35,pan!.y+pan!.height/2,{steps:4});
      await expect(page.locator(".clipper-center-guides .vertical")).toHaveCount(0);
      await page.mouse.move(pan!.x+pan!.width/2,pan!.y+pan!.height/2,{steps:4});
      await expect(page.locator(".clipper-center-guides .vertical")).toBeVisible();
      await page.mouse.up();
      await expect(page.locator(".clipper-center-guides")).toHaveCount(0);
      if(layout==="FILL"){
        await expect(page.getByRole("slider",{name:/Horizontal framing/i})).toHaveCount(0);
        await expect(page.getByRole("slider",{name:/Vertical framing/i})).toHaveCount(0);
      }
      await page.getByRole("button",{name:"RESET ZOOM"}).click();
    }
  }
  await page.getByRole("button",{name:"FREECAM",exact:true}).click();
  await page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true}).click();
  await expect(page.getByRole("slider",{name:/Video zoom/i})).toHaveCount(0);
  await page.getByRole("slider",{name:"Camera position X"}).fill("80");
  await page.getByRole("slider",{name:"Camera position X"}).press("Control+z");
  await expect(page.getByRole("slider",{name:"Camera position X"})).toHaveValue("50");
  await page.getByRole("slider",{name:"Camera position X"}).fill("80");
  await page.getByRole("slider",{name:"Camera position Y"}).fill("60");
  await page.getByRole("slider",{name:"Camera size"}).fill("30");
  await page.getByRole("button",{name:"Reset camera X"}).click();
  await expect(page.getByRole("slider",{name:"Camera position X"})).toHaveValue("50");
  await expect(page.getByRole("slider",{name:"Camera position Y"})).toHaveValue("60");
  await page.getByRole("button",{name:"RESET CAMERA",exact:true}).click();
  await expect(page.getByRole("slider",{name:"Camera position X"})).toHaveValue("50");
  await expect(page.getByRole("slider",{name:"Camera position Y"})).toHaveValue("2");
  await expect(page.getByRole("slider",{name:"Camera size"})).toHaveValue("77");
  const freecamBanner=page.locator(".clipper-social-banner");
  const freecamLogo=freecamBanner.locator(".clipper-social-banner-logo");
  const freecamBounds=await freecamBanner.boundingBox();
  const logoBounds=await freecamLogo.boundingBox();
  expect(logoBounds!.x).toBeGreaterThanOrEqual(freecamBounds!.x-.5);
  await page.getByRole("button",{name:"SPLIT",exact:true}).click();
  await expect(page.getByRole("slider",{name:/Video zoom/i})).toHaveCount(0);
  await page.getByRole("button",{name:"SQUARES",exact:true}).click();
  await expect(page.getByRole("slider",{name:/Video zoom/i})).toHaveCount(0);
  await page.getByRole("button",{name:"BLUR",exact:true}).click();
  await page.locator(".fill-pan-layer").hover();
  await page.mouse.wheel(0,-100);
  await page.getByRole("button",{name:"defaults"}).click();
  await expect(page.getByRole("button",{name:"BLUR",exact:true})).toHaveClass(/active/);
  await expect(page.getByRole("slider",{name:/Video zoom/i})).toHaveCount(0);
});

test("all Social Tag styles move in every Clipper layout, reset, and send Split distance to export",async({page})=>{
  await mockDesktop(page);
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper",{exact:true}).last().click();
  await page.getByText("Social Tag",{exact:true}).click();
  await page.getByPlaceholder("kanaladi").fill("batuhanfurkan5");
  const style=page.getByRole("combobox",{name:"Style"});
  for(const tagStyle of ["boxed","plain"]){
    await style.selectOption(tagStyle);
    for(const layout of ["SPLIT","SQUARES","FREECAM","ORIGINAL SIZE","BLUR","FILL"]){
      await page.getByRole("button",{name:layout,exact:true}).click();
      if(["SPLIT","SQUARES","FREECAM"].includes(layout))await page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true}).click();
      const distance=page.getByRole("slider",{name:"Move tag away from camera"});
      await distance.fill("0");
      const tag=page.locator(".clipper-social-tag");
      const before=await tag.boundingBox();
      await distance.fill("50");
      const after=await tag.boundingBox();
      expect(after!.y-before!.y,`${tagStyle}/${layout}`).toBeGreaterThan(2);
      await page.getByRole("button",{name:"Reset tag distance"}).click();
      await expect(distance).toHaveValue("0");
    }
  }
  await style.selectOption("kick_banner");
  await page.getByRole("button",{name:"SQUARES",exact:true}).click();
  const distance=page.getByRole("slider",{name:"Move banner away from camera"});
  await distance.fill("35");
  await page.getByRole("button",{name:"SPLIT",exact:true}).click();
  await expect(distance).toHaveValue("35");
  const advanced=page.locator(".clipper-advanced").first();
  const social=page.locator(".clipper-watermark-controls").last();
  const advancedBox=await advanced.boundingBox(),socialBox=await social.boundingBox();
  expect(advancedBox!.y).toBeGreaterThanOrEqual(socialBox!.y+socialBox!.height);
  await stageMocks(page);
  await page.getByRole("button",{name:"▶ render clipper",exact:true}).click();
  await expect(page.locator(".job-head p")).toHaveText("complete");
  const request=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").at(-1).args.request);
  expect(request.params.vertical_layout).toBe("split");
  expect(request.params.social_tag_seam_offset).toBe("35");
});

test("Clipper layout order and source view reset when opening a project or new video",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper",{exact:true}).last().click();
  const layouts=await page.locator(".transform-options.three button").allTextContents();
  expect(layouts.map(label=>label.trim())).toEqual(["SPLIT","SQUARES","FREECAM","ORIGINAL SIZE","BLUR","FILL"]);
  const source=page.getByRole("button",{name:"SOURCE REGIONS",exact:true});
  await expect(source).toHaveAttribute("aria-pressed","true");
  await page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true}).click();
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
    if(cmd==="write_project"){(window as any).__SAVED_PROJECT__=args.contents;return null}
    if(cmd==="plugin:dialog|open")return "C:\\fixtures\\sample.cproj";
    if(cmd==="read_project")return (window as any).__SAVED_PROJECT__;
    return undefined;
  }});
  await page.getByRole("button",{name:"SAVE PROJECT"}).click();
  await expect.poll(()=>page.evaluate(()=>Boolean((window as any).__SAVED_PROJECT__))).toBe(true);
  await page.getByRole("button",{name:"close",exact:true}).click();
  await page.getByRole("button",{name:"OPEN PROJECT"}).click();
  await expect(source).toHaveAttribute("aria-pressed","true");
  await expect(page.locator(".region-a")).toBeVisible();
  await page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true}).click();
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=null;(window as any).__TEST_DROP__("C:\\fixtures\\second.mp4")});
  await expect(page.locator(".tool-settings-head h2")).toHaveText("Transform");
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper",{exact:true}).last().click();
  await expect(source).toHaveAttribute("aria-pressed","true");
});

test("Clipper output preview separates source editing and opens the last render in the default player",async({page})=>{
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper",{exact:true}).last().click();
  await expect(page.getByRole("button",{name:"SOURCE REGIONS",exact:true})).toHaveAttribute("aria-pressed","true");
  await page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true}).click();
  await expect(page.locator(".clipper-output-canvas")).toBeVisible();
  await expect(page.locator(".transform-source-box")).toHaveCount(0);
  await page.getByText("Social Tag",{exact:true}).click();
  await page.getByPlaceholder("kanaladi").fill("batuhanfurkan5");
  await page.getByRole("combobox",{name:"Style"}).selectOption("kick_banner");
  const banner=page.locator(".clipper-social-banner"),distance=page.getByRole("slider",{name:"Move banner away from camera"});
  const box=(await page.locator(".clipper-output-canvas").boundingBox())!;
  const before=(await banner.boundingBox())!;
  await distance.fill("16");
  const after=(await banner.boundingBox())!;
  expect(after.y-before.y).toBeCloseTo(box.height*.22*.16,0);
  await page.getByRole("button",{name:"SOURCE REGIONS",exact:true}).click();
  await expect(page.locator(".region-a")).toBeVisible();
  await expect(banner).toHaveCount(0);
  const region=(await page.locator(".region-a").boundingBox())!;
  await page.mouse.move(region.x+region.width/2,region.y+region.height/2);
  await page.mouse.down();await page.mouse.move(region.x+region.width/2-15,region.y+region.height/2+20);await page.mouse.up();
  await page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true}).click();
  await expect(banner).toBeVisible();await expect(distance).toHaveValue("16");
  await page.getByRole("button",{name:"▶ render clipper",exact:true}).click();
  const play=page.getByRole("button",{name:"▶ play",exact:true});
  await expect(play).toBeVisible();await play.click();
  const opened=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="plugin:opener|open_path").at(-1));
  expect(opened.args.path).toContain("output-1.mp4");
  await distance.fill("0");await play.click();
  await expect(page.getByText("Playing the last render; the new settings have not been rendered yet.",{exact:true})).toBeVisible();
});

test("Process actions align with status while frame metrics remain centered",async({page})=>{
  await page.setViewportSize({width:1600,height:900});
  await mockDesktop(page);await openFixture(page);await stageMocks(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper",{exact:true}).last().click();
  await page.evaluate(()=>{
    const previous=(window as any).__TEST_HANDLER__;
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="run_operation"?new Promise(resolve=>{(window as any).__FINISH_TEST_JOB__=()=>resolve({output:"C:\\fixtures\\aligned-output.mp4",elapsed:.1})}):previous?.(cmd,args);
  });
  const checkAlignment=async(buttonName:string)=>{
    const head=(await page.locator(".job-head").boundingBox())!;
    const status=(await page.locator(".job-status-line").boundingBox())!;
    const stats=(await page.locator(".job-stats").boundingBox())!;
    const action=(await page.getByRole("button",{name:buttonName,exact:true}).boundingBox())!;
    expect(Math.abs(stats.x+stats.width/2-head.x-head.width/2)).toBeLessThan(24);
    expect(Math.abs(action.y+action.height/2-status.y-status.height/2)).toBeLessThan(14);
  };
  await page.getByRole("button",{name:"▶ render clipper",exact:true}).click();
  await expect(page.getByRole("button",{name:"cancel job",exact:true})).toBeVisible();
  await checkAlignment("cancel job");
  await page.evaluate(()=>(window as any).__FINISH_TEST_JOB__());
  await expect(page.locator(".job-head p")).toHaveText("complete");
  await checkAlignment("Open as source");
  await checkAlignment("show in folder");
});

test("Clipper watermark uses Text presets, mouse controls and a single real export",async({page},testInfo)=>{
  test.skip(process.platform!=="win32","Windows FFmpeg audit");test.setTimeout(180_000);
  const audit=resolve(testInfo.outputDir,"watermark");mkdirSync(audit,{recursive:true});
  const source=resolve(audit,"source.mp4");
  expect(spawnSync("ffmpeg",["-v","error","-y","-f","lavfi","-i","color=black:s=1920x1080:r=30:d=1","-c:v","libx264",source]).status).toBe(0);
  const fontPath="C:/Windows/Fonts/impact.ttf",font=readFileSync(fontPath).toString("base64");
  await page.addInitScript(({fontPath})=>localStorage.setItem("container-text-presets-v1",JSON.stringify([{id:"wm",name:"My watermark",sourceWidth:1920,layer:{id:1,text:"myChannel",x:35,y:60,size:92,color:"#00ff00",opacity:85,align:"center",fontName:"Impact",font_path:fontPath,outline:4,outline_color:"#ffffff",shadow:5,shadow_color:"#ff0000",background:true,background_color:"#203040",background_opacity:60,background_padding:12}}])),{fontPath});
  await page.route("http://asset.localhost/**",route=>route.fulfill({body:readFileSync(source),contentType:"video/mp4"}));
  await mockDesktop(page,{fixture:{...sample,path:source}});await openFixture(page);await stageMocks(page);
  await page.evaluate(({font,fontPath})=>{const previous=(window as any).__TEST_HANDLER__;(window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="list_system_fonts"?[{name:"Impact",path:fontPath}]:cmd==="font_preview_data"?`data:font/ttf;base64,${font}`:previous(cmd,args)},{font,fontPath});
  await page.getByPlaceholder("search tools...").fill("Clipper");await page.getByText("Clipper",{exact:true}).last().click();
  await page.getByRole("checkbox",{name:"Watermark",exact:true}).check();
  await page.getByRole("combobox",{name:"Watermark preset",exact:true}).selectOption("wm");
  await page.locator(".clipper-watermark-controls").first().getByRole("button",{name:"Apply",exact:true}).click();
  await expect(page.getByRole("textbox",{name:"Watermark text"})).toHaveValue("myChannel");
  await expect(page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true})).toHaveAttribute("aria-pressed","true");
  await page.getByText("Appearance settings",{exact:true}).click();
  await expect(page.getByRole("slider",{name:"Font size",exact:true})).toHaveValue("52");
  const group=page.getByRole("group",{name:"Move watermark"});const before=(await group.boundingBox())!;
  // Repeated preset/layout changes must not accumulate stale pointer handlers or geometry.
  const runtimeErrors:string[]=[];page.on("pageerror",error=>runtimeErrors.push(error.message));
  for(let cycle=0;cycle<12;cycle++){
    await page.getByRole("button",{name:["SPLIT","SQUARES","FREECAM"][cycle%3],exact:true}).click();
    await page.getByRole("combobox",{name:"Watermark preset",exact:true}).selectOption("wm");
    await page.locator(".clipper-watermark-controls").first().getByRole("button",{name:"Apply",exact:true}).click();
    const start=(await group.boundingBox())!,x=start.x+start.width/2,y=start.y+start.height/2;
    await page.mouse.move(x,y);await page.mouse.down();await page.keyboard.down("Shift");
    await page.mouse.move(x+4,y+18,{steps:3});
    expect(Math.abs((await group.boundingBox())!.x-start.x)).toBeLessThan(1);
    await page.keyboard.up("Shift");await page.mouse.move(x+15,y+20,{steps:3});await page.mouse.up();
    expect((await group.boundingBox())!.x-start.x).toBeGreaterThan(10);
    const resize=(await page.getByRole("button",{name:"Resize watermark"}).boundingBox())!;
    await page.mouse.move(resize.x+5,resize.y+5);await page.mouse.down();await page.mouse.move(resize.x+15,resize.y+5,{steps:3});await page.mouse.up();
    await page.getByRole("checkbox",{name:"Watermark",exact:true}).uncheck();await expect(group).toHaveCount(0);
    await page.getByRole("checkbox",{name:"Watermark",exact:true}).check();await expect(group).toBeVisible();
  }
  expect(runtimeErrors).toEqual([]);
  await page.getByRole("button",{name:"SPLIT",exact:true}).click();
  await page.getByRole("combobox",{name:"Watermark preset",exact:true}).selectOption("wm");
  await page.locator(".clipper-watermark-controls").first().getByRole("button",{name:"Apply",exact:true}).click();
  await page.getByText("Appearance settings",{exact:true}).click();
  await page.mouse.move(before.x+before.width/2,before.y+before.height/2);await page.mouse.down();await page.mouse.move(before.x+before.width/2+20,before.y+before.height/2+15,{steps:4});await page.mouse.up();
  expect((await group.boundingBox())!.x).toBeGreaterThan(before.x+15);
  const displaced=(await group.boundingBox())!;
  await page.getByRole("button",{name:"Center horizontally",exact:true}).click();
  const centered=(await group.boundingBox())!,canvasBox=(await page.locator(".watermark-canvas-layer canvas").boundingBox())!;
  expect(Math.abs(centered.y-displaced.y)).toBeLessThan(1);
  expect(Math.abs(centered.x+centered.width/2-(canvasBox.x+canvasBox.width/2))).toBeLessThan(2);
  for(const vertical of [true,false]){
    const start=(await group.boundingBox())!,x=start.x+start.width/2,y=start.y+start.height/2;
    await page.mouse.move(x,y);await page.keyboard.down("Shift");await page.mouse.down();
    await page.mouse.move(x+(vertical?5:25),y+(vertical?25:5),{steps:5});await page.mouse.up();await page.keyboard.up("Shift");
    const end=(await group.boundingBox())!;
    expect(Math.abs(vertical?end.x-start.x:end.y-start.y)).toBeLessThan(1);
    expect(vertical?end.y-start.y:end.x-start.x).toBeGreaterThan(20);
  }
  await page.getByRole("button",{name:"Center watermark",exact:true}).click();
  const fullyCentered=(await group.boundingBox())!;
  expect(Math.abs(fullyCentered.y+fullyCentered.height/2-(canvasBox.y+canvasBox.height/2))).toBeLessThan(2);
  const handle=(await page.getByRole("button",{name:"Resize watermark"}).boundingBox())!;
  await page.mouse.move(handle.x+5,handle.y+5);await page.mouse.down();await page.mouse.move(handle.x+25,handle.y+5,{steps:4});await page.mouse.up();
  const jobs:any[]=[];
  for(const layout of ["SPLIT","SQUARES","FREECAM","ORIGINAL SIZE","BLUR","FILL"]){
    await page.getByRole("button",{name:layout,exact:true}).click();
    await page.getByRole("button",{name:"▶ render clipper",exact:true}).click();await expect(page.locator(".job-head p")).toHaveText("complete");
    const request=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").at(-1).args.request);
    const data=await page.locator(".watermark-canvas-layer canvas").evaluate((canvas:HTMLCanvasElement)=>canvas.toDataURL());expect(request.params.text_raster_png).toBe(data);
    const layer=JSON.parse(request.params.watermark_layer);expect(layer.outline).toBe(2.25);expect(layer.shadow).toBe(2.8125);expect(layer.background).toBe(true);
    const name=layout.replaceAll(" ","-").toLowerCase(),raster=resolve(audit,`${name}-raster.png`);writeFileSync(raster,Buffer.from(data.split(",")[1],"base64"));
    delete request.params.text_raster_png;request.params.text_raster_path=raster;jobs.push({name,request});
  }
  const manifest=resolve(audit,"requests.json");writeFileSync(manifest,JSON.stringify(jobs));
  await page.getByRole("button",{name:"SAVE PROJECT",exact:true}).click();
  const saved=await page.evaluate(()=>(window as any).__SAVED_PROJECT__);
  expect(JSON.stringify(saved)).toContain("watermark_layer");expect(JSON.stringify(saved)).not.toContain("text_raster_png");
  await page.screenshot({path:"test-results/clipper-watermark-controls.png"});
  const rendered=spawnSync("cargo",["test","--manifest-path","src-tauri/Cargo.toml","--lib","clipper_preview_export_audit","--","--ignored"],{env:{...process.env,CONTAINER_CLIPPER_AUDIT_MANIFEST:manifest},encoding:"utf8",timeout:120_000});expect(rendered.status,rendered.stdout+rendered.stderr).toBe(0);
  for(const job of jobs){const difference=await page.evaluate(async({reference,output})=>{const pixels=async(data:string)=>{const image=new Image();image.src=`data:image/png;base64,${data}`;await image.decode();const canvas=document.createElement("canvas");canvas.width=image.width;canvas.height=image.height;const ctx=canvas.getContext("2d")!;ctx.fillStyle="black";ctx.fillRect(0,0,canvas.width,canvas.height);ctx.drawImage(image,0,0);return ctx.getImageData(0,0,canvas.width,canvas.height).data};const a=await pixels(reference),b=await pixels(output);let sum=0;for(let i=0;i<a.length;i++)sum+=Math.abs(a[i]-b[i]);return sum/a.length},{reference:readFileSync(job.request.params.text_raster_path).toString("base64"),output:readFileSync(resolve(audit,`${job.name}-export.png`)).toString("base64")});expect(difference,job.name).toBeLessThan(1)}
});

test("Clipper real-media preview matches exported pixels at zero, sixteen and thirty-one percent",async({page},testInfo)=>{
  test.skip(process.platform!=="win32","Real Clipper export audit runs in the Windows CI and release jobs.");
  test.setTimeout(900_000);
  const audit=resolve(testInfo.outputDir,"clipper-parity");mkdirSync(audit,{recursive:true});
  const source=process.env.CONTAINER_CLIPPER_AUDIT_MEDIA??resolve(audit,"synthetic-source.mp4");
  if(!process.env.CONTAINER_CLIPPER_AUDIT_MEDIA){
    // Keep spatial detail, but reserve green for the logo so source colours
    // cannot be misclassified as logo pixels by the bounds comparison below.
    const generated=spawnSync("ffmpeg",["-hide_banner","-loglevel","error","-y","-f","lavfi","-i","testsrc2=size=1920x1080:rate=30:duration=1,hue=s=0","-c:v","libx264","-preset","ultrafast","-pix_fmt","yuv420p","-movflags","+faststart",source],{encoding:"utf8",timeout:120_000});
    expect(generated.status,generated.stdout+generated.stderr).toBe(0);
  }
  await page.setViewportSize({width:1600,height:1000});
  const mediaBytes=readFileSync(source);
  await page.route("http://asset.localhost/**",route=>route.fulfill({body:mediaBytes,contentType:"video/mp4",headers:{"Access-Control-Allow-Origin":"*"}}));
  await mockDesktop(page,{fixture:{...sample,path:source}});await openFixture(page);await stageMocks(page);
  await page.evaluate(()=>{
    const previous=(window as any).__TEST_HANDLER__;
    (window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="detect_camera_region"?{x:78,y:75,width:22,height:25,confidence:1,samples:1,matched_samples:1}:previous?.(cmd,args);
  });
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper",{exact:true}).last().click();
  await page.getByRole("button",{name:/AUTO-DETECT CAMERA/}).click();
  await expect.poll(()=>page.locator(".video-canvas video").first().evaluate((v:any)=>v.readyState)).toBeGreaterThanOrEqual(2);
  await page.getByText("Social Tag",{exact:true}).click();
  await page.getByPlaceholder("kanaladi").fill("batuhanfurkan5");
  await page.getByRole("combobox",{name:"Style"}).selectOption("kick_banner");
  await page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true}).click();
  await expect(page.locator(".clipper-output-canvas")).toBeVisible();
  await page.evaluate(()=>document.fonts.ready);
  const jobs:{name:string;request:unknown;preview:string;bannerTop:number;bannerBottom:number}[]=[];
  for(const layout of ["SPLIT","SQUARES","FREECAM"]){
    await page.getByRole("button",{name:layout,exact:true}).click();
    for(const distance of ["0","16","31"]){
      await page.getByRole("slider",{name:"Move banner away from camera"}).fill(distance);
      await page.getByRole("button",{name:"▶ render clipper",exact:true}).click();
      await expect(page.locator(".job-head p")).toHaveText("complete");
      const request=await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").at(-1).args.request);
      const name=`${layout.toLowerCase()}-${distance}`;
      const box=(await page.locator(".clipper-output-canvas").boundingBox())!;
      const banner=(await page.locator(".clipper-social-banner").boundingBox())!;
      const prefix=(await page.locator(".clipper-social-banner-prefix").boundingBox())!;
      const bar=(await page.locator(".clipper-social-banner-bg").boundingBox())!;
      expect(prefix.y,`${name}: prefix must stay inside the black bar`).toBeGreaterThanOrEqual(bar.y-.5);
      const preview=await page.screenshot({clip:box,path:resolve(audit,`${name}-preview.png`),timeout:15_000});
      jobs.push({name,request,preview:preview.toString("base64"),bannerTop:Math.floor((banner.y-box.y)/box.height*640),bannerBottom:Math.ceil((banner.y+banner.height-box.y)/box.height*640)});
    }
  }
  const manifest=resolve(audit,"requests.json");writeFileSync(manifest,JSON.stringify(jobs.map(({name,request})=>({name,request}))));
  const rendered=spawnSync("cargo",["test","--manifest-path","src-tauri/Cargo.toml","--lib","clipper_preview_export_audit","--","--ignored"],{cwd:resolve("."),env:{...process.env,CONTAINER_CLIPPER_AUDIT_MANIFEST:manifest},encoding:"utf8",timeout:600_000});
  expect(rendered.status,rendered.stdout+rendered.stderr).toBe(0);
  for(const job of jobs){
    const exported=readFileSync(resolve(audit,`${job.name}-export.png`)).toString("base64");
    const difference=await page.evaluate(async({preview,exported,bannerTop,bannerBottom})=>{
      const pixels=async(base64:string)=>{const img=new Image();img.src=`data:image/png;base64,${base64}`;await img.decode();const c=document.createElement("canvas");c.width=360;c.height=640;const ctx=c.getContext("2d")!;ctx.drawImage(img,0,0,360,640);return ctx.getImageData(0,0,360,640).data};
      const a=await pixels(preview),b=await pixels(exported);let sum=0;
      const greenBounds=(data:Uint8ClampedArray)=>{let top=640,bottom=0,left=360,right=0;for(let y=Math.max(0,bannerTop);y<Math.min(640,bannerBottom);y++)for(let x=0;x<140;x++){const i=(y*360+x)*4;if(data[i]>30&&data[i]<130&&data[i+1]>220&&data[i+2]<100){top=Math.min(top,y);bottom=Math.max(bottom,y);left=Math.min(left,x);right=Math.max(right,x)}}return {top,bottom,left,right}};
      for(let i=0;i<a.length;i+=4)for(let channel=0;channel<3;channel++)sum+=Math.abs(a[i+channel]-b[i+channel]);
      return {mean:sum/(360*640*3),previewLogo:greenBounds(a),exportLogo:greenBounds(b)};
    },{preview:job.preview,exported,bannerTop:job.bannerTop,bannerBottom:job.bannerBottom});
    await testInfo.attach(job.name,{body:JSON.stringify(difference),contentType:"application/json"});
    expect(difference.mean,`${job.name}: composition pixel error`).toBeLessThan(12);
    for(const edge of ["top","bottom","left","right"] as const)expect(Math.abs(difference.previewLogo[edge]-difference.exportLogo[edge]),`${job.name}: logo ${edge}`).toBeLessThanOrEqual(2);
  }
});

test("Social Tag preview keeps text and icon aligned at small sizes",async({page})=>{
  await mockDesktop(page);
  await openFixture(page);
  await page.setViewportSize({width:1920,height:1080});
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper",{exact:true}).last().click();
  await page.getByRole("button",{name:"OUTPUT PREVIEW",exact:true}).click();
  await page.getByText("Social Tag",{exact:true}).click();
  await page.getByPlaceholder("kanaladi").fill("eray");
  const style=page.getByRole("combobox",{name:"Style"});
  const platform=page.getByRole("combobox",{name:"Platform"});
  const size=page.locator(".clipper-watermark-controls input[type=range]").first();
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
  await expect(page.getByRole("button",{name:"show in folder",exact:true})).toBeVisible();
  await page.getByRole("combobox",{name:"Position"}).selectOption("left");
  await expect(page.locator(".job-head p")).toHaveText("settings changed · render again");
  await expect(page.getByRole("button",{name:"old output · not updated",exact:true})).toBeVisible();
  for(const width of [1440,1100,900]){
    await page.setViewportSize({width,height:900});
    if(width===1440&&process.env.UI_AUDIT_SCREENSHOTS)await page.screenshot({path:"test-results/clipper-process-actions-1440.png"});
    const snapshot=await page.evaluate(()=>{
      const process=document.querySelector(".job")!.getBoundingClientRect();
      const settings=document.querySelector(".settings")!.getBoundingClientRect();
      const actions=Array.from(document.querySelectorAll(".job .job-action"),action=>({label:action.textContent?.trim()??"",bounds:action.getBoundingClientRect().toJSON()}));
      return {process:process.toJSON(),settings:settings.toJSON(),actions};
    });
    for(const {label,bounds} of snapshot.actions){
      expect(bounds.x,`${width}px: ${label} must stay inside Process`).toBeGreaterThanOrEqual(snapshot.process.x-1);
      expect(bounds.x+bounds.width).toBeLessThanOrEqual(snapshot.process.x+snapshot.process.width+1);
      expect(bounds.x+bounds.width).toBeLessThan(snapshot.settings.x);
      expect(bounds.y+bounds.height).toBeLessThanOrEqual(snapshot.process.y+snapshot.process.height+1);
    }
  }
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
  await expect(page.locator(".landing-copy h2")).toHaveText("one place. every tool.");
  await page.locator(".landing-language").getByRole("button", { name: "TR" }).click();
  await expect(page.locator(".landing-project-actions")).toHaveText("PROJE AÇ");
  await expect(page.locator(".landing-copy h2")).toHaveText("tek yerde. tüm araçlar.");
  await page.reload();
  await expect(page.locator(".landing-project-actions")).toHaveText("PROJE AÇ");
  await expect(page.locator(".landing-copy h2")).toHaveText("tek yerde. tüm araçlar.");
  await page.locator(".landing-language").getByRole("button", { name: "EN" }).click();
  await expect(page.locator(".landing-project-actions")).toHaveText("OPEN PROJECT");
  await expect(page.locator(".landing-copy h2")).toHaveText("one place. every tool.");
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

for (const [kind, path, operation, expectedOptions] of [["audio", "C:\\fixtures\\sample.mp3", "audio_convert", ["audio_convert", "fix_timestamps"]], ["image", "C:\\fixtures\\sample.jpg", "image_compressor", ["image_compressor", "metadata_cleaner"]]] as const) {
  test(`Batch opens ${kind} sources with a matching operation`, async ({ page }) => {
    await mockDesktop(page, { fixture: { ...sample, path, name: path.split("\\").at(-1)!, kind, duration: kind === "image" ? 0.04 : 60, width: kind === "image" ? 640 : 0, height: kind === "image" ? 360 : 0, fps: kind === "image" ? 25 : 0, codec: kind === "image" ? "mjpeg" : "mp3", audio_codec: kind === "audio" ? "mp3" : null } });
    await openFixture(page);
    await page.getByRole("button", { name: "BATCH", exact: true }).click();
    await expect(page.locator(".batch-control select").first()).toHaveValue(operation);
    expect(await page.locator(".batch-control select").first().locator("option").evaluateAll(options => options.map(option => (option as HTMLOptionElement).value))).toEqual(expectedOptions);
    await expect(page.getByRole("button", { name: /START QUEUE/ })).toBeEnabled();
    await page.evaluate(() => { (window as any).__TEST_HANDLER__ = (cmd: string) => cmd === "run_operation" ? { output: "C:\\fixtures\\converted.out", elapsed: 0.1 } : undefined; });
    await page.getByRole("button", { name: /START QUEUE/ }).click();
    await expect(page.locator(".batch-items article > strong")).toHaveText("complete");
    expect(await page.evaluate(() => (window as any).__TEST_CALLS__.filter((call: any) => call.cmd === "run_operation").at(-1).args.request.operation)).toBe(operation);
  });
}

test("Batch video operations exclude audio-only and image-only tools", async ({ page }) => {
  await mockDesktop(page); await openFixture(page);
  await page.getByRole("button", { name: "BATCH", exact: true }).click();
  const options = await page.locator(".batch-control select").first().locator("option").evaluateAll(nodes => nodes.map(node => (node as HTMLOptionElement).value));
  expect(options).toContain("fps");
  expect(options).toContain("extract_audio");
  expect(options).not.toContain("audio_convert");
  expect(options).not.toContain("image_compressor");
  expect(options).not.toContain("metadata_cleaner");
});

test("landing multi-video selection opens Batch and converts every video to the chosen FPS", async ({ page }) => {
  await mockDesktop(page);
  await page.evaluate(() => {
    (window as any).__TEST_HANDLER__ = (cmd: string, args: any) => {
      if (cmd === "plugin:dialog|open" && args.options?.filters?.[0]?.extensions?.includes("bmp")) return ["C:\\fixtures\\sample.mp4", "C:\\fixtures\\second.mp4"];
      if (cmd === "run_operation") return { output: `${args.request.input}.out.mp4`, elapsed: 0.1 };
    };
  });
  await page.locator(".dropzone").click();
  await expect(page.locator(".batch-workspace")).toBeVisible();
  await expect(page.locator(".batch-items article")).toHaveCount(2);
  await expect(page.locator(".settings")).toHaveCount(0);
  await expect(page.locator(".file-summary")).toContainText("2");
  await expect(page.locator(".file-summary")).not.toContainText("sample.mp4");
  await page.locator(".batch-control select").first().selectOption("fps");
  await page.getByRole("spinbutton", { name: "Target FPS" }).fill("10");
  await expect(page.getByRole("combobox", { name: "Quality" })).toHaveValue("0");
  await page.getByRole("button", { name: /START QUEUE/ }).click();
  await expect(page.locator(".batch-items article > strong")).toHaveText(["complete", "complete"]);
  const requests = await page.evaluate(() => (window as any).__TEST_CALLS__.filter((call: any) => call.cmd === "run_operation").map((call: any) => call.args.request));
  expect(requests.map((request: any) => request.input)).toEqual(["C:\\fixtures\\sample.mp4", "C:\\fixtures\\second.mp4"]);
  expect(requests.map((request: any) => [request.operation, request.params.fps, request.params.crf])).toEqual([["fps", "10", "0"], ["fps", "10", "0"]]);
  await page.getByRole("button", { name: "SAVE PROJECT" }).click();
  const saved = await page.evaluate(() => JSON.parse((window as any).__TEST_CALLS__.find((call: any) => call.cmd === "write_project").args.contents));
  expect(saved.workspaceMode).toBe("batch");
  expect(saved.batch.items.map((item: any) => item.path)).toEqual(["C:\\fixtures\\sample.mp4", "C:\\fixtures\\second.mp4"]);
  expect(saved.batch.selected.id).toBe("fps");
  expect(saved.resources.map((resource: any) => resource.path)).toContain("C:\\fixtures\\second.mp4");
});

test("dropping multiple videos routes to Batch and further drops extend the queue", async ({ page }) => {
  await mockDesktop(page);
  await page.evaluate(() => (window as any).__TEST_DROP__(["C:\\fixtures\\sample.mp4", "C:\\fixtures\\second.mp4"]));
  await expect(page.locator(".batch-items article")).toHaveCount(2);
  await page.evaluate(() => (window as any).__TEST_DROP__("C:\\fixtures\\third.mp4"));
  await expect(page.locator(".batch-items article")).toHaveCount(3);
  await expect(page.locator(".file-summary")).toContainText("3");
});

test("Batch retries only failed rows and keeps completed output actions", async ({ page }) => {
  await mockDesktop(page);
  await openFixture(page);
  await page.getByRole("button", { name: "BATCH", exact: true }).click();
  await page.getByRole("button", { name: "+ FILES", exact: true }).click();
  await expect(page.locator(".batch-items article")).toHaveCount(2);
  await page.evaluate(() => {
    (window as any).__TEST_HANDLER__ = (cmd: string, args: any) => {
      if (cmd === "run_operation" && args.request.input.includes("second")) return Promise.reject(new Error("second failed"));
      if (cmd === "run_operation") return { output: "C:\\fixtures\\first-output.mp4", elapsed: 0.1 };
    };
  });
  await page.getByRole("button", { name: /START QUEUE/ }).click();
  await expect(page.locator(".batch-items article > strong")).toHaveText(["complete", "failed"]);
  await expect(page.getByRole("button", { name: "Play output for sample.mp4" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Show output for sample.mp4" })).toBeVisible();
  if (process.env.UI_AUDIT_SCREENSHOTS) {
    await page.screenshot({ path: "test-results/batch-retry-output-actions-light.png" });
    await page.evaluate(() => document.documentElement.dataset.theme = "dark");
    await page.screenshot({ path: "test-results/batch-retry-output-actions-dark.png" });
  }
  await page.getByRole("button", { name: "Play output for sample.mp4" }).click();
  await page.getByRole("button", { name: "Show output for sample.mp4" }).click();
  const openerCalls = await page.evaluate(() => (window as any).__TEST_CALLS__.filter((call: any) => call.cmd.startsWith("plugin:opener|")));
  expect(openerCalls.map((call: any) => call.cmd)).toEqual(expect.arrayContaining(["plugin:opener|open_path", "plugin:opener|reveal_item_in_dir"]));
  await page.evaluate(() => {
    (window as any).__TEST_HANDLER__ = (cmd: string) => cmd === "run_operation" ? { output: "C:\\fixtures\\second-output.mp4", elapsed: 0.1 } : undefined;
  });
  await page.getByRole("button", { name: /RETRY FAILED ONLY \(1\)/ }).click();
  await expect(page.locator(".batch-items article > strong")).toHaveText(["complete", "complete"]);
  const requests = await page.evaluate(() => (window as any).__TEST_CALLS__.filter((call: any) => call.cmd === "run_operation").map((call: any) => call.args.request.input));
  expect(requests).toEqual(["C:\\fixtures\\sample.mp4", "C:\\fixtures\\second.mp4", "C:\\fixtures\\second.mp4"]);
  await expect(page.locator(".batch-items article").first()).toContainText("first-output.mp4");
});

test("Batch shows source metadata and blocks an incompatible file before render", async ({ page }) => {
  await mockDesktop(page);
  await openFixture(page);
  await page.getByRole("button", { name: "BATCH", exact: true }).click();
  await page.evaluate(() => {
    (window as any).__TEST_HANDLER__ = (cmd: string, args: any) => {
      if (cmd === "plugin:dialog|open" && args.options?.multiple) return ["C:\\fixtures\\sound.mp3"];
      if (cmd === "probe_media" && args.path.endsWith("sound.mp3")) return { ...((window as any).__TEST_AUDIO__ ?? {}), path: args.path, kind: "audio", fps: null, duration: 12, width: null, height: null };
    };
  });
  await page.getByRole("button", { name: "+ FILES", exact: true }).click();
  await expect(page.locator(".batch-source-meta").first()).toContainText("1920×1080 · 30 FPS · 1:00");
  await expect(page.locator(".batch-source-meta").last()).toContainText("AUDIO · 0:12");
  await expect(page.locator(".batch-item-warning")).toContainText("does not support this file type");
  await expect(page.getByRole("button", { name: /START QUEUE/ })).toBeDisabled();
  expect(await page.evaluate(() => (window as any).__TEST_CALLS__.filter((call: any) => call.cmd === "run_operation").length)).toBe(0);
});

test("Batch warns when target FPS exceeds a source and offers a smaller high-quality output", async ({ page }) => {
  await mockDesktop(page);
  await page.evaluate(() => {
    (window as any).__TEST_HANDLER__ = (cmd: string, args: any) => {
      if (cmd === "plugin:dialog|open" && args.options?.filters?.[0]?.extensions?.includes("bmp")) return ["C:\\fixtures\\sample.mp4", "C:\\fixtures\\slow.mp4"];
      if (cmd === "probe_media" && args.path.endsWith("slow.mp4")) return { path: args.path, kind: "video", fps: 5, duration: 12, width: 640, height: 360 };
      if (cmd === "run_operation") return { output: `${args.request.input}.out.mp4`, elapsed: 0.1 };
    };
  });
  await page.locator(".dropzone").click();
  await page.locator(".batch-control select").first().selectOption("fps");
  await page.getByRole("spinbutton", { name: "Target FPS" }).fill("10");
  await expect(page.locator(".batch-warning")).toContainText("frames will be duplicated");
  await page.getByRole("combobox", { name: "Quality" }).selectOption("16");
  await page.getByRole("button", { name: /START QUEUE/ }).click();
  await expect(page.locator(".batch-items article > strong")).toHaveText(["complete", "complete"]);
  const requests = await page.evaluate(() => (window as any).__TEST_CALLS__.filter((call: any) => call.cmd === "run_operation").map((call: any) => call.args.request));
  expect(requests.map((request: any) => request.params.crf)).toEqual(["16", "16"]);
});

test("mixed multi-selection does not silently drop files", async ({ page }) => {
  await mockDesktop(page);
  await page.evaluate(() => (window as any).__TEST_DROP__(["C:\\fixtures\\sample.mp4", "C:\\fixtures\\still.jpg"]));
  await expect(page.locator(".dropzone")).toBeVisible();
  await expect(page.locator(".batch-workspace")).toHaveCount(0);
  await expect(page.locator(".app-toast")).toContainText("Multi-import accepts videos only");
});

test("workspace roundtrips preserve Clipper edits, SmartCut cuts and Batch inputs", async ({ page }) => {
  await mockDesktop(page);
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Clipper");
  await page.getByText("Clipper", {exact:true}).last().click();
  await page.getByText("Social Tag", {exact:true}).click();
  await page.getByPlaceholder("kanaladi").fill("keep_this_name");
  await page.getByRole("button", {name:"SMARTCUT",exact:true}).click();
  await page.getByRole("button", {name:"DETECT SILENCES",exact:true}).click();
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

test("Batch removal during rendering keeps queue identities stable", async ({ page }) => {
  await mockDesktop(page);await openFixture(page);
  await page.evaluate(()=>{(window as any).__TEST_PENDING__=[];(window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="run_operation"?new Promise(resolve=>(window as any).__TEST_PENDING__.push(()=>resolve({output:"C:\\fixtures\\out.mp4",elapsed:1}))):undefined});
  await page.getByRole("button",{name:"BATCH",exact:true}).click();
  await page.getByRole("button",{name:"+ FILES",exact:true}).click();
  await page.getByRole("button",{name:/START QUEUE/}).click();
  await page.waitForFunction(()=>(window as any).__TEST_PENDING__.length===1);
  await page.getByRole("button",{name:"Remove from queue",exact:true}).last().click();
  await page.evaluate(()=>(window as any).__TEST_PENDING__.shift()());
  await expect(page.locator(".batch-control select").first()).toBeEnabled();
  await expect(page.locator(".batch-items article > strong")).toHaveText(["complete"]);
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").length)).toBe(1);
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
  await page.getByRole("button", {name:"DETECT SILENCES",exact:true}).click();
  await page.waitForFunction(()=>(window as any).__TEST_PENDING__.length===1);
  await page.getByText("OTHER SETTINGS").click();
  await page.getByRole("slider",{name:"MINIMUM PAUSE"}).fill("0.75");
  await page.evaluate(()=>(window as any).__TEST_PENDING__.shift()());
  await page.waitForFunction(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="analyze_autocut").length===2);
  await expect(page.getByRole("button",{name:"EXPORT MP4",exact:true})).toBeDisabled();
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="analyze_autocut").map((call:any)=>call.args.request.minimum_pause))).toEqual([0.41,0.75]);
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

test("UI consistency: global settings preserve the open media and persist preferences",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.getByRole("button",{name:"General settings",exact:true}).click();
  const dialog=page.getByRole("dialog",{name:"General settings",exact:true});
  await dialog.getByRole("combobox",{name:"Language",exact:true}).selectOption("tr");
  await expect(page.locator(".filename")).toHaveText("sample.mp4");
  await page.getByRole("dialog").getByRole("combobox",{name:"Tema",exact:true}).selectOption("light");
  await page.getByRole("checkbox",{name:/İşlem bitince bildir/}).check();
  await page.screenshot({path:"test-results/general-settings-tr-light.png"});
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("button",{name:"Genel ayarlar",exact:true})).toBeFocused();
  await expect(page.locator(".settings h2")).toHaveText("Dönüştür");
  expect(await page.evaluate(()=>localStorage.getItem("container-completion-alert"))).toBe("true");
  expect(await page.evaluate(()=>document.documentElement.dataset.theme)).toBe("light");
  await page.getByRole("button",{name:"Genel ayarlar",exact:true}).click();
  await expect(page.getByRole("checkbox",{name:/İşlem bitince bildir/})).toBeChecked();
});

test("Accent wheel updates global controls live, persists and respects themes",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  const stack=page.locator(".processing-stack-add");
  const original=await stack.evaluate(el=>getComputedStyle(el).color);
  await page.getByRole("button",{name:"General settings",exact:true}).click();
  const dialog=page.getByRole("dialog",{name:"General settings",exact:true});
  await dialog.getByRole("button",{name:"Color 330°",exact:true}).click();
  const changed=await stack.evaluate(el=>({color:getComputedStyle(el).color,background:getComputedStyle(el).backgroundColor}));
  expect(changed.color).not.toBe(original);
  expect(changed.background).not.toBe("rgb(24, 40, 59)");
  const wheel=dialog.getByRole("slider",{name:"Color wheel"});
  await wheel.focus();await page.keyboard.press("ArrowRight");await expect(wheel).toHaveAttribute("aria-valuenow","331");
  const box=(await wheel.boundingBox())!;
  await page.mouse.move(box.x+box.width/2,box.y+5);await page.mouse.down();
  await page.mouse.move(box.x+box.width-5,box.y+box.height/2);await page.mouse.up();
  await expect(wheel).toHaveAttribute("aria-valuenow","90");
  await dialog.getByRole("slider",{name:/Saturation/}).fill("70");
  await dialog.getByRole("combobox",{name:"Theme",exact:true}).selectOption("light");
  expect(await page.evaluate(()=>document.documentElement.style.getPropertyValue("--accent-hue"))).toBe("90");
  expect(await page.evaluate(()=>parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--accent-lightness")))).toBeLessThanOrEqual(44);
  // Accent preferences must not redefine semantic success colors.
  expect(await page.evaluate(()=>getComputedStyle(document.documentElement).getPropertyValue("--green").trim())).toBe("#117d43");
  await page.keyboard.press("Escape");await expect(page.locator(".filename")).toHaveText("sample.mp4");
  await page.reload();await openFixture(page);
  expect(await page.evaluate(()=>document.documentElement.style.getPropertyValue("--accent-hue"))).toBe("90");
  expect(await page.evaluate(()=>document.documentElement.style.getPropertyValue("--accent-saturation"))).toBe("70%");
  await page.getByRole("button",{name:"General settings",exact:true}).click();
  await page.getByRole("dialog").getByRole("button",{name:"Reset",exact:true}).click();
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem("container-accent")!))).toEqual({hue:213,saturation:94});
  await page.screenshot({path:"test-results/accent-settings-light.png"});
});

test("UI typography separates readable labels from technical values in both themes",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  for(const theme of ["light","dark"]){
    await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
    for(const selector of [".transform-options button",".tool-row b",".tool-row small",".job-stats b"]){
      const target=page.locator(selector).first();
      const style=await target.evaluate(el=>({family:getComputedStyle(el).fontFamily,size:parseFloat(getComputedStyle(el).fontSize)}));
      expect(style.family).toContain("Geist Variable");expect(style.family).not.toContain("Mono");expect(style.size).toBeGreaterThanOrEqual(11);
    }
    expect(await page.locator(".job-stats").evaluate(el=>getComputedStyle(el).fontFamily)).toContain("Mono");
    await page.screenshot({path:`test-results/ui-typography-${theme}.png`});
  }
});

test("Interpolation hint follows English and Turkish live without leaking saved language",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("FPS Interpolation");
  await page.locator(".tool-row").filter({has:page.getByText("FPS Interpolation",{exact:true})}).click();
  await expect(page.locator(".hint")).toContainText("Automatically set to the next suitable multiple of 60");
  await page.getByRole("button",{name:"General settings",exact:true}).click();
  await page.locator(".general-settings-dialog select").first().selectOption("tr");
  await page.getByRole("button",{name:"Tamam",exact:true}).click();
  await expect(page.locator(".hint")).toContainText("Dosya açıldığında");
  await page.getByRole("button",{name:"Genel ayarlar",exact:true}).click();
  await page.locator(".general-settings-dialog select").first().selectOption("en");
  await page.getByRole("button",{name:"Done",exact:true}).click();
  await expect(page.locator(".hint")).not.toContainText("Dosya açıldığında");
  await expect(page.locator(".hint")).toContainText("Automatically set");
});

test("Silent video explains SmartCut and Extract Audio without starting jobs",async({page})=>{
  await mockDesktop(page,{fixture:{...sample,audio_codec:null,audio_tracks:[]}});await openFixture(page);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  await page.getByRole("button",{name:"DETECT SILENCES",exact:true}).click();
  await expect(page.locator(".ac-error")).toContainText("This video has no audio");
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="analyze_autocut").length)).toBe(0);
  await page.getByRole("button",{name:"TOOLBOX",exact:true}).click();
  await page.locator(".tabs").getByRole("button",{name:/^audio$/i}).click();
  await page.getByPlaceholder("search tools...").fill("Extract Audio");
  await page.locator(".tool-row").filter({has:page.getByText("Extract Audio",{exact:true})}).click();
  await page.getByRole("button",{name:/render extract audio/i}).click();
  await expect(page.locator(".center-stack")).toContainText("This video has no audio");
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").length)).toBe(0);
});

test("Downloader engine indicator stays green or red independently of accent",async({page})=>{
  await mockDesktop(page);await page.goto("/");
  await page.locator(".downloader-quick-trigger").click();
  const dot=page.locator(".engine-details .status-dot");await expect(dot).toBeVisible();
  for(const missing of [false,true]){
    await dot.evaluate((el,missing)=>{el.classList.toggle("missing",missing);document.documentElement.style.setProperty("--blue","orange")},missing);
    await expect.poll(()=>dot.evaluate((el,missing)=>{const probe=document.createElement("span");probe.style.color=missing?"var(--red)":"var(--green)";el.append(probe);const expected=getComputedStyle(probe).color;probe.remove();return getComputedStyle(el).backgroundColor===expected},missing)).toBe(true);
  }
});

test("Upload icon follows the live accent in both themes",async({page})=>{
  await mockDesktop(page);await page.goto("/");
  const icon=page.locator(".drop-icon");await expect(icon).toBeVisible();
  const initial=await icon.evaluate(el=>getComputedStyle(el).color);
  await page.getByRole("button",{name:"General settings",exact:true}).click();
  const dialog=page.getByRole("dialog",{name:"General settings",exact:true});
  await dialog.getByRole("button",{name:"Color 330°",exact:true}).click();
  await expect.poll(()=>icon.evaluate(el=>getComputedStyle(el).color)).not.toBe(initial);
  for(const theme of ["light","dark"]){
    await dialog.getByRole("combobox",{name:"Theme",exact:true}).selectOption(theme);
    await expect.poll(()=>icon.evaluate(el=>{const probe=document.createElement("span");probe.style.color="var(--blue)";el.append(probe);const expected=getComputedStyle(probe).color;probe.remove();return getComputedStyle(el).color===expected})).toBe(true);
  }
});

test("Accent picker fits small windows and ignores corrupt saved preferences",async({page})=>{
  await page.setViewportSize({width:340,height:500});
  await page.addInitScript(()=>localStorage.setItem("container-accent",'{"hue":"invalid","saturation":500}'));
  await mockDesktop(page);await openFixture(page);
  await page.getByRole("button",{name:"General settings",exact:true}).click();
  const dialog=page.getByRole("dialog",{name:"General settings",exact:true});
  await expect(dialog.getByRole("slider",{name:"Color wheel"})).toHaveAttribute("aria-valuenow","213");
  expect(await dialog.evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true);
  const bounds=(await dialog.boundingBox())!;expect(bounds.x).toBeGreaterThanOrEqual(0);expect(bounds.y).toBeGreaterThanOrEqual(0);
  expect(bounds.y+bounds.height).toBeLessThanOrEqual(500);
  await page.screenshot({path:"test-results/accent-settings-small.png"});
  await dialog.getByRole("button",{name:"Done",exact:true}).click();
  await expect(dialog).not.toBeVisible();
});

test("UI consistency: search and empty queue provide a clear next action",async({page})=>{
  await mockDesktop(page);
  await expect(page.locator(".multi-import-hint")).toContainText("multiple videos");
  await expect(page.locator(".downloader-quick-trigger")).toContainText("Download from link");
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("nothing-matches-this-tool");
  await expect(page.getByText("No matching tools found.",{exact:true})).toBeVisible();
  await page.getByRole("button",{name:"Clear search",exact:true}).click();
  expect(await page.locator(".tool-row").count()).toBeGreaterThan(0);
  await page.getByRole("button",{name:"BATCH",exact:true}).click();
  await page.getByRole("button",{name:"Remove from queue",exact:true}).click();
  await expect(page.getByText("Your queue is empty",{exact:true})).toBeVisible();
  await page.getByRole("button",{name:"+ Add files",exact:true}).click();
  await expect(page.locator(".batch-items article")).toHaveCount(1);
});

test("UI consistency: completed Batch outputs stay accessible during the next job",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.getByRole("button",{name:"BATCH",exact:true}).click();
  await page.getByRole("button",{name:"+ FILES",exact:true}).click();
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string,args:any)=>{
    if(cmd==="run_operation")return args.request.input.includes("second")?new Promise((_resolve,reject)=>(window as any).__CANCEL_SECOND__=reject):{output:"C:\\fixtures\\first-output.mp4"};
    if(cmd==="cancel_job"){(window as any).__CANCEL_SECOND__(new Error("cancelled"));return null}
    return undefined;
  }});
  await page.getByRole("button",{name:/START QUEUE/}).click();
  await expect(page.locator(".batch-items article>strong")).toHaveText(["complete","running"]);
  const first=page.locator(".batch-items article").first();
  await expect(first.getByRole("button",{name:"Play output for sample.mp4"})).toBeEnabled();
  await expect(first.getByRole("button",{name:"Show output for sample.mp4"})).toBeEnabled();
  await expect(first.getByRole("button",{name:"Open in Toolbox",exact:true})).toBeDisabled();
  await first.getByRole("button",{name:"Play output for sample.mp4"}).click();
  await first.getByRole("button",{name:"Show output for sample.mp4"}).click();
  await page.getByRole("button",{name:"Cancel this job",exact:true}).click();
  await expect(page.locator(".batch-items article>strong")).toHaveText(["complete","cancelled"]);
});

test("UI consistency: SmartCut quality labels and output actions preserve export settings",async({page})=>{
  await mockDesktop(page);await openFixture(page);
  await page.getByRole("button",{name:"SMARTCUT",exact:true}).click();
  const quality=page.getByRole("combobox",{name:"QUALITY",exact:true});
  await expect(quality).toHaveValue("preserve");
  await expect(quality.locator("option:checked")).toHaveText("Very high");
  await expect(quality.locator("option")).toHaveText(["Lossless","Very high","High · CRF 18","Medium · CRF 22","Small · CRF 26"]);
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="export_autocut"?{output:"C:\\fixtures\\smartcut.mp4"}:undefined});
  await page.getByRole("button",{name:"DETECT SILENCES",exact:true}).click();
  await page.getByRole("button",{name:"EXPORT MP4",exact:true}).click();
  await expect(page.locator(".output-actions button")).toHaveText(["play","show in folder","Open in editor"]);
  await page.getByRole("button",{name:"play",exact:true}).click();
  await page.getByRole("button",{name:"show in folder",exact:true}).click();
  expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.find((call:any)=>call.cmd==="export_autocut").args.request.quality)).toBe("preserve");
});

test("UI consistency: custom modal keyboard focus cannot escape into the editor",async({page})=>{
  await mockDesktop(page);
  const opener=page.getByRole("button",{name:"Clean CONTAINER Output",exact:true});
  await opener.click();
  const buttons=page.getByRole("dialog").getByRole("button");
  await buttons.last().focus();await page.keyboard.press("Tab");
  await expect(buttons.first()).toBeFocused();
  await page.keyboard.press("Shift+Tab");await expect(buttons.last()).toBeFocused();
  await buttons.first().click();await expect(opener).toBeFocused();
});

test("Image Compressor supports PNG targets, exposes safe palette mode and lays out size estimates",async({page},testInfo)=>{
  const png=resolve(testInfo.outputDir,"source.png");mkdirSync(testInfo.outputDir,{recursive:true});
  expect(spawnSync("ffmpeg",["-v","error","-y","-f","lavfi","-i","testsrc2=s=1254x1254","-frames:v","1",png]).status).toBe(0);
  await page.route("http://asset.localhost/**",route=>route.fulfill({contentType:"image/png",body:readFileSync(png)}));
  await mockDesktop(page,{fixture:{...sample,path:"C:\\fixtures\\source.png",name:"source.png",kind:"image",width:1254,height:1254,codec:"png",size:1_800_000}});
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="estimate_image_compression"?Number(args.request.params.quality)*1024:undefined});
  await openFixture(page);
  await page.getByPlaceholder("search tools...").fill("Image Compressor");
  await page.locator(".tool-row").click();
  await expect(page.locator(".image-size-summary")).toContainText("Estimated output");
  await expect(page.locator(".image-size-summary")).toContainText("82.00 KB");
  await expect(page.locator(".image-size-summary")).toContainText("quality percentages do not apply");
  await expect(page.getByRole("spinbutton",{name:/^Quality/,exact:false})).toHaveCount(0);
  await page.getByRole("combobox",{name:"Compression mode",exact:true}).selectOption("target");
  await expect(page.locator(".image-size-summary")).toContainText("Target size");
  await expect(page.locator(".image-size-summary")).not.toContainText("Estimated output");
  const render=page.getByRole("button",{name:/render image compressor/i});
  await expect(render).toBeEnabled();
  await expect(page.getByRole("combobox",{name:"PNG compression",exact:true})).toHaveValue("lossless");
  await page.getByRole("combobox",{name:"PNG compression",exact:true}).selectOption("palette");
  await expect(page.locator(".image-size-summary")).toContainText("not fully lossless");
  await page.getByRole("combobox",{name:"Output format",exact:true}).selectOption("webp");
  await expect(render).toBeEnabled();
  await page.getByRole("combobox",{name:"Compression mode",exact:true}).selectOption("quality");
  await page.getByRole("spinbutton",{name:/^Quality/,exact:false}).fill("62");
  await expect(page.locator(".image-size-summary")).toContainText("62.00 KB");
  for(const theme of ["dark","light"]){
    await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
    for(const width of [1440,900]){
      await page.setViewportSize({width,height:768});
      await page.locator(".field-list").evaluate(el=>el.scrollTop=el.scrollHeight);
      const rows=await page.locator(".image-size-summary>div").evaluateAll(rows=>rows.map(row=>{const label=row.querySelector("span")!.getBoundingClientRect(),value=row.querySelector("b")!.getBoundingClientRect();return {labelRight:label.right,valueLeft:value.left,valueRight:value.right,rowRight:row.getBoundingClientRect().right}}));
      for(const row of rows){expect(row.labelRight).toBeLessThanOrEqual(row.valueLeft);expect(row.valueRight).toBeLessThanOrEqual(row.rowRight+1)}
      await page.screenshot({path:`test-results/image-compressor-${theme}-${width}.png`});
    }
  }
});

for(const pngMode of ["lossless","palette"]){
  test(`PNG target render and Batch preserve ${pngMode} settings without blocking`,async({page})=>{
    await mockDesktop(page,{fixture:{...sample,path:"C:\\fixtures\\source.png",name:"source.png",kind:"image",codec:"png",size:1_800_000}});
    await page.goto("/");
    await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="run_operation"?{output:"C:\\fixtures\\compressed.png",elapsed:.1}:undefined});
    await openFixture(page);
    await page.getByPlaceholder("search tools...").fill("Image Compressor");await page.locator(".tool-row").click();
    await page.getByRole("combobox",{name:"Compression mode",exact:true}).selectOption("target");
    await page.getByRole("combobox",{name:"PNG compression",exact:true}).selectOption(pngMode);
    await page.getByRole("button",{name:/render image compressor/i}).click();
    await expect(page.locator(".job-head p")).toHaveText("complete");
    await expect(page.locator(".image-size-summary")).toContainText("target was not reached");
    expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").at(-1).args.request.params.png_mode)).toBe(pngMode);
    await page.getByRole("button",{name:"BATCH",exact:true}).click();
    const controls=page.locator(".batch-control");
    await controls.getByRole("combobox",{name:"Compression mode",exact:true}).selectOption("target");
    await controls.getByRole("combobox",{name:"PNG compression",exact:true}).selectOption(pngMode);
    await page.getByRole("button",{name:/START QUEUE/}).click();
    await expect(page.locator(".batch-items article > strong")).toHaveText("complete");
    expect(await page.evaluate(()=>(window as any).__TEST_CALLS__.filter((call:any)=>call.cmd==="run_operation").at(-1).args.request.params.png_mode)).toBe(pngMode);
  });
}

test("Image Compressor reports failed estimates and ignores a stale response",async({page})=>{
  await mockDesktop(page,{fixture:{...sample,path:"C:\\fixtures\\source.jpg",name:"source.jpg",kind:"image",codec:"mjpeg"}});
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string)=>cmd==="estimate_image_compression"?Promise.reject(new Error("estimate unavailable")):undefined});
  await openFixture(page);await page.getByPlaceholder("search tools...").fill("Image Compressor");await page.locator(".tool-row").click();
  await expect(page.locator(".image-size-summary")).toContainText("Unavailable");
  await page.evaluate(()=>{(window as any).__TEST_HANDLER__=(cmd:string,args:any)=>cmd==="estimate_image_compression"?(args.request.params.quality==="82"?new Promise(resolve=>(window as any).__OLD_ESTIMATE__=resolve):42_000):undefined});
  await page.getByRole("button",{name:"Retry estimate",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>!!(window as any).__OLD_ESTIMATE__)).toBe(true);
  await page.getByRole("spinbutton",{name:/^Quality/,exact:false}).fill("42");
  await expect(page.locator(".image-size-summary")).toContainText("41.02 KB");
  await page.evaluate(()=>(window as any).__OLD_ESTIMATE__(100_000));
  await expect(page.locator(".image-size-summary")).toContainText("41.02 KB");
});

test("Color sliders work directly, reset to neutral and support undo on images",async({page},testInfo)=>{
  const png=resolve(testInfo.outputDir,"source.png");mkdirSync(testInfo.outputDir,{recursive:true});
  expect(spawnSync("ffmpeg",["-v","error","-y","-f","lavfi","-i","testsrc2=s=1254x1254","-frames:v","1",png]).status).toBe(0);
  await page.route("http://asset.localhost/**",route=>route.fulfill({contentType:"image/png",body:readFileSync(png)}));
  await mockDesktop(page,{fixture:{...sample,path:"C:\\fixtures\\source.png",name:"source.png",kind:"image",codec:"png"}});
  await openFixture(page);await page.getByPlaceholder("search tools...").fill("Color Adjustment");await page.locator(".tool-row").click();
  await expect(page.locator(".color-workspace input[type=checkbox]")).toHaveCount(0);
  const brightness=page.getByRole("slider",{name:"Brightness",exact:true});
  const sharpen=page.getByRole("slider",{name:"Sharpen",exact:true});
  await expect(sharpen).toHaveValue("0");await expect(brightness).toBeEnabled();
  await brightness.fill("20");
  await page.waitForTimeout(400);
  await sharpen.fill("15");await page.waitForTimeout(400);
  await page.locator(".tool-settings-head h2").click();await page.keyboard.press("Control+z");
  await expect(sharpen).toHaveValue("0");await expect(brightness).toHaveValue("20");
  await page.getByRole("button",{name:"Reset Brightness",exact:true}).click();await expect(brightness).toHaveValue("0");
  await page.getByRole("button",{name:"Warm",exact:true}).click();await expect(page.getByRole("slider",{name:"Temperature",exact:true})).toHaveValue("5000");
  await page.getByRole("button",{name:"Reset",exact:true}).click();await expect(page.getByRole("slider",{name:"Temperature",exact:true})).toHaveValue("6500");
  await expect(sharpen).toHaveValue("0");
  await page.screenshot({path:"test-results/image-color-direct-sliders.png"});
});

test("Download from link uses a balanced rectangular button in both themes",async({page})=>{
  await mockDesktop(page);
  for(const theme of ["dark","light"]){await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);const button=page.locator(".downloader-quick-trigger");await expect(button).toContainText("Download from link");expect(await button.evaluate(el=>getComputedStyle(el).borderRadius)).toBe("7px");const box=(await button.boundingBox())!;expect(box.height).toBeGreaterThanOrEqual(36);await page.screenshot({path:`test-results/landing-download-button-${theme}.png`});}
});

test("Landing engine status is centered in the bottom action row without collisions",async({page})=>{
  await mockDesktop(page);await page.goto("/");
  for(const size of [{width:1920,height:1000},{width:900,height:600},{width:480,height:570}]){
    await page.setViewportSize(size);
    const status=page.locator(".landing-engine-status");await expect(status).toBeVisible();
    const a=(await status.boundingBox())!,b=(await page.locator(".downloader-quick-trigger").boundingBox())!;
    expect(Math.abs(a.x+a.width/2-size.width/2)).toBeLessThan(2);
    if(size.width>650)expect(Math.abs(a.y+a.height/2-b.y-b.height/2)).toBeLessThan(3);
    expect(a.x>=b.x+b.width||a.y+a.height<=b.y||a.y>=b.y+b.height).toBe(true);
  }
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
