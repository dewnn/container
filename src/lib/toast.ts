export interface ToastDetail {
  message: string;
  kind?: "error" | "info";
}

export function reportProblem(reason: unknown): void {
  const message = reason instanceof Error ? reason.message : String(reason ?? "");
  if (!message || /cancel(?:led|ed)?/i.test(message)) return;
  window.dispatchEvent(new CustomEvent<ToastDetail>("container-toast", {
    detail: { message, kind: "error" }
  }));
}
