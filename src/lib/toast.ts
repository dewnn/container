import { invoke } from "@tauri-apps/api/core";

export interface ToastDetail {
  message: string;
  kind?: "error" | "info" | "success";
}

export function reportProblem(reason: unknown): void {
  const message = reason instanceof Error ? reason.message : String(reason ?? "");
  if (!message || /cancel(?:led|ed)?/i.test(message)) return;
  void invoke("log_ui_error", { message }).catch(() => {});
  window.dispatchEvent(new CustomEvent<ToastDetail>("container-toast", {
    detail: { message, kind: "error" }
  }));
}
