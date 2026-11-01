import type { Locale } from './i18n';

// The report tab opens with `noopener` → there is NO window handle. The two operations
// previously performed on the opened window (assigning documentElement.lang and calling
// win.print()) are therefore embedded in the document itself. This module only transforms
// strings; it does not touch the DOM, so it remains testable.

/// Set the `<html>` tag's `lang` to the active locale. The classic report hard-codes `lang="tr"`.
export function withLang(html: string, locale: Locale): string {
  const start = html.search(/<html\b/i);
  if (start < 0) return html;
  const end = html.indexOf('>', start);
  if (end < 0) return html;
  const tag = html.slice(start, end + 1).replace(/\slang="[^"]*"/i, '');
  return html.slice(0, start) + tag.replace(/^<html/i, `<html lang="${locale}"`) + html.slice(end + 1);
}

/// Insert a trigger before `</body>` so the document opens the print dialog when it loads.
export function withAutoPrint(html: string): string {
  const trigger = '<script>addEventListener("load",()=>{focus();print();});<\/script>';
  return html.includes('</body>')
    ? html.replace('</body>', `${trigger}</body>`)
    : html + trigger;
}
