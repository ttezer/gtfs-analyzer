import { describe, expect, it } from 'vitest';
import { tFatal } from '../i18n';
import type { FatalError } from '../types';

// Fatal mesajı pipeline'ın Türkçe metniydi; UI başlığı çeviriyor ama altındaki mesajı
// olduğu gibi gösteriyordu. Artık bildirimlerle aynı modelde `{code}.{variant}` şablonundan.
const zip: FatalError = {
  code: 'ZipUnreadable',
  message: "'stops.txt' okunamadı: bad crc",
  params: { variant: 'entry_read', file: 'stops.txt', detail: 'bad crc' },
};

describe('tFatal', () => {
  it('fills the template in the requested locale', () => {
    expect(tFatal(zip, 'en')).toBe("'stops.txt' could not be read: bad crc");
    expect(tFatal(zip, 'fr')).toBe('« stops.txt » n’a pas pu être lu : bad crc');
    expect(tFatal(zip, 'ja')).toBe("'stops.txt'を読み取れませんでした：bad crc");
  });

  it('keeps the pipeline text for Turkish, unknown variants and old payloads', () => {
    expect(tFatal(zip, 'tr')).toBe(zip.message);
    expect(tFatal({ ...zip, params: { variant: 'no_such_variant' } }, 'en')).toBe(zip.message);
    expect(tFatal({ code: 'ZipUnreadable', message: 'eski' }, 'en')).toBe('eski');
  });
});
