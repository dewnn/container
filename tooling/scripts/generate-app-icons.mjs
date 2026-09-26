// Render the original vector mark with a rounded plate; never alter project.ico.
// Requires the existing Playwright Chromium installation. --check makes no writes.
import { readFile, writeFile } from "node:fs/promises";
import { chromium } from "@playwright/test";

const root = new URL("../../", import.meta.url);
const check = process.argv.includes("--check");
const browser = await chromium.launch();
const sizes = [16, 24, 32, 48, 64, 128, 256];
async function output(relative, bytes) {
  const path = new URL(relative, root);
  if (check) {
    if (!bytes.equals(await readFile(path))) throw new Error(`Stale icon: ${relative}`);
  } else await writeFile(path, bytes);
}

// 32-bit Windows DIB frames plus explicit transparency masks, including small sizes.
function ico(frames) {
  const header = Buffer.alloc(6 + 16 * frames.length);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(frames.length, 4);
  const images = [];
  let offset = header.length;
  frames.forEach(({ size, rgba }, index) => {
    const maskStride = Math.ceil(size / 32) * 4;
    const pixels = size * size * 4;
    const image = Buffer.alloc(40 + pixels + maskStride * size);
    image.writeUInt32LE(40, 0);
    image.writeInt32LE(size, 4);
    image.writeInt32LE(size * 2, 8);
    image.writeUInt16LE(1, 12);
    image.writeUInt16LE(32, 14);
    image.writeUInt32LE(pixels, 20);
    for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
      const from = (y * size + x) * 4;
      const to = 40 + ((size - y - 1) * size + x) * 4;
      image.set([rgba[from + 2], rgba[from + 1], rgba[from], rgba[from + 3]], to);
      if (!rgba[from + 3]) image[40 + pixels + (size - y - 1) * maskStride + (x >> 3)] |= 0x80 >> (x % 8);
    }
    const entry = 6 + index * 16;
    header[entry] = header[entry + 1] = size === 256 ? 0 : size;
    header.writeUInt16LE(1, entry + 4);
    header.writeUInt16LE(32, entry + 6);
    header.writeUInt32LE(image.length, entry + 8);
    header.writeUInt32LE(offset, entry + 12);
    offset += image.length;
    images.push(image);
  });
  return Buffer.concat([header, ...images]);
}

try {
  const page = await browser.newPage();
  for (const theme of ["dark", "light"]) {
    const source = theme === "dark" ? "container.svg" : "container-light.svg";
    const svg = await readFile(new URL(`src-tauri/icons/${source}`, root), "utf8");
    const frames = await page.evaluate(async ({ svg, sizes }) => {
      const image = new Image();
      image.src = "data:image/svg+xml;base64," + btoa(svg);
      await image.decode();
      return [...sizes, 512].map(size => {
        const canvas = document.createElement("canvas");
        canvas.width = canvas.height = size;
        const ctx = canvas.getContext("2d");
        ctx.drawImage(image, 0, 0, size, size);
        const rgba = [...ctx.getImageData(0, 0, size, size).data];
        for (const [x, y] of [[0, 0], [size - 1, 0], [0, size - 1], [size - 1, size - 1]]) {
          if (rgba[(y * size + x) * 4 + 3] !== 0) throw new Error(`Icon ${size}px corner ${x},${y} alpha=${rgba[(y * size + x) * 4 + 3]}`);
        }
        if (rgba[(Math.floor(size / 2) * size + Math.floor(size / 2)) * 4 + 3] !== 255) throw new Error("Icon center is not opaque");
        return { size, rgba, png: canvas.toDataURL("image/png").split(",")[1] };
      });
    }, { svg, sizes });
    await output(`src/public/logo-${theme}.png`, Buffer.from(frames.at(-1).png, "base64"));
    if (theme === "dark") {
      for (const [size, name] of [[32, "32x32"], [128, "128x128"], [256, "128x128@2x"]]) {
        await output(`src-tauri/icons/${name}.png`, Buffer.from(frames.find(frame => frame.size === size).png, "base64"));
      }
      const bytes = ico(frames.filter(frame => frame.size <= 256));
      await output("src-tauri/icons/icon.ico", bytes);
      await output("src-tauri/windows/setup-dark.ico", bytes);
    }
  }
  console.log(`${check ? "Verified" : "Generated"} rounded app/window/installer icons; project icon unchanged.`);
} finally {
  await browser.close();
}
