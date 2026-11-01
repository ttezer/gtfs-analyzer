// #45 pure helpers — independent of render/worker imports → directly unit-testable.

export type UrlCheck =
  | { ok: true; url: URL }
  | { ok: false; reason: 'invalid' | 'https' | 'credentials' };

/**
 * Validate a feed URL client-side (without network access): require a valid URL and `https`,
 * and reject embedded credentials (user:pass@). This is the first defense against SSRF/privacy issues.
 */
export function validateFeedUrl(raw: string): UrlCheck {
  let url: URL;
  try { url = new URL(raw.trim()); }
  catch { return { ok: false, reason: 'invalid' }; }
  if (url.protocol !== 'https:') return { ok: false, reason: 'https' };
  if (url.username || url.password) return { ok: false, reason: 'credentials' };
  return { ok: true, url };
}

/** Derive a readable filename from the URL path (append `.zip` when there is no extension; use `feed.zip` when empty). */
export function urlFileName(url: URL): string {
  const last = url.pathname.split('/').filter(Boolean).pop() ?? '';
  let decoded = last;
  try { decoded = decodeURIComponent(last); } catch { /* ham bırak */ }
  if (!decoded) return 'feed.zip';
  return /\.[a-z0-9]+$/i.test(decoded) ? decoded : `${decoded}.zip`;
}

/** Do the first 4 bytes match the local ZIP signature? (PK\x03\x04) */
export function isZipSignature(bytes: Uint8Array): boolean {
  return bytes.length >= 4 && bytes[0] === 0x50 && bytes[1] === 0x4B && bytes[2] === 0x03 && bytes[3] === 0x04;
}
