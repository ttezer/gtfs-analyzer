// Ayrıntıların agency'ye göre süzülmesi (MobilityData gtfs-validator #2201). Hangi notice'ın
// hangi agency'ye düştüğünü motor hesaplar (`agency_breakdown.agencies[].notice_indices`);
// arayüz atıf kuralını YENİDEN YAZMAZ, yalnız bu indeksleri kullanır.
import type { Notice, R9Item, ValidationResult } from './types';
import { t, intlLocale } from './i18n';
import { escHtml } from './escape';

/** Seçili agency'lerin notice'ları (birleşim, sonuç sırasıyla). Filtre yoksa `null`. */
export function filterNoticesByAgency(result: ValidationResult, selected: readonly string[]): Notice[] | null {
  const breakdown = result.agency_breakdown;
  if (!breakdown || breakdown.agencies.length < 2 || selected.length === 0) return null;
  const allowed = new Set<number>();
  for (const agency of breakdown.agencies) {
    if (!selected.includes(agency.agency_id)) continue;
    for (const i of agency.notice_indices ?? []) allowed.add(i);
  }
  return result.notices.filter((_, i) => allowed.has(i));
}

/**
 * R9 kalemlerini süzülmüş notice'lara daraltır. `renderR9` kuralın önemini, başlığını ve
 * "çıkar" düğmesini `notice_ids[0]` üzerinden bulur; bu ilk notice seçili agency'ye ait
 * değilse satır eksik çizilirdi. Bu yüzden süzgeçte kalan kimlikler başa alınır; hiçbiri
 * kalmadıysa o kuralın süzgeçteki ilk notice'ı başa eklenir. Görünür notice'ı olmayan
 * kurallar düşer; sayılar ve skor etkileri (feed geneli) değişmez.
 */
export function narrowR9Items(items: readonly R9Item[], notices: readonly Notice[]): R9Item[] {
  const kept = new Set(notices.map(n => n.id));
  const firstOfRule = new Map<string, string>();
  for (const n of notices) if (!firstOfRule.has(n.rule_id)) firstOfRule.set(n.rule_id, n.id);
  return items.flatMap(item => {
    const fallback = firstOfRule.get(item.rule_id);
    if (!fallback) return [];
    const present = item.notice_ids.filter(id => kept.has(id));
    const rest = item.notice_ids.filter(id => !kept.has(id));
    return [{ ...item, notice_ids: present.length ? [...present, ...rest] : [fallback, ...item.notice_ids] }];
  });
}

export function renderAgencyFilterBar(result: ValidationResult, selected: readonly string[]): string {
  const breakdown = result.agency_breakdown;
  if (!breakdown || breakdown.agencies.length < 2) return '';
  // Izgara: satır başına üç çip; kutucuklar aynı hizada (dar ekranda tek sütun).
  // Ada göre sıralı (dile duyarlı); aynı adlı agency'ler kimliğe göre ayrılır.
  const labelOf = (a: { agency_name: string; agency_id: string }) => a.agency_name || a.agency_id || t('agency.unnamed');
  const sorted = [...breakdown.agencies].sort((x, y) =>
    labelOf(x).localeCompare(labelOf(y), intlLocale(), { sensitivity: 'base' }) || x.agency_id.localeCompare(y.agency_id));
  const chips = sorted.map(a => {
    const label = labelOf(a);
    const checked = selected.includes(a.agency_id) ? 'checked' : '';
    return `<label class="agency-chip" title="agency_id: ${escHtml(a.agency_id)}">
      <input type="checkbox" class="agency-filter-cb" value="${escHtml(a.agency_id)}" ${checked}>
      <span class="agency-chip-name">${escHtml(label)}</span>
      <span class="agency-chip-count">${(a.notice_indices ?? []).length}</span>
    </label>`;
  }).join('');
  return `
    <div class="card agency-filter">
      <div class="agency-filter-head">
        <strong>${escHtml(t('agency.filter.label'))}</strong>
        ${selected.length ? `<button type="button" class="agency-filter-clear">${escHtml(t('agency.filter.all'))}</button>` : ''}
      </div>
      <div class="agency-chip-grid">${chips}</div>
      ${selected.length ? `<p class="agency-filter-warning" role="note">${escHtml(t('agency.filter.hint'))}</p>` : ''}
    </div>`;
}

/** Seçim değişince `onChange` yeni kimlik listesiyle çağrılır. */
export function attachAgencyFilterListeners(root: HTMLElement, onChange: (ids: string[]) => void): void {
  const boxes = () => Array.from(root.querySelectorAll<HTMLInputElement>('.agency-filter-cb'));
  boxes().forEach(cb => cb.addEventListener('change', () => {
    onChange(boxes().filter(b => b.checked).map(b => b.value));
  }));
  root.querySelector('.agency-filter-clear')?.addEventListener('click', () => onChange([]));
}
