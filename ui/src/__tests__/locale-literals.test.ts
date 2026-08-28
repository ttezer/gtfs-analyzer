import { describe, it, expect } from 'vitest';

// Sayı/tarih biçimi tek kaynaktan gelmeli: `intlLocale()` (i18n.ts).
// Dağınık `toLocaleString('tr-TR')` çağrıları İngilizce ve Japonca arayüzde de
// Türk biçimi basıyordu (16 çağrı ölçüldü); Fransızca eklenince aynı hata
// binlik ayracı da bozacaktı (fr-FR ayracı U+202F). Bu kapı literalin geri
// sızmasını engeller. `node:fs` yerine Vite glob'u: projede @types/node yok.
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
