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
  it('rejects local and private network destinations (security review #4)', () => {
    for (const u of [
      'https://localhost/x.zip', 'https://router/x.zip', 'https://nas.local/x.zip',
      'https://127.0.0.1/x.zip', 'https://2130706433/x.zip', 'https://0x7f.1/x.zip',
      'https://10.1.2.3/x.zip', 'https://172.16.0.1/x.zip', 'https://192.168.1.1/x.zip',
      'https://169.254.169.254/x.zip', 'https://100.64.0.1/x.zip', 'https://0.0.0.0/x.zip',
      'https://[::1]/x.zip', 'https://[fd00::1]/x.zip', 'https://[fe80::1]/x.zip',
      'https://[::ffff:192.168.1.1]/x.zip',
    ]) {
      expect(validateFeedUrl(u), u).toEqual({ ok: false, reason: 'private' });
    }
  });
  it('still accepts public hosts and public IP literals', () => {
    for (const u of ['https://8.8.8.8/x.zip', 'https://172.32.0.1/x.zip', 'https://[2001:db8::1]/x.zip',
      'https://files.mobilitydatabase.org/x.zip']) {
      expect(validateFeedUrl(u).ok, u).toBe(true);
    }
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
