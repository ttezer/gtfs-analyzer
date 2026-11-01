// Escape helpers for embedding feed-sourced (untrusted) strings in HTML.
// Safe for both HTML text and attribute contexts: quotes are escaped too, so values cannot
// break out of double- or single-quoted attributes (XSS / attribute break).

export function escHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}
