import type { R5Report } from './types';

export const GOLDEN_SCHEMA = 'gtfs-analyzer-golden/3';

// Golden snapshot'taki scores nesnesini oluşturur. Saf fonksiyon — DOM bağımlılığı yok.
// scores.score, v2 golden okuyucuları için geriye uyumluluk takma adıdır.
export function buildGoldenScores(r5: R5Report) {
  return {
    overall:    r5.score,
    score:      r5.score,          // v2 takma ad: eski golden okuyucuları kırılmaz
    pub_score:  r5.pub_score,
    spec:       r5.spec_score,
    interop:    r5.interop_score,
    quality:    r5.quality_score,  // yalnız Quality sınıfı — overall'dan farklı değer
    analytics:  r5.analytics_score,
  };
}
