// Sayısal değerlerin kullanıcıya gösterim biçimi. Tarih/saat için → dates.ts

/// Bayt sayısını okunur birime çevirir. MB eşiğinde `Intl.NumberFormat` kullanılır:
/// ondalık ayracı kullanıcının diline uyar (tr "12,3 MB" · en "12.3 MB").
export function formatBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${Math.round(b / 1024)} KB`;
  const mb = b / (1024 * 1024);
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(mb)} MB`;
}
