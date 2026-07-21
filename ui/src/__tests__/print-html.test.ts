import { describe, expect, it } from 'vitest';
import { withAutoPrint, withLang } from '../print-doc';

// Rapor sekmesi artık `noopener` ile açılıyor → pencere handle'ı YOK. Eskiden pencerede
// yapılan iki iş (documentElement.lang ataması ve win.print() çağrısı) bu yüzden belgenin
// kendisine taşındı. Aşağıdaki testler o taşımanın doğru yapıldığını sabitler.

const CLASSIC = '<!DOCTYPE html>\n<html lang="tr">\n<head><title>x</title></head>\n<body><p>rapor</p></body>\n</html>';
const EXECUTIVE = '<!doctype html>\n<html lang="ja">\n<head></head>\n<body><section>y</section></body>\n</html>';

describe('withLang', () => {
  it('replaces the lang of the first html tag, not the doctype', () => {
    // İlk yazımda regex `^`'a bağlıydı; <!DOCTYPE> önde olduğu için hiç eşleşmiyordu.
    expect(withLang(CLASSIC, 'en')).toContain('<html lang="en">');
    expect(withLang(CLASSIC, 'en')).not.toContain('lang="tr"');
    expect(withLang(CLASSIC, 'en').startsWith('<!DOCTYPE html>')).toBe(true);
  });

  it('overrides a locale the document hardcoded', () => {
    expect(withLang(EXECUTIVE, 'tr')).toContain('<html lang="tr">');
    expect(withLang(EXECUTIVE, 'tr')).not.toContain('lang="ja"');
  });

  it('adds lang when the html tag carries none', () => {
    expect(withLang('<!doctype html><html><body>z</body></html>', 'ja')).toContain('<html lang="ja">');
  });

  it('leaves a fragment without an html tag untouched', () => {
    expect(withLang('<p>parça</p>', 'en')).toBe('<p>parça</p>');
  });

  it('keeps other attributes on the html tag', () => {
    expect(withLang('<html lang="tr" data-x="1">a</html>', 'en')).toContain('data-x="1"');
  });
});

describe('withAutoPrint', () => {
  it('injects the trigger before the closing body tag', () => {
    const out = withAutoPrint(CLASSIC);
    expect(out.indexOf('print()')).toBeLessThan(out.indexOf('</body>'));
    expect(out).toContain('</body>');
  });

  it('appends the trigger when there is no body tag', () => {
    expect(withAutoPrint('<p>x</p>')).toContain('print()');
  });

  it('closes the script tag so it cannot swallow the rest of the document', () => {
    expect(withAutoPrint(CLASSIC)).toContain('<\/script>');
  });
});
