export type StartupAction = "open-path" | "offer-recovery" | "discard-recovery";

export function startupAction(path: string | null, interrupted: boolean): StartupAction {
  if (path) return "open-path";
  return interrupted ? "offer-recovery" : "discard-recovery";
}
