export function isTextEditingTarget(tagName: string | undefined, isContentEditable: boolean): boolean {
  return isContentEditable || ["INPUT", "SELECT", "TEXTAREA"].includes(tagName?.toUpperCase() ?? "");
}
