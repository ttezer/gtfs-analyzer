import { describe, it, expect } from 'vitest';
import { GOLDEN_SCHEMA, buildGoldenScores } from '../golden';

// BART V014 golden'ından alınan gerçek R5Report değerleri.
// Kaynak: Test Feed/V014/BART/mdb-53-202606250002_2026-06-27_0.1.4.json (schema v2, scores.score=83.6)
const BART_V014_R5 = {
  score:           83.6,
  pub_score:       92.6,
  spec_score:      92.6,
  interop_score:   90.1,
  quality_score:   65.8,  // yalnız Quality sınıfı — score'dan farklı (karmaşanın özü buydu)
  analytics_score: 63.3,
};

describe('GOLDEN_SCHEMA', () => {
  it('doğru şema sabiti', () => {
    expect(GOLDEN_SCHEMA).toBe('gtfs-analyzer-golden/3');
  });
});

describe('buildGoldenScores — BART V014', () => {
  const scores = buildGoldenScores(BART_V014_R5);

  it('overall r5.score ile eşit (genel skor)', () => {
    expect(scores.overall).toBe(BART_V014_R5.score);
  });

  it('score v2 takma adı — overall ile aynı değer', () => {
    expect(scores.score).toBe(scores.overall);
  });

  it('pub_score r5.pub_score ile eşit', () => {
    expect(scores.pub_score).toBe(BART_V014_R5.pub_score);
  });

  it('quality r5.quality_score ile eşit', () => {
    expect(scores.quality).toBe(BART_V014_R5.quality_score);
  });

  it('quality ≠ overall (karmaşanın önlendiği temel kontrat)', () => {
    expect(scores.quality).not.toBe(scores.overall);
  });

  it('alt-skorlar doğru kaynaklardan gelir', () => {
    expect(scores.spec).toBe(BART_V014_R5.spec_score);
    expect(scores.interop).toBe(BART_V014_R5.interop_score);
    expect(scores.analytics).toBe(BART_V014_R5.analytics_score);
  });
});
