import { copyFile, stat } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const release = path.join(root, "src-tauri", "target", "release");
const source = path.join(release, "container-studio.exe");
const destination = path.join(release, "container-studio-dev.exe");

await copyFile(source, destination);
const sourceStat = await stat(source);
const destinationStat = await stat(destination);
if (sourceStat.size !== destinationStat.size) {
  throw new Error("CONTAINER DEV executable verification failed after copy.");
}
console.log(`Synced CONTAINER DEV: ${destination}`);
