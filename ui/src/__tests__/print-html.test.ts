import { describe, expect, it } from 'vitest';
import { AUTO_PRINT_SCRIPT, PRINT_BUTTON_SCRIPT, REPORT_INLINE_SCRIPTS, withAutoPrint, withLang } from '../print-doc';

// The report tab now opens with `noopener` → there is NO window handle. The two operations
// previously performed on the window (setting documentElement.lang and calling win.print())
// were therefore moved to the document itself. The tests below pin down that move.

const CLASSIC = '<!DOCTYPE html>\n<html lang="tr">\n<head><title>x</title></head>\n<body><p>rapor</p></body>\n</html>';
const EXECUTIVE = '<!doctype html>\n<html lang="ja">\n<head></head>\n<body><section>y</section></body>\n</html>';

describe('withLang', () => {
  it('replaces the lang of the first html tag, not the doctype', () => {
    // The first implementation anchored the regex to `^`; it never matched because <!DOCTYPE> came first.
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

// The production CSP (vite.config.ts) allows inline scripts only by the hash of
// REPORT_INLINE_SCRIPTS, and no inline on* handlers at all. A report script whose text
// drifts from those constants, or a new onclick=, would silently stop working in report tabs.
describe('report scripts under the CSP', () => {
  const sources = import.meta.glob(['../executive-report.ts', '../print-doc.ts', '../pages/export.ts'],
    { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

  it('the auto-print trigger is exactly a hashed script', () => {
    const body = withAutoPrint(CLASSIC).match(/<script>([\s\S]*?)<\/script>/)?.[1];
    expect(REPORT_INLINE_SCRIPTS).toContain(body);
    expect(body).toBe(AUTO_PRINT_SCRIPT);
  });

  it('report sources use no inline event handlers', () => {
    for (const [file, src] of Object.entries(sources)) {
      expect(src.match(/\son[a-z]+\s*=\s*["'`]/g), file).toBeNull();
    }
  });

  it('the executive report embeds only the hashed print-button script', () => {
    const src = sources['../executive-report.ts'];
    const scripts = [...src.matchAll(/<script>([\s\S]*?)<\/script>/g)].map(m => m[1]);
    expect(scripts).toEqual(['${PRINT_BUTTON_SCRIPT}']);
    expect(REPORT_INLINE_SCRIPTS).toContain(PRINT_BUTTON_SCRIPT);
  });
});
