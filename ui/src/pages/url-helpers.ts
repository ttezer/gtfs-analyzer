// #45 saf yardımcıları — render/worker importundan bağımsız → doğrudan birim testlenebilir.

export type UrlCheck =
  | { ok: true; url: URL }
  | { ok: false; reason: 'invalid' | 'https' | 'credentials' };

/**
 * Feed URL'sini client-side (ağ yok) doğrular: geçerli URL, yalnız `https`,
 * gömülü kimlik bilgisi (user:pass@) reddedilir. SSRF/gizlilik için ilk savunma.
 */
export function validateFeedUrl(raw: string): UrlCheck {
  let url: URL;
  try { url = new URL(raw.trim()); }
  catch { return { ok: false, reason: 'invalid' }; }
  if (url.protocol !== 'https:') return { ok: false, reason: 'https' };
  if (url.username || url.password) return { ok: false, reason: 'credentials' };
  return { ok: true, url };
}

/** URL yolundan okunabilir dosya adı türet (uzantı yoksa `.zip` ekle; yoksa `feed.zip`). */
export function urlFileName(url: URL): string {
  const last = url.pathname.split('/').filter(Boolean).pop() ?? '';
  let decoded = last;
  try { decoded = decodeURIComponent(last); } catch { /* ham bırak */ }
  if (!decoded) return 'feed.zip';
  return /\.[a-z0-9]+$/i.test(decoded) ? decoded : `${decoded}.zip`;
}

/** İlk 4 bayt yerel ZIP imzası mı? (PK\x03\x04) */
export function isZipSignature(bytes: Uint8Array): boolean {
  return bytes.length >= 4 && bytes[0] === 0x50 && bytes[1] === 0x4B && bytes[2] === 0x03 && bytes[3] === 0x04;
}
