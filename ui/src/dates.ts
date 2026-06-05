// kk-date sarmalayıcısı — tüm tarih formatlama/aritmetiği tek noktadan.
// kk-date (kentkartlab) GTFS takvim TARİHLERİ (YYYYMMDD) içindir; GTFS'nin
// 24:00'ı aşan stop_times SAATLERİ için DEĞİL (onlar Rust tarafında işlenir).
// kk-date geçersiz tarihte exception atar → bir doğrulama aracı çökmemeli,
// bu yüzden her çağrı defansif sarılır ve ham değere düşer.
import KkDate from 'kk-date';
import { getLocale } from './i18n';

const LOCALE_TAG: Record<string, string> = { tr: 'tr', en: 'en', ja: 'ja' };
function tag(): string {
  return LOCALE_TAG[getLocale()] ?? 'en';
}

/** YYYYMMDD tamsayısı → yerelleştirilmiş uzun tarih (örn. "23 Ağustos 2024"). null-güvenli. */
export function fmtServiceDate(ymd: number | null | undefined, template = 'DD MMMM YYYY'): string | null {
  if (ymd == null) return null;
  try {
    return new KkDate(String(ymd), 'YYYYMMDD').config({ locale: tag() }).format(template);
  } catch {
    return String(ymd);
  }
}

/** İki YYYYMMDD arasındaki kapsayıcı gün sayısı (başlangıç ve bitiş dahil). */
export function inclusiveDaySpan(startYmd: number, endYmd: number): number {
  try {
    const days = new KkDate(String(startYmd), 'YYYYMMDD').diff(new KkDate(String(endYmd), 'YYYYMMDD'), 'days');
    return Math.abs(days) + 1;
  } catch {
    return 0;
  }
}

/** İki YYYYMMDD arasındaki gün ofseti (mutlak; eksen konumlandırması için). */
export function dayOffset(fromYmd: number, toYmd: number): number {
  try {
    const d = new KkDate(String(fromYmd), 'YYYYMMDD').diff(new KkDate(String(toYmd), 'YYYYMMDD'), 'days');
    return Math.abs(d);
  } catch {
    return 0;
  }
}

/** Date → yerelleştirilmiş tarih+saat damgası (rapor oluşturulma zamanı). */
export function fmtTimestamp(d: Date, template = 'DD MMM YYYY HH:mm'): string {
  try {
    return new KkDate(d).config({ locale: tag() }).format(template);
  } catch {
    return d.toLocaleString();
  }
}
