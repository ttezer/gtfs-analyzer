import { describe, it, expect } from 'vitest';

// Number/date formatting must have one source: `intlLocale()` (i18n.ts).
// Scattered `toLocaleString('tr-TR')` calls printed Turkish formatting in English and Japanese
// interfaces too (16 calls measured); adding French would have broken its thousands separator
// as well (fr-FR separator U+202F). This guard prevents the literal from leaking back in.
// Use Vite's glob instead of `node:fs`: the project has no @types/node.
const SOURCES = import.meta.glob('../**/*.ts', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

const ALLOWED = /\/i18n\.ts$/;
const BANNED = /'(tr-TR|en-US|ja-JP|fr-FR)'/;

describe('locale etiketleri tek kaynakta', () => {
  it('i18n.ts dışında BCP-47 literali yok', () => {
    const offenders = Object.entries(SOURCES)
      .filter(([path]) => !ALLOWED.test(path) && !path.includes('__tests__'))
      .flatMap(([path, text]) =>
        text
          .split('\n')
          .map((line, i) => ({ path, line: line.trim(), no: i + 1 }))
          .filter(({ line }) => BANNED.test(line)),
      )
      .map(({ path, no, line }) => `${path}:${no} ${line}`);
    expect(offenders).toEqual([]);
  });

  it('taranan kaynak sayısı makul (glob boş dönerse kapı sahte yeşil olur)', () => {
    expect(Object.keys(SOURCES).length).toBeGreaterThan(20);
  });
});
