// Saf karşılaştırma yardımcıları (render/worker'dan bağımsız → doğrudan birim testlenebilir).
// Yalnız `GoldenRun` tipini (runtime import değil) alır.
import type { GoldenRun } from '../golden';

/** config delta'yı anahtar sırasından bağımsız hale getirir; geçersiz JSON'da ham değeri döner. */
export function normalizeConfig(value: string): string {
  try {
    const parsed = JSON.parse(value || '{}') as Record<string, unknown>;
    return JSON.stringify(Object.fromEntries(Object.entries(parsed).sort()));
  } catch {
    return value;
  }
}

/** Feed adları ikisi de dolu ve farklıysa true (uyumsuz feed uyarısı). */
export function feedMismatch(before: GoldenRun, now: GoldenRun): boolean {
  return Boolean(before.feedName && now.feedName && before.feedName !== now.feedName);
}

/** feed/servis başlangıç-bitiş metriklerinden herhangi biri değiştiyse true. */
export function dateRangeChanged(before: GoldenRun, now: GoldenRun): boolean {
  return ['feed_start_date', 'feed_end_date', 'service_start_date', 'service_end_date']
    .some(key => before.metrics[key] !== now.metrics[key]);
}

/** config delta'lar (anahtar sırasından bağımsız) farklıysa true. */
export function configChanged(before: GoldenRun, now: GoldenRun): boolean {
  return normalizeConfig(before.configDelta) !== normalizeConfig(now.configDelta);
}

/** Notice yoğunluğu: paydaya göre ölçekli (per 1000 sefer / per 100k stop_times). Payda 0 ise 0. */
export function noticeDensity(run: GoldenRun, denominator: number, scale: number): number {
  return denominator > 0 ? run.noticeTotal / denominator * scale : 0;
}
