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

  it('translates browser-side resource limits, with and without the last stage', () => {
    const memory: FatalError = {
      code: 'ResourceLimit',
      message: 'The feed is too large for the browser: WASM ran out of memory. Try a smaller feed. (unreachable)',
      params: { variant: 'browser_memory', detail: 'unreachable' },
    };
    expect(tFatal(memory, 'en')).toBe('The feed is too large for the browser: WASM ran out of memory. Try a smaller feed. (unreachable)');
    expect(tFatal(memory, 'tr')).toBe('Feed tarayıcı için çok büyük: WASM belleği yetersiz kaldı. Daha küçük bir feed deneyin. (unreachable)');

    const timeout: FatalError = { code: 'ResourceLimit', message: 'x', params: { variant: 'timeout', minutes: '15' } };
    expect(tFatal(timeout, 'en')).toBe('Validation did not finish within 15 minutes. Try a smaller feed.');
    const timeoutAt: FatalError = {
      code: 'ResourceLimit', message: 'x', params: { variant: 'timeout_at_stage', minutes: '15', stage: 'K6' },
    };
    expect(tFatal(timeoutAt, 'ja')).toBe('検証が 15 分以内に完了しませんでした。より小さなフィードでお試しください。（最後に完了した段階：K6）');
    expect(tFatal(timeoutAt, 'fr')).toBe('La validation ne s’est pas terminée en 15 minutes. Essayez un jeu de données plus petit. (dernière étape terminée : K6)');

    const crash: FatalError = {
      code: 'ResourceLimit', message: 'x', params: { variant: 'worker_crash_at_stage', detail: 'RuntimeError', stage: 'K2' },
    };
    expect(tFatal(crash, 'tr')).toBe('Arka plan doğrulama işçisi beklenmedik şekilde durdu. RuntimeError (son tamamlanan aşama: K2)');
  });
});
