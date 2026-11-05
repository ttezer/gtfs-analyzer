import { describe, it, expect } from 'vitest';
import { crossFilter, type CrossFilterRow, type CrossFilterState } from '../cross-filter';

const rows: CrossFilterRow[] = [
  { sev: 'CRITICAL', cls: 'SPEC', rule: 'STM_056', file: 'stop_times.txt' },
  { sev: 'CRITICAL', cls: 'SPEC', rule: 'STM_056', file: 'stop_times.txt' },
  { sev: 'LOW', cls: 'ANALYTICS', rule: 'STM_021', file: 'stop_times.txt' },
  { sev: 'HIGH', cls: 'SPEC', rule: 'AGN_003', file: 'agency.txt' },
];
const none = (): CrossFilterState => ({ sev: new Set(), cls: new Set(), rule: '', file: '' });

describe('crossFilter (Ayrıntılar ve Düzeltme süzgeçleri)', () => {
  it('filtre yokken bütün satırlar görünür, seçenekler satır sayısıyla sayılır', () => {
    const f = crossFilter(rows, none());
    expect(f.visible).toEqual([true, true, true, true]);
    expect([...f.rules]).toEqual([['STM_056', 2], ['STM_021', 1], ['AGN_003', 1]]);
    expect([...f.files]).toEqual([['stop_times.txt', 3], ['agency.txt', 1]]);
  });

  it('önem seçimi kural ve dosya listelerini daraltır', () => {
    const f = crossFilter(rows, { ...none(), sev: new Set(['CRITICAL']) });
    expect(f.visible).toEqual([true, true, false, false]);
    expect([...f.rules]).toEqual([['STM_056', 2]]);
    expect([...f.files]).toEqual([['stop_times.txt', 2]]);
  });

  it('kural seçimi kendi listesini daraltmaz, dosya listesini daraltır', () => {
    const f = crossFilter(rows, { ...none(), rule: 'AGN_003' });
    expect(f.visible).toEqual([false, false, false, true]);
    expect([...f.rules.keys()]).toEqual(['STM_056', 'STM_021', 'AGN_003']);
    expect([...f.files]).toEqual([['agency.txt', 1]]);
  });

  it('seçili kuralın satırı kalmazsa sayısı listede yer almaz (sayfa onu 0 ile tutar)', () => {
    const f = crossFilter(rows, { ...none(), rule: 'STM_021', sev: new Set(['CRITICAL']) });
    expect(f.visible.some(Boolean)).toBe(false);
    expect(f.rules.has('STM_021')).toBe(false);
  });
});
