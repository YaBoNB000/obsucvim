/**
 * Browser-readable session tokens have been retired. Sessions now use an
 * HttpOnly same-origin cookie; only clear legacy demo/auth keys on migration.
 */
const LEGACY_STORAGE_KEYS = [
  "obsucvim.token",
  "obsucvim.user",
  "obsucvim.users",
  "obsucvim.authlock",
  "obsucvim.quota",
];

export function clearLegacyAuthData() {
  try {
    for (const key of LEGACY_STORAGE_KEYS) localStorage.removeItem(key);
  } catch {
    /* storage may be unavailable in privacy modes */
  }
}
