// Agency dökümü paneli (MobilityData gtfs-validator #2201). Rapor sayfasının sonunda,
// yalnız birden fazla agency'li feed'lerde görünür. Sayılar ETKİLENEN KAYITLARDIR
// (`affected_entity_count`): toplulanmış bir özet altındaki bütün seferleri/satırları sayar.
// Skorlar feed geneli kalır; bu panel onları agency'ye bölmez.
import type { AgencyBreakdown, Severity, ValidationResult } from '../types';
import { t, intlLocale } from '../i18n';
import { escHtml } from '../escape';

const SEVERITIES: Severity[] = ['CRITICAL', 'HIGH', 'MEDIUM', 'LOW', 'INFO'];

interface Row {
  label: string;
  /** Agency satırında `agency_id` (ipucu olarak gösterilir); diğer satırlarda yok. */
  agencyId?: string;
  /** Agency satırında route üzerinden çözülen sefer sayısı (oranın paydası). */
  tripCount?: number;
  /** Agency satırında en az bir bulgunun konusu olan sefer sayısı. */
  affectedTrips?: number;
  bySeverity: Record<Severity, number>;
  total: number;
  /** kural → etkilenen kayıt */
  rules: Map<string, number>;
}

function emptyRow(label: string, agencyId?: string, tripCount?: number, affectedTrips?: number): Row {
  return {
    label,
    agencyId,
    tripCount,
    affectedTrips,
    bySeverity: { CRITICAL: 0, HIGH: 0, MEDIUM: 0, LOW: 0, INFO: 0 },
    total: 0,
    rules: new Map(),
  };
}

function add(row: Row, rule: string, severity: Severity | undefined, n: number): void {
  if (n === 0) return;
  if (severity) row.bySeverity[severity] += n;
  row.total += n;
  row.rules.set(rule, (row.rules.get(rule) ?? 0) + n);
}

/** Dökümü satırlara çevirir: agency'ler (feed sırasıyla), ardından atfedilemeyenler. */
export function agencyRows(breakdown: AgencyBreakdown, severityOf: Map<string, Severity>): Row[] {
  // Ad gösterilir; ad yoksa kimlik, o da yoksa "agency_id'siz agency".
  const agencies = new Map<string, Row>(
    breakdown.agencies.map(a => [
      a.agency_id,
      emptyRow(a.agency_name || a.agency_id || t('agency.unnamed'), a.agency_id, a.trip_count, a.affected_trip_count),
    ]),
  );
  const unattributed = emptyRow(t('agency.row.unattributed'));
  const unsupported = emptyRow(t('agency.row.unsupported'));
  const notApplicable = emptyRow(t('agency.row.not_applicable'));
  for (const [rule, counts] of Object.entries(breakdown.rules)) {
    const severity = severityOf.get(rule);
    for (const set of counts.agency_sets) {
      for (const id of set.agency_ids) {
        const row = agencies.get(id);
        if (row) add(row, rule, severity, set.affected_entity_count);
      }
    }
    add(unattributed, rule, severity, Object.values(counts.unattributed).reduce((a, b) => a + b, 0));
    add(unsupported, rule, severity, counts.unsupported);
    add(notApplicable, rule, severity, counts.not_applicable);
  }
  return [...agencies.values(), unattributed, unsupported, notApplicable];
}

/**
 * Etkilenen sefer oranı (%): en az bir bulgunun KONUSU olan seferler / agency'nin seferleri.
 * Hat ve durak bulguları oranı etkilemez (Toplam sütununda görünür); böylece oran %0-100
 * arasında kalır ve farklı büyüklükteki agency'ler karşılaştırılabilir. Agency başına skor
 * bilinçli olarak yok: skorlar feed geneline göre normalize edilir. Seferi yoksa `null`.
 */
export function affectedTripShare(affectedTrips: number | undefined, tripCount: number | undefined): number | null {
  return tripCount ? ((affectedTrips ?? 0) * 100) / tripCount : null;
}

