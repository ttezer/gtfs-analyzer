// GEÇİCİ ölçüm düzeneği — commit edilmez. PLANNER_PERF_DIR içindeki notices.json + rest.json ile
// planlamacı sayfasının HTML üretimini zamanlar.
import { it, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { renderProducer } from '../pages/producer';
import type { ValidationResult } from '../types';

vi.mock('../state', () => ({ setPage: vi.fn(), getState: () => ({ agencyFilter: [] }), setAgencyFilter: vi.fn() }));
vi.mock('../hidden-rules', () => ({ renderHiddenRulesBar: () => '', attachHiddenRuleListeners: () => undefined }));

const dir = process.env.PLANNER_PERF_DIR;

it.skipIf(!dir)('planlamacı sayfası büyük feed ölçümü', () => {
  const rest = JSON.parse(readFileSync(`${dir}/rest.json`, 'utf8'));
  const notices = JSON.parse(readFileSync(`${dir}/notices.json`, 'utf8'));
  const result = { ...rest, notices } as ValidationResult;
  const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
  const t0 = performance.now();
  renderProducer(el, result);
  const t1 = performance.now();
  const T = (globalThis as any).__plannerTimes as Array<[string, number]>;
  if (T) console.log('[perf-steps] ' + T.slice(1).map(([l, v], i) => `${l}=${(v - T[i][1]).toFixed(0)}ms`).join(' | '));
  console.log(`[perf] notices=${notices.length} render=${(t1 - t0).toFixed(0)}ms html=${(el.innerHTML.length / 1e6).toFixed(1)}MB rows=${(el.innerHTML.match(/planner-affected-row/g) ?? []).length} cards=${(el.innerHTML.match(/class="producer-card"/g) ?? []).length}`);
}, 600_000);
