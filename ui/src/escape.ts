// Feed kaynaklı (güvenilmez) string'leri HTML'e gömerken kullanılan escape yardımcıları.
// Hem HTML text hem de attribute bağlamı için güvenli: tırnaklar da kaçışlanır ki
// double/single quoted attribute içinde değer dışına çıkılamasın (XSS / attribute-break).

export function escHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}
