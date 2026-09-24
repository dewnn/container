// Export assets are derived from the same SVGs as the preview. Run after changing
// either SVG: node scripts/generate-social-tag-marks.mjs
import { chromium } from "@playwright/test";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";

const output = new URL("../src-tauri/resources/social-tags/", import.meta.url);
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  const manifest = {};
  for (const [name, source, color, width, height] of [
    ["kick-plain", "kick", "#53fc19", 768, 864],
    ["kick-boxed", "kick", "#050805", 768, 864],
    ["twitch", "twitch", "#9146FF", 768, 768],
  ]) {
    const svg = (await readFile(new URL(`../src/assets/${source}-mark.svg`, import.meta.url), "utf8")).replace(/\r\n/g, "\n");
    const data = await page.evaluate(async ({ svg, color, width, height }) => {
      const image = new Image();
      image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg.replace(/fill="#[\da-f]+"/gi, `fill="${color}"`))}`;
      await image.decode();
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      canvas.getContext("2d").drawImage(image, 0, 0, width, height);
      return canvas.toDataURL("image/png").split(",")[1];
    }, { svg, color, width, height });
    const png = Buffer.from(data, "base64");
    await writeFile(new URL(`${name}.png`, output), png);
    manifest[name] = {
      source: `${source}-mark.svg`,
      sourceSha256: createHash("sha256").update(svg).digest("hex"),
      pngSha256: createHash("sha256").update(png).digest("hex"),
    };
  }
  await writeFile(new URL("manifest.json", output), `${JSON.stringify(manifest, null, 2)}\n`);
} finally {
  await browser.close();
}
