import { expect, test, type Page } from "@playwright/test";

const sample = {
  path: "C:\\fixtures\\sample.mp4", name: "sample.mp4", kind: "video", duration: 60,
  width: 1920, height: 1080, fps: 30, codec: "h264", audio_codec: "aac",
  audio_tracks: [], pixel_format: "yuv420p", bits_per_raw_sample: 8,
  color_transfer: null, color_primaries: null, color_space: null,
  bitrate: 3_000_000, size: 1_000_000, start_timecode: null,
};

async function mockDesktop(page: Page, options: { invalidSecond?: boolean; interrupted?: boolean; startupPath?: string | null; savedRecovery?: boolean; missingProjectSource?: boolean } = {}) {
  await page.addInitScript(({ media, invalidSecond, interrupted, startupPath, savedRecovery, missingProjectSource }) => {
    const callbacks = new Map<number, (...args: any[]) => void>();
    const events = new Map<string, number>();
    let callbackId = 0;
    let fileOpens = 0;
    const mock: Record<string, any> = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
      transformCallback(callback: (...args: any[]) => void) { const id = ++callbackId; callbacks.set(id, callback); return id; },
      unregisterCallback(id: number) { callbacks.delete(id); },
      convertFileSrc(path: string) { return `http://asset.localhost/${encodeURIComponent(path)}`; },
      async invoke(cmd: string, args: Record<string, any> = {}) {
        if (cmd === "plugin:event|listen") { events.set(args.event, args.handler); return ++callbackId; }
        if (cmd === "plugin:event|unlisten") return null;
        if (cmd === "plugin:dialog|open") {
          fileOpens += 1;
          return invalidSecond && fileOpens > 1 ? "C:\\fixtures\\broken.mp4" : media.path;
        }
        if (cmd === "plugin:dialog|save") return "C:\\fixtures\\sample.containerproject";
        if (cmd === "plugin:dialog|confirm") return false;
        if (cmd === "plugin:app|version") return "0.15.0-dev.1";
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
  }, { media: sample, ...options });
  await page.goto("/");
  if (!options.startupPath) await expect(page.locator(".dropzone")).toBeVisible();
}

async function openFixture(page: Page) {
  await page.locator(".dropzone").click();
  await expect(page.locator(".settings")).toBeVisible();
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
