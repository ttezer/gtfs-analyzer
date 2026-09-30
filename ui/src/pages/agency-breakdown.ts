// Agency dökümü paneli (MobilityData gtfs-validator #2201). Rapor sayfasının sonunda,
// yalnız birden fazla agency'li feed'lerde görünür. Sayılar ETKİLENEN KAYITLARDIR
// (`affected_entity_count`): toplulanmış bir özet altındaki bütün seferleri/satırları sayar.
// Skorlar feed geneli kalır; bu panel onları agency'ye bölmez.
import type { AgencyBreakdown, Severity, ValidationResult } from '../types';
import { t } from '../i18n';
import { escHtml } from '../escape';

const SEVERITIES: Severity[] = ['CRITICAL', 'HIGH', 'MEDIUM', 'LOW', 'INFO'];

interface Row {
  label: string;
  bySeverity: Record<Severity, number>;
  total: number;
  /** kural → etkilenen kayıt */
  rules: Map<string, number>;
}

function emptyRow(label: string): Row {
  return {
    label,
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
  const agencies = new Map<string, Row>(
    breakdown.agencies.map(id => [id, emptyRow(id === '' ? t('agency.unnamed') : id)]),
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
  const body = rows
    .filter(row => row.total > 0 || breakdown.agencies.includes(row.label))
    .map(row => `
      <tr>
        <td>
          <details class="agency-rules">
            <summary>${escHtml(row.label)}</summary>
            <ul>${ruleList(row) || `<li>${escHtml(t('agency.no_findings'))}</li>`}</ul>
          </details>
        </td>
        ${cells(row)}
        <td class="num"><strong>${row.total.toLocaleString()}</strong></td>
      </tr>`)
    .join('');

  return `
    <div class="card agency-breakdown">
      <h3 class="rpt-section-title">${escHtml(t('agency.title'))}</h3>
      <p class="hint">${escHtml(t('agency.hint'))}</p>
      ${breakdown.complete ? '' : `<p class="hint agency-incomplete">${escHtml(t('agency.incomplete'))}</p>`}
      <div class="table-scroll">
        <table class="data-table">
          <thead><tr>
            <th>${escHtml(t('agency.col.agency'))}</th>
            ${SEVERITIES.map(s => `<th class="num">${escHtml(t(`domain.sev.${s}`))}</th>`).join('')}
            <th class="num">${escHtml(t('agency.col.total'))}</th>
          </tr></thead>
          <tbody>${body}</tbody>
        </table>
      </div>
    </div>`;
}
