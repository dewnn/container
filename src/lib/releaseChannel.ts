export function updatesAllowedForVersion(version: string): boolean {
  return /^\d+\.\d+\.\d+$/.test(version.trim());
}
