// #45 pure helpers — independent of render/worker imports → directly unit-testable.

export type UrlCheck =
  | { ok: true; url: URL }
  | { ok: false; reason: 'invalid' | 'https' | 'credentials' | 'private' };

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
  if (isPrivateHost(url.hostname)) return { ok: false, reason: 'private' };
  return { ok: true, url };
}

/**
 * Local, private and link-local destinations (security review 11-12, #4): a shared `?url=`
 * link must not make the visitor's browser fetch their router or intranet. `URL` already
 * normalises IPv4 spellings (`2130706433`, `0x7f.1` → `127.0.0.1`) and brackets IPv6.
 * Hostnames that only RESOLVE to a private address cannot be seen from the browser; the
 * browser's own Private Network Access checks cover those.
 */
export function isPrivateHost(hostname: string): boolean {
  const h = hostname.toLowerCase().replace(/\.$/, '');
  if (h === 'localhost' || h.endsWith('.localhost') || h.endsWith('.local')
    || h.endsWith('.internal') || h.endsWith('.home.arpa') || !h.includes('.') && !h.startsWith('[')) {
    return true;
  }
  const v4 = h.match(/^(\d+)\.(\d+)\.(\d+)\.(\d+)$/);
  if (v4) return isPrivateV4(v4.slice(1).map(Number));
  if (h.startsWith('[')) {
    const v6 = h.slice(1, -1);
    if (v6 === '::' || v6 === '::1') return true;
    const mapped = v6.match(/^::ffff:(\d+\.\d+\.\d+\.\d+)$/) ?? null;
    if (mapped) return isPrivateV4(mapped[1].split('.').map(Number));
    const hex = v6.match(/^::ffff:([0-9a-f]{1,4}):([0-9a-f]{1,4})$/);
    if (hex) {
      const hi = parseInt(hex[1], 16), lo = parseInt(hex[2], 16);
      return isPrivateV4([hi >> 8, hi & 255, lo >> 8, lo & 255]);
    }
    const first = parseInt(v6.split(':')[0] || '0', 16);
    return (first & 0xfe00) === 0xfc00 || (first & 0xffc0) === 0xfe80; // fc00::/7, fe80::/10
  }
  return false;
}

function isPrivateV4([a, b]: number[]): boolean {
  return a === 0 || a === 10 || a === 127 || a === 169 && b === 254
    || a === 172 && b >= 16 && b <= 31 || a === 192 && b === 168 || a === 100 && b >= 64 && b <= 127;
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
