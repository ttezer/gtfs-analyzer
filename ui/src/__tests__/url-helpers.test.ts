import { describe, expect, it } from 'vitest';
import { validateFeedUrl, urlFileName, isZipSignature } from '../pages/url-helpers';

describe('validateFeedUrl', () => {
  it('accepts a valid https URL', () => {
    const r = validateFeedUrl('https://files.mobilitydatabase.org/mdb-247/x.zip');
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.url.host).toBe('files.mobilitydatabase.org');
  });
  it('trims surrounding whitespace', () => {
    expect(validateFeedUrl('  https://example.org/gtfs.zip  ').ok).toBe(true);
  });
  it('rejects non-https (http)', () => {
    expect(validateFeedUrl('http://example.org/gtfs.zip')).toEqual({ ok: false, reason: 'https' });
  });
  it('rejects non-https (ftp/file)', () => {
    expect(validateFeedUrl('ftp://example.org/gtfs.zip').ok).toBe(false);
    expect(validateFeedUrl('file:///etc/passwd')).toEqual({ ok: false, reason: 'https' });
  });
  it('rejects embedded credentials (SSRF/leak guard)', () => {
    expect(validateFeedUrl('https://user:pass@example.org/gtfs.zip')).toEqual({ ok: false, reason: 'credentials' });
    expect(validateFeedUrl('https://user@example.org/gtfs.zip')).toEqual({ ok: false, reason: 'credentials' });
  });
  it('rejects malformed input', () => {
    expect(validateFeedUrl('not a url')).toEqual({ ok: false, reason: 'invalid' });
    expect(validateFeedUrl('')).toEqual({ ok: false, reason: 'invalid' });
  });
});

describe('urlFileName', () => {
  const name = (u: string) => urlFileName(new URL(u));
  it('keeps a .zip filename', () => {
    expect(name('https://x.org/path/gtfs.zip')).toBe('gtfs.zip');
  });
  it('keeps any explicit extension', () => {
    expect(name('https://x.org/feed.gtfs')).toBe('feed.gtfs');
  });
  it('appends .zip when the last segment has no extension', () => {
    expect(name('https://x.org/mdb-247-202606260041')).toBe('mdb-247-202606260041.zip');
  });
  it('falls back to feed.zip for a bare host/root', () => {
    expect(name('https://x.org/')).toBe('feed.zip');
    expect(name('https://x.org')).toBe('feed.zip');
  });
  it('ignores query string, decodes percent-encoding', () => {
    expect(name('https://x.org/a/my%20feed.zip?rev=2')).toBe('my feed.zip');
  });
});

describe('isZipSignature', () => {
  it('true for the local ZIP magic PK\\x03\\x04', () => {
    expect(isZipSignature(new Uint8Array([0x50, 0x4B, 0x03, 0x04, 0x14]))).toBe(true);
  });
  it('false for non-zip bytes', () => {
    expect(isZipSignature(new Uint8Array([0x25, 0x50, 0x44, 0x46]))).toBe(false); // %PDF
  });
  it('false when fewer than 4 bytes', () => {
    expect(isZipSignature(new Uint8Array([0x50, 0x4B]))).toBe(false);
  });
});
