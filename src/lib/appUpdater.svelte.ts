// In-app updater state: checking GitHub for a new release, downloading it and
// restarting. Development builds never update (see releaseChannel.ts).
import { getVersion } from "@tauri-apps/api/app";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { updatesAllowedForVersion } from "./releaseChannel";
import { reportProblem } from "./toast";

export class AppUpdater {
  version = $state("");
  available: Update | null = $state(null);
  panelOpen = $state(false);
  checking = $state(false);
  installing = $state(false);
  status = $state("");
  downloaded = $state(0);
  total = $state(0);
  readonly enabled = $derived(updatesAllowedForVersion(this.version));
  readonly devVersion = $derived(/-dev(?:\.|$)/i.test(this.version) ? this.version : "");

  constructor(private readonly language: () => "tr" | "en") {}

  async loadVersion() {
    this.version = await getVersion().catch(() => this.version);
  }

  async check(manual = true) {
    if (this.checking || this.installing) return;
    if (!this.version) this.version = await getVersion().catch(() => "");
    if (!updatesAllowedForVersion(this.version)) return;
    const tr = this.language() === "tr";
    if (manual) this.panelOpen = true;
    this.checking = true;
    this.status = tr ? "Güncellemeler denetleniyor…" : "Checking for updates…";
    try {
      const result = await check({ timeout: 15000 });
      if (this.available && this.available !== result) await this.available.close().catch(() => {});
      this.available = result;
      this.status = result
        ? (tr ? `CONTAINER ${result.version} hazır.` : `CONTAINER ${result.version} is available.`)
        : (tr ? "CONTAINER güncel." : "CONTAINER is up to date.");
      if (result) this.panelOpen = true;
    } catch (reason) {
      this.status = tr ? "Güncelleme denetlenemedi. İnternet bağlantını kontrol et." : "Could not check for updates. Check your internet connection.";
      if (!manual) this.panelOpen = false;
      if (manual) reportProblem(this.status);
      console.warn("Update check failed", reason);
    } finally {
      this.checking = false;
    }
  }

  async install() {
    const update = this.available;
    if (!update || this.installing) return;
    const tr = this.language() === "tr";
    this.installing = true;
    this.downloaded = 0;
    this.total = 0;
    this.status = tr ? "Güncelleme indiriliyor…" : "Downloading update…";
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") this.total = event.data.contentLength ?? 0;
        if (event.event === "Progress") this.downloaded += event.data.chunkLength;
        if (event.event === "Finished") this.status = tr ? "Güncelleme kuruluyor; CONTAINER yeniden başlayacak…" : "Installing update; CONTAINER will restart…";
      }, { timeout: 300000, restartAfterInstall: true });
    } catch (reason) {
      this.status = tr ? `Güncelleme kurulamadı: ${String(reason)}` : `Update could not be installed: ${String(reason)}`;
      reportProblem(this.status);
      this.installing = false;
    }
  }
}
