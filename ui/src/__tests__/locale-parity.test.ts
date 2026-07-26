import { describe, it, expect } from 'vitest';
import tr from '../locales/tr';
import en from '../locales/en';
import ja from '../locales/ja';
import enExport from '../../../crates/cli/locales/en.json';
import jaExport from '../../../crates/cli/locales/ja.json';

const keys = (o: Record<string, string>): Set<string> => new Set(Object.keys(o));
const minus = (a: Set<string>, b: Set<string>): string[] => [...a].filter((x) => !b.has(x));

describe('locale parity (registry kural anahtarları)', () => {
  it('ruleTitles anahtarları tr/en/ja arasında birebir aynı', () => {
    const t = keys(tr.ruleTitles);
    const e = keys(en.ruleTitles);
    const j = keys(ja.ruleTitles);
    expect(minus(t, e)).toEqual([]); // tr'de olup en'de olmayan
    expect(minus(e, t)).toEqual([]); // en'de olup tr'de olmayan
    expect(minus(e, j)).toEqual([]); // en'de olup ja'da olmayan
    expect(minus(j, e)).toEqual([]); // ja'da olup en'de olmayan (orphan → RTS_015 sınıfı)
  });

  it('ruleMessages anahtarları en/ja arasında birebir aynı (tr bilinçli boş)', () => {
    expect(Object.keys(tr.ruleMessages)).toHaveLength(0);
    const e = keys(en.ruleMessages);
    const j = keys(ja.ruleMessages);
    expect(minus(e, j)).toEqual([]);
    expect(minus(j, e)).toEqual([]);
  });

  it('ruleRemediations anahtarları en/ja arasında birebir aynı (tr bilinçli boş)', () => {
    expect(Object.keys(tr.ruleRemediations)).toHaveLength(0);
    const e = keys(en.ruleRemediations);
    const j = keys(ja.ruleRemediations);
    expect(minus(e, j)).toEqual([]); // en'de olup ja'da olmayan
    expect(minus(j, e)).toEqual([]); // ja'da olup en'de olmayan (orphan)
  });
});

// Rust CLI'nin `--lang` çıktısı bu locale'lerden TÜRETİLİR: `npm run locales:export`
// crates/cli/locales/{en,ja}.json üretir, CLI onu include_str! ile gömer. Locale
// güncellenip export çalıştırılmazsa CLI eski metni yayınlar — bu test onu yakalar.
describe('CLI locale export (crates/cli/locales)', () => {
  it.each([
    ['en', en, enExport],
    ['ja', ja, jaExport],
  ] as const)('%s.json locale ile aynı (bayat ise: npm run locales:export)', (_lang, locale, exported) => {
    expect(exported.messages).toEqual(locale.ruleMessages);
    expect(exported.remediations).toEqual(locale.ruleRemediations);
    expect(exported.titles).toEqual(locale.ruleTitles);
  });
});
