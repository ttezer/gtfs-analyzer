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

// Report tabs are blob: documents, so they inherit the app's Content-Security-Policy: inline
// scripts run only when their exact text is hashed into script-src. vite.config.ts hashes these
// two constants at build time; change the text here and the CSP follows. No inline on* handlers.
export const AUTO_PRINT_SCRIPT = 'addEventListener("load",()=>{focus();print();});';
export const PRINT_BUTTON_SCRIPT = 'document.querySelectorAll(".print-action").forEach(b=>b.addEventListener("click",()=>print()));';
export const REPORT_INLINE_SCRIPTS = [AUTO_PRINT_SCRIPT, PRINT_BUTTON_SCRIPT];

/// Insert a trigger before `</body>` so the document opens the print dialog when it loads.
export function withAutoPrint(html: string): string {
  const trigger = `<script>${AUTO_PRINT_SCRIPT}<\/script>`;
  return html.includes('</body>')
    ? html.replace('</body>', `${trigger}</body>`)
    : html + trigger;
}