export function renderAgencyBreakdown(result: ValidationResult): string {
  const breakdown = result.agency_breakdown;
  if (!breakdown || breakdown.agencies.length < 2) return '';

  const severityOf = new Map<string, Severity>();
  for (const n of result.notices) if (!severityOf.has(n.rule_id)) severityOf.set(n.rule_id, n.severity);
  const rows = agencyRows(breakdown, severityOf);

  const cells = (row: Row) => SEVERITIES
    .map(s => `<td class="num">${row.bySeverity[s] ? row.bySeverity[s].toLocaleString() : '—'}</td>`)
    .join('');
  const ruleList = (row: Row) => [...row.rules.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([rule, n]) => `<li><code>${escHtml(rule)}</code> ${escHtml(t(`rule.${rule}`))} — <strong>${n.toLocaleString()}</strong></li>`)
    .join('');
  // Yüzde işaretinin yeri dile göre değişir (tr "%60", en "60%", fr "60 %").
  const fmtShare = (d: number | null) =>
    d === null ? '—' : (d / 100).toLocaleString(intlLocale(), { style: 'percent', maximumFractionDigits: 1 });
  const columns = SEVERITIES.length + 4;

  // Her agency satırı kural listesiyle birlikte kendi <tbody>'sindedir: sıralama grupları
  // yer değiştirir, açık listeler satırından ayrılmaz. Agency dışı satırlar en alttaki
  // ayrı gövdede sabit kalır. Kural listesi satırın ALTINDA tam genişlikte açılır.
  const group = (row: Row, i: number) => {
    const share = affectedTripShare(row.affectedTrips, row.tripCount);
    const sortKeys = row.agencyId === undefined ? '' : [
      `data-k-agency="${escHtml(row.label.toLocaleLowerCase())}"`,
      ...SEVERITIES.map(s => `data-k-${s}="${row.bySeverity[s]}"`),
      `data-k-total="${row.total}"`,
      `data-k-trips="${row.tripCount ?? -1}"`,
      `data-k-share="${share ?? -1}"`,
    ].join(' ');
    return `
      <tr ${sortKeys ? 'class="agency-row"' : ''} ${sortKeys}>
        <td>
          <button type="button" class="agency-toggle" aria-expanded="false" aria-controls="agency-rules-${i}"
            ${row.agencyId ? `title="agency_id: ${escHtml(row.agencyId)}"` : ''}>▸ ${escHtml(row.label)}</button>
        </td>
        ${cells(row)}
        <td class="num"><strong>${row.total.toLocaleString()}</strong></td>
        <td class="num">${row.tripCount === undefined ? '—' : row.tripCount.toLocaleString()}</td>
        <td class="num" ${row.affectedTrips === undefined ? '' : `title="${row.affectedTrips.toLocaleString()} / ${(row.tripCount ?? 0).toLocaleString()}"`}>${fmtShare(share)}</td>
      </tr>
      <tr id="agency-rules-${i}" class="agency-rules-row" hidden>
        <td colspan="${columns}">
          <ul>${ruleList(row) || `<li>${escHtml(t('agency.no_findings'))}</li>`}</ul>
        </td>
      </tr>`;
  };
  const visible = rows.filter(row => row.total > 0 || row.agencyId !== undefined);
  const agencyBodies = visible
    .map((row, i) => row.agencyId === undefined ? '' : `<tbody class="agency-group">${group(row, i)}</tbody>`)
    .join('');
  const otherRows = visible
    .map((row, i) => row.agencyId === undefined ? group(row, i) : '')
    .join('');

  const th = (key: string, label: string, title?: string, numeric = true) =>
    `<th class="${numeric ? 'num ' : ''}agency-sortable" data-sort="${key}" ${title ? `title="${escHtml(title)}"` : ''}
       role="button" tabindex="0">${escHtml(label)} <span class="sort-ind"></span></th>`;

  return `
    <div class="card agency-breakdown">
      <h3 class="rpt-section-title">${escHtml(t('agency.title'))}</h3>
      <p class="hint">${escHtml(t('agency.hint'))}</p>
      ${breakdown.complete ? '' : `<p class="hint agency-incomplete">${escHtml(t('agency.incomplete'))}</p>`}
      <div class="table-scroll">
        <table class="data-table agency-table">
          <thead><tr>
            ${th('agency', t('agency.col.agency'), undefined, false)}
            ${SEVERITIES.map(s => th(s, t(`domain.sev.${s}`))).join('')}
            ${th('total', t('agency.col.total'))}
            ${th('trips', t('agency.col.trips'), t('agency.trips_tip'))}
            ${th('share', t('agency.col.affected_share'), t('agency.affected_share_tip'))}
          </tr></thead>
          ${agencyBodies}
          <tbody class="agency-other">${otherRows}</tbody>
        </table>
      </div>
    </div>`;
}

/** Agency gruplarını `key` sütununa göre sıralar (`desc` için büyükten küçüğe). */
export function sortAgencyGroups(table: HTMLTableElement, key: string, desc: boolean): void {
  const groups = Array.from(table.querySelectorAll<HTMLTableSectionElement>('tbody.agency-group'));
  // HTML öznitelik adları küçük harfe iner: data-k-CRITICAL → dataset.kCritical.
  const k = key.toLowerCase();
  const field = `k${k[0].toUpperCase()}${k.slice(1)}`;
  const value = (g: HTMLTableSectionElement) => g.querySelector<HTMLElement>('.agency-row')?.dataset[field] ?? '';
  groups.sort((a, b) => {
    const [va, vb] = [value(a), value(b)];
    const cmp = key === 'agency' ? va.localeCompare(vb) : Number(va) - Number(vb);
    return desc ? -cmp : cmp;
  });
  const anchor = table.querySelector('tbody.agency-other');
  for (const g of groups) table.insertBefore(g, anchor);
}

/** Kural listesini aç/kapa; sütun başlığıyla sırala (ikinci tıklama yönü çevirir). */
export function attachAgencyBreakdownListeners(root: HTMLElement): void {
  root.querySelectorAll<HTMLButtonElement>('.agency-toggle').forEach(btn => {
    btn.addEventListener('click', () => {
      const detail = root.querySelector<HTMLElement>(`#${btn.getAttribute('aria-controls')}`);
      if (!detail) return;
      const open = detail.hidden;
      detail.hidden = !open;
      btn.setAttribute('aria-expanded', String(open));
      btn.textContent = `${open ? '▾' : '▸'}${btn.textContent!.slice(1)}`;
    });
  });
  const table = root.querySelector<HTMLTableElement>('.agency-table');
  if (!table) return;
  let current = '';
  let desc = false;
  table.querySelectorAll<HTMLElement>('.agency-sortable').forEach(th => {
    const sort = () => {
      const key = th.dataset['sort']!;
      // Sayılar ilk tıklamada büyükten küçüğe, ad ilk tıklamada A→Z.
      desc = key === current ? !desc : key !== 'agency';
      current = key;
      sortAgencyGroups(table, key, desc);
      table.querySelectorAll<HTMLElement>('.agency-sortable .sort-ind').forEach(i => { i.textContent = ''; });
      th.querySelector('.sort-ind')!.textContent = desc ? '▼' : '▲';
    };
    th.addEventListener('click', sort);
    th.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); sort(); } });
  });
}
