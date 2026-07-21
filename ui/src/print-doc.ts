import type { Locale } from './i18n';

// Rapor sekmesi `noopener` ile açılır → pencere handle'ı YOK. Eskiden açılan pencerede
// yapılan iki iş (documentElement.lang ataması, win.print() çağrısı) bu yüzden belgenin
// kendisine gömülür. Bu modül saf string dönüşümleridir; DOM'a dokunmaz, böylece
// test edilebilir kalır.

/// `<html>` etiketinin `lang`'ını aktif dile çeker. Klasik rapor `lang="tr"` sabitler.
export function withLang(html: string, locale: Locale): string {
  const start = html.search(/<html\b/i);
  if (start < 0) return html;
  const end = html.indexOf('>', start);
  if (end < 0) return html;
  const tag = html.slice(start, end + 1).replace(/\slang="[^"]*"/i, '');
  return html.slice(0, start) + tag.replace(/^<html/i, `<html lang="${locale}"`) + html.slice(end + 1);
}

/// Belge yüklenince yazdırma diyaloğunu kendi açsın diye `</body>` öncesine tetikleyici koyar.
export function withAutoPrint(html: string): string {
  const trigger = '<script>addEventListener("load",()=>{focus();print();});<\/script>';
  return html.includes('</body>')
    ? html.replace('</body>', `${trigger}</body>`)
    : html + trigger;
}
