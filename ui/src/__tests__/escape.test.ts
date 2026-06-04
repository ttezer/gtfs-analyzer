import { describe, it, expect } from 'vitest';
import { escHtml } from '../escape';

describe('escHtml — feed kaynaklı string XSS/attribute koruması', () => {
  it('HTML text bağlamı: <, >, & kaçışlanır', () => {
    expect(escHtml('<img src=x onerror=alert(1)>')).toBe(
      '&lt;img src=x onerror=alert(1)&gt;',
    );
    expect(escHtml('a & b')).toBe('a &amp; b');
  });

  it('attribute bağlamı: çift ve tek tırnak kaçışlanır (attribute-break engellenir)', () => {
    // Örn. <option value="${escHtml(f)}"> içinde değer dışına çıkma denemesi
    expect(escHtml('x" autofocus onfocus=alert(1) "')).toBe(
      'x&quot; autofocus onfocus=alert(1) &quot;',
    );
    expect(escHtml("x' onfocus=alert(1)")).toBe('x&#39; onfocus=alert(1)');
  });

  it('çıktıda ham < > " \' kalmaz', () => {
    const out = escHtml(`<script>"'&</script>`);
    expect(out).not.toMatch(/[<>]/);
    expect(out).not.toContain('"');
    expect(out).not.toContain("'");
  });

  it('zararsız metin değişmeden geçer (& hariç)', () => {
    expect(escHtml('İstasyon 42 — Köprü')).toBe('İstasyon 42 — Köprü');
  });

  it('& önce kaçışlanır (çift kaçış olmaz)', () => {
    // < → &lt; üretildikten sonra tekrar & kaçışlanmamalı
    expect(escHtml('<')).toBe('&lt;');
    expect(escHtml('&lt;')).toBe('&amp;lt;');
  });
});
