import { describe, expect, it } from 'vitest';
import { tFatal } from '../i18n';
import type { FatalError } from '../types';

// The fatal message was the pipeline's Turkish text; the UI translated the heading but showed
// the message below it unchanged. It now uses the same `{code}.{variant}` template model as notices.
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

  it('translates a worker-side config parse error in all locales', () => {
    const config: FatalError = {
      code: 'InvalidInput',
      message: 'Unexpected end of JSON input',
      params: {
        variant: 'config_parse',
        detail: 'Unexpected end of JSON input',
      },
    };
    expect(tFatal(config, 'en')).toBe('Config parse error: Unexpected end of JSON input');
    expect(tFatal(config, 'fr')).toBe("Erreur d’analyse de la configuration : Unexpected end of JSON input");
    expect(tFatal(config, 'ja')).toBe('設定の解析エラー：Unexpected end of JSON input');
    expect(tFatal(config, 'tr')).toBe('Config ayrıştırma hatası: Unexpected end of JSON input');
  });
});
