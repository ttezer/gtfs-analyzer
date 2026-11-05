import { attachAgencyFilterListeners, filterNoticesByAgency, narrowR9Items, renderAgencyFilterBar } from '../agency-filter';
import { getState, setAgencyFilter } from '../state';
import type { ValidationResult, Notice, R9Item, NameIndex, Severity } from '../types';
import { SEVERITY_TR, SEVERITY_COLOR, RULE_CLASS_TR, t, tMsg, tRemediation, intlLocale } from '../i18n';
import { MAX_MAP_PINS, openMapModal, type MapPin, type MapOptions } from '../map-modal';
import { requestShapeCoords } from '../validator-client';
import { openPatternModal } from '../pattern-modal';
import { escHtml } from '../escape';
import { crossFilter } from '../cross-filter';
import { SEVERITY_ORDER, SEVERITY_RANK } from '../severity-order';
import { renderHiddenRulesBar, attachHiddenRuleListeners } from '../hidden-rules';

// EN/JA parity: route-scoped findings use entity_id = route_id and the {route_label} message
// template. Inject name_index.routes[route_id] = route_short_name (or long_name) into details
// so the en/ja templates show the same friendly route code as Turkish (the Turkish runtime
// message already contains route_short_name; this is only for the en/ja template path). Idempotent; non-route
// It does not touch non-route entities.
export function augmentRouteLabels(notices: Notice[], nameIndex: NameIndex): void {
  for (const n of notices) {
    const eid = n.entity_id ?? '';
    if (!eid || !(eid in nameIndex.routes)) continue;
    n.details = { ...(n.details ?? {}), route_label: nameIndex.routes[eid] || eid };
  }
}

/** Spec rules cannot be disabled (the publication decision depends on them) → do not show the button. */
function hideBtn(notice: Notice | undefined): string {
  if (!notice || notice.rule_class === 'SPEC') return '';
  const label = t('fix.hide_rule');
  const hint  = t('fix.hide_rule_hint');
  return `<button class="hide-rule-btn" data-rule="${escHtml(notice.rule_id)}" title="${hint}" aria-label="${hint}">✕ ${label}</button>`;
}

export function renderFix(root: HTMLElement, fullResult: ValidationResult, fileFilter?: string, classFilter?: string): void {
  augmentRouteLabels(fullResult.notices, fullResult.name_index);
  // Agency filter: R2 narrows to notices from the selected agencies, and R9 to the rules
  // in those notices. Scores and R9 score effects remain feed-wide.
  const agencyFilter = getState().agencyFilter;
  const filtered = filterNoticesByAgency(fullResult, agencyFilter);
  const keptIds = filtered ? new Set(filtered.map(n => n.id)) : null;
  const result: ValidationResult = filtered
    ? {
        ...fullResult,
        notices: filtered,
        // The R2 badge shows the item count; items with no filtered notices are removed.
        reports: {
          ...fullResult.reports,
          r2: { ...fullResult.reports.r2, items: fullResult.reports.r2.items.filter(i => keptIds!.has(i.notice_id)) },
        },
      }
    : fullResult;
  const noticeMap = new Map<string, Notice>(result.notices.map(n => [n.id, n]));
  const r9Items = filtered
    ? narrowR9Items(fullResult.reports.r9.items, filtered)
    : fullResult.reports.r9.items;

  const totalDelta    = result.reports.r9.items.reduce((s, i) => s + i.score_delta, 0);
  const normFactor    = totalDelta > 0 ? (100 - result.reports.r5.score) / totalDelta : 1;
  const totalPubDelta = result.reports.r9.items.reduce((s, i) => s + i.pub_score_delta, 0);
  const pubNormFactor = totalPubDelta > 0 ? (100 - result.reports.r5.pub_score) / totalPubDelta : 1;

  const deltaMap = new Map<string, { qualDelta: number; pubDelta: number; count: number }>(
    result.reports.r9.items.map(i => [i.rule_id, {
      qualDelta: i.score_delta     * normFactor,
      pubDelta:  i.pub_score_delta * pubNormFactor,
      count:     i.affected_instance_count,
    }])
  );

  root.innerHTML = `
    <section class="page-fix">
      ${renderHiddenRulesBar()}
      ${renderAgencyFilterBar(fullResult, agencyFilter)}
      ${renderR9(r9Items, noticeMap, normFactor, pubNormFactor, result.capped_totals)}
      ${renderR2(result, noticeMap, deltaMap, result.name_index, fileFilter, classFilter)}
    </section>`;
  attachAgencyFilterListeners(root, ids => {
    setAgencyFilter(ids);
    window.dispatchEvent(new CustomEvent('gtfs-navigate'));
  });
}

function renderR9(items: R9Item[], noticeMap: Map<string, Notice>, normFactor: number, pubNormFactor: number, cappedTotals: Record<string, number>): string {
  if (items.length === 0) {
    return `<div class="card"><h2>${t('fix.r9_title')}</h2><p class="empty">${t('fix.r9_empty')}</p></div>`;
  }

  // result.notices = display_notices (after the cap); calculate how many notices are shown per rule.
  const shownCounts: Record<string, number> = {};
  for (const n of noticeMap.values()) {
    shownCounts[n.rule_id] = (shownCounts[n.rule_id] ?? 0) + 1;
  }

  const rowPairs = items.map((item, i) => {
    const notice = item.notice_ids[0] ? noticeMap.get(item.notice_ids[0]) : undefined;
    const severity = notice?.severity ?? 'INFO';
    const badgeHtml = item.labels.map(l => `<span class="label-badge label-${l}">${t(`label.${l}`) !== `label.${l}` ? t(`label.${l}`) : l}</span>`).join('');

    const pubSd  = item.pub_score_delta * pubNormFactor;
    const qualSd = item.score_delta     * normFactor;
    const pubHtml  = pubSd  >= 0.05 ? `<span class="pub-delta">+${pubSd.toFixed(1)}</span>`   : '<span class="muted-text">—</span>';
    const qualHtml = qualSd >= 0.05 ? `<span class="score-delta">+${qualSd.toFixed(1)}</span>` : '<span class="muted-text">—</span>';
    const realTotal = cappedTotals[item.rule_id];
    // Total: the actual total only for rules affected by the cap.
    const totalHtml = realTotal != null
      ? `<span class="cap-total">${realTotal.toLocaleString(intlLocale())}</span>`
      : '<span class="muted-text">—</span>';
    // Error: the number shown in the R2 table (after the cap).
    const shownCount = shownCounts[item.rule_id] ?? item.affected_instance_count;

    const mainRow = `
      <tr class="r9-main-row" data-idx="${i}" data-sev="${severity}">
        <td>
          <span class="r9-arrow">▶</span> <code>${escHtml(item.rule_id)}</code>
          ${notice ? `<span class="r9-rule-title">${escHtml(t('rule.' + notice.rule_id))}</span>` : ''}
          ${hideBtn(notice)}
        </td>
        <td style="color:${SEVERITY_COLOR[severity]}">${SEVERITY_TR[severity]}</td>
        <td>${badgeHtml}</td>
        <td data-val="${shownCount}">${shownCount.toLocaleString(intlLocale())}</td>
        <td data-val="${realTotal ?? -1}">${totalHtml}</td>
        <td class="score" data-val="${item.priority_score}">${item.priority_score.toFixed(1)}</td>
        <td class="score-delta-cell" data-val="${pubSd}">${pubHtml}</td>
        <td class="score-delta-cell" data-val="${qualSd}">${qualHtml}</td>
        <td data-val="${item.realized_dependent_count}">${item.realized_dependent_count}</td>
        <td data-val="${item.fix_effort}">${item.fix_effort.toFixed(1)}</td>
      </tr>`;

    const detailRow = `
      <tr class="r9-detail-row" data-for="${i}" hidden>
        <td colspan="10">
          <div class="r9-detail">
            ${notice ? `<p><strong>${t('fix.r9_message')}</strong> ${escHtml(t('rule.' + notice.rule_id))}</p>` : ''}
            ${notice?.remediation ? `<p><strong>${t('fix.r9_remediation')}</strong> ${escHtml(tRemediation(notice))}</p>` : ''}
          </div>
        </td>
      </tr>`;

    return mainRow + detailRow;
  }).join('');

  const sevPresent = Array.from(new Set(items.map(it => {
    const n = it.notice_ids[0] ? noticeMap.get(it.notice_ids[0]) : undefined;
    return n?.severity ?? 'INFO';
  })));
  const sevChips = sevPresent.map(s =>
    `<button class="sev-chip" data-sev="${s}" style="--chip-color:${SEVERITY_COLOR[s]}">${SEVERITY_TR[s]}</button>`
  ).join('');

  return `
    <details class="card r9-card">
      <summary>
        <h2>${t('fix.r9_title')} <span class="count-badge">${items.length}</span></h2>
        <span class="r9-collapsed-hint">${t('fix.r9_collapsed_hint')}</span>
      </summary>
      <p class="hint">${t('fix.r9_hint')}</p>
      ${sevPresent.length > 1 ? `<div class="r9-sev-filter" id="r9-sev-filter" role="group" aria-label="${t('fix.th.severity')}">${sevChips}</div>` : ''}
      <div class="table-scroll">
        <table class="data-table" id="r9-table">
          <thead><tr>
            <th class="r9-sortable" data-col="0" data-type="text">${t('fix.th.rule')} <span class="sort-ind"></span></th>
            <th class="r9-sortable" data-col="1" data-type="sev">${t('fix.th.severity')} <span class="sort-ind"></span></th>
            <th class="r9-sortable" data-col="2" data-type="text">${t('fix.th.label')} <span class="sort-ind"></span></th>
            <th class="r9-sortable" data-col="3" data-type="num">${t('fix.th.count')} <span class="col-info" role="button" tabindex="0" data-tip="${t('fix.th.count.tip')}">ℹ</span> <span class="sort-ind"></span></th>
            <th class="r9-sortable" data-col="4" data-type="num">${t('fix.th.total')} <span class="col-info" role="button" tabindex="0" data-tip="${t('fix.th.total.tip')}">ℹ</span> <span class="sort-ind"></span></th>
            <th class="r9-sortable score" data-col="5" data-type="num">${t('fix.th.score')} <span class="col-info" role="button" tabindex="0" data-tip="${t('fix.th.score.tip')}">ℹ</span> <span class="sort-ind"></span></th>
            <th class="r9-sortable score-delta-cell" data-col="6" data-type="num">${t('fix.th.pub')} <span class="col-info" role="button" tabindex="0" data-tip="${t('fix.th.pub.tip')}">ℹ</span> <span class="sort-ind"></span></th>
            <th class="r9-sortable score-delta-cell" data-col="7" data-type="num">${t('fix.th.quality')} <span class="col-info" role="button" tabindex="0" data-tip="${t('fix.th.quality.tip')}">ℹ</span> <span class="sort-ind"></span></th>
            <th class="r9-sortable" data-col="8" data-type="num">${t('fix.th.dependent')} <span class="col-info" role="button" tabindex="0" data-tip="${t('fix.th.dependent.tip')}">ℹ</span> <span class="sort-ind"></span></th>
            <th class="r9-sortable" data-col="9" data-type="num">${t('fix.th.effort')} <span class="col-info" role="button" tabindex="0" data-tip="${t('fix.th.effort.tip')}">ℹ</span> <span class="sort-ind"></span></th>
          </tr></thead>
          <tbody>${rowPairs}</tbody>
        </table>
      </div>
    </details>`;
}


// ONE GTFS spec link for a finding. Priority: GTFS-JP profile rules / _jp files → gtfs.jp;
// otherwise notice.file → gtfs.org schedule reference anchor (remove the dot from the filename:
// stop_times.txt → #stop_timestxt). Feed-level findings without a file → null (no link).
// Even for a multi-file rule, notice.file is the "primary file where the problem was reported" → one link is enough.
function specUrl(notice: Notice, jpProfile?: string | null): string | null {
  const file = notice.file ?? '';
  if (notice.rule_id.startsWith('JPN_') || file.endsWith('_jp.txt')) {
    // v4 (2026-03) is published on MLIT's COMmmmONS page; the reference on gtfs.jp
    // describes GTFS-JP 第2版 and is the correct address for v3/Auto. The old link pointed
    // to the site's `testsite/` draft path — moved to the canonical path (2026-09-20).
    return jpProfile?.toLowerCase() === 'v4'
      ? 'https://www.mlit.go.jp/commmmons/document/007/'
      : 'https://www.gtfs.jp/developpers-guide/format-reference.html';
  }
  if (!file) return null;
  return `https://gtfs.org/documentation/schedule/reference/#${file.replace(/\./g, '')}`;
}

function renderR2(result: ValidationResult, noticeMap: Map<string, Notice>, deltaMap: Map<string, { qualDelta: number; pubDelta: number; count: number }>, nameIndex: NameIndex, fileFilter?: string, classFilter?: string): string {
  const items = result.reports.r2.items;
  if (items.length === 0) {
    return `<div class="card"><h2>${t('fix.r2_title')}</h2><p class="empty">${t('fix.r2_empty')}</p></div>`;
  }

  const uniqueRules = [...new Map(
    items
      .filter(item => noticeMap.has(item.notice_id))
      .map(item => {
        const n = noticeMap.get(item.notice_id)!;
        return [n.rule_id, t('rule.' + n.rule_id)] as [string, string];
      })
  ).entries()].sort(([a], [b]) => a.localeCompare(b));

  const ruleOptions = uniqueRules
    .map(([id, title]) => `<option value="${escHtml(id)}">${escHtml(id)} — ${escHtml(title)}</option>`)
    .join('');

  // Unique filenames for the file filter.
  const uniqueFiles = [...new Set(
    items
      .filter(item => noticeMap.has(item.notice_id))
      .map(item => noticeMap.get(item.notice_id)!.file)
      .filter((f): f is string => !!f)
  )].sort();
  const fileOptions = uniqueFiles
    .map(f => `<option value="${escHtml(f)}"${fileFilter && fileFilter === f ? ' selected' : ''}>${escHtml(f)}</option>`)
    .join('');

  // Severity and class: a multi-select chip filter like R9 (only values that are present).
  const CLS_ORDER = ['SPEC', 'INTEROP', 'QUALITY', 'ANALYTICS'];
  const sevPresent = new Set<string>();
  const clsPresent = new Set<string>();
  for (const item of items) {
    const n = noticeMap.get(item.notice_id);
    if (!n) continue;
    sevPresent.add(n.severity);
    clsPresent.add(n.rule_class);
  }
  const sevChips = SEVERITY_ORDER.filter(s => sevPresent.has(s)).map(s =>
    `<button class="sev-chip" data-sev="${s}" style="--chip-color:${SEVERITY_COLOR[s as keyof typeof SEVERITY_COLOR]}">${SEVERITY_TR[s as keyof typeof SEVERITY_TR]}</button>`
  ).join('');
  const clsChips = CLS_ORDER.filter(c => clsPresent.has(c)).map(c =>
    `<button class="cls-chip${classFilter === c ? ' active' : ''}" data-cls="${c}">${RULE_CLASS_TR[c as keyof typeof RULE_CLASS_TR]}</button>`
  ).join('');

  const all = t('fix.filter.all');
  const filterBar = `
    <div class="filter-bar">
      ${sevPresent.size > 1 ? `<div class="filter-chip-group" id="r2-sev-filter"><span class="filter-cap">${t('fix.filter.severity')}</span>${sevChips}</div>` : ''}
      ${clsPresent.size > 1 ? `<div class="filter-chip-group" id="r2-cls-filter"><span class="filter-cap">${t('fix.filter.class')}</span>${clsChips}</div>` : ''}
      <label>${t('fix.filter.rule')}
        <select id="rule-filter">
          <option value="">${all}</option>
          ${ruleOptions}
        </select>
      </label>
      ${uniqueFiles.length > 0 ? `
      <label>${t('fix.filter.file')}
        <select id="file-filter">
          <option value="">${all}</option>
          ${fileOptions}
        </select>
      </label>` : ''}
      <span id="filter-count" class="filter-count"></span>
    </div>`;

  const rows = items.map(item => {
    const notice = noticeMap.get(item.notice_id);
    if (!notice) return '';
    const entry = deltaMap.get(notice.rule_id);
    const pubUnit  = entry != null && entry.count > 0 ? entry.pubDelta  / entry.count : null;
    const qualUnit = entry != null && entry.count > 0 ? entry.qualDelta / entry.count : null;
    const pubHtml  = pubUnit  != null && pubUnit  >= 0.005 ? `<span class="pub-delta">+${pubUnit.toFixed(2)}</span>`   : '<span class="muted-text">—</span>';
    const qualHtml = qualUnit != null && qualUnit >= 0.005 ? `<span class="score-delta">+${qualUnit.toFixed(2)}</span>` : '<span class="muted-text">—</span>';
    const mapLabel = t('fix.map_btn');
    const mapBtn   = hasMapCoords(notice, nameIndex)
      ? `<button class="map-pin-btn" data-notice-id="${escHtml(notice.id)}" title="${mapLabel}" aria-label="${mapLabel}">📍</button>`
      : '';
    // Patterns: show only for route-structure/service rules where viewing the pattern is useful
    // (PATTERN_RULES), and only when the entity is actually a route (route_id).
    const patLabel = t('fix.pattern_btn');
    const patBtn   = patternRouteId(notice, nameIndex)
      ? `<button class="pattern-btn" data-notice-id="${escHtml(notice.id)}" title="${patLabel}" aria-label="${patLabel}">${PATTERN_ICON}</button>`
      : '';
    const specHref = specUrl(notice, result.metrics.gtfs_jp_profile);
    const ruleLink = specHref
      ? `<a href="${escHtml(specHref)}" target="_blank" rel="noopener" class="spec-link" title="${t('fix.spec_link')}">${escHtml(item.display_label)}</a>`
      : escHtml(item.display_label);
    // Put the hide button in the first column: the last column was hidden behind horizontal scrolling.
    const ruleCell = `${ruleLink}${hideBtn(notice)}`;
    return `
      <tr data-severity="${notice.severity}" data-class="${notice.rule_class}" data-rule="${notice.rule_id}" data-file="${escHtml(notice.file ?? '')}">
        <td>${ruleCell}</td>
        <td style="color:${SEVERITY_COLOR[notice.severity]}">${SEVERITY_TR[notice.severity]}</td>
        <td>${RULE_CLASS_TR[notice.rule_class]}</td>
        <td>${escHtml(tMsg(notice))}</td>
        <td>${notice.file ? escHtml(notice.file) : '—'}</td>
        <td>${notice.service_id ? escHtml(notice.service_id) : '—'}</td>
        <td>${notice.line ?? '—'}</td>
        <td>${notice.field ? escHtml(notice.field) : '—'}</td>
        <td class="score-delta-cell">${pubHtml}</td>
        <td class="score-delta-cell">${qualHtml}</td>
        <td class="map-btn-cell">${mapBtn}${patBtn}</td>
      </tr>`;
  }).join('');

  return `
    <div class="card" id="r2-card">
      <h2>${t('fix.r2_title')} <span class="count-badge">${items.length}</span></h2>
      ${filterBar}
      <div id="r2-cap-warning" class="cap-warning" hidden></div>
      <div class="table-scroll">
        <table class="data-table" id="r2-table">
          <thead><tr>
            <th class="r2-sortable" data-col="0" data-type="text">${t('fix.r2.th.rule')} <span class="sort-ind"></span></th>
            <th class="r2-sortable" data-col="1" data-type="sev">${t('fix.r2.th.severity')} <span class="sort-ind"></span></th>
            <th class="r2-sortable" data-col="2" data-type="text">${t('fix.r2.th.class')} <span class="sort-ind"></span></th>
            <th class="r2-sortable" data-col="3" data-type="text">${t('fix.r2.th.message')} <span class="sort-ind"></span></th>
            <th class="r2-sortable" data-col="4" data-type="text">${t('fix.r2.th.file')} <span class="sort-ind"></span></th>
            <th class="r2-sortable" data-col="5" data-type="text">${t('fix.r2.th.service')} <span class="sort-ind"></span></th>
            <th class="r2-sortable" data-col="6" data-type="num">${t('fix.r2.th.row')} <span class="sort-ind"></span></th>
            <th class="r2-sortable" data-col="7" data-type="text">${t('fix.r2.th.field')} <span class="sort-ind"></span></th>
            <th class="r2-sortable score-delta-cell" data-col="8" data-type="num">${t('fix.r2.th.pub')} <span class="col-info" role="button" tabindex="0" data-tip="${t('fix.r2.th.pub.tip')}">ℹ</span> <span class="sort-ind"></span></th>
            <th class="r2-sortable score-delta-cell" data-col="9" data-type="num">${t('fix.r2.th.quality')} <span class="col-info" role="button" tabindex="0" data-tip="${t('fix.r2.th.quality.tip')}">ℹ</span> <span class="sort-ind"></span></th>
            <th class="map-btn-cell"></th>
          </tr></thead>
          <tbody>${rows}</tbody>
        </table>
      </div>
    </div>`;
}

function stopPin(id: string, nameIndex: NameIndex, primary: boolean, suffix?: string): MapPin | null {
  const coords = nameIndex.stop_coords[id];
  if (!coords) return null;
  const name = nameIndex.stops[id] ?? id;
  // name and id come from the feed → escaping is required because Leaflet renders bindPopup as HTML.
  const label = `<strong>${escHtml(name)}</strong>${suffix ? ` ${escHtml(suffix)}` : ''}<br><code>${escHtml(id)}</code>`;
  return { lat: coords[0], lon: coords[1], label, primary };
}

// Build a safe (escaped) map popup label from the feed-provided name + id.
// Because map labels are rendered as HTML through bindPopup/innerHTML,
// escape feed values in one place (the repeated <strong>/<code> pattern).
function stopLabel(name: string, id: string): string {
  return `<strong>${escHtml(name)}</strong><br><code>${escHtml(id)}</code>`;
}

function pushMapPin(pins: MapPin[], pin: MapPin): void {
  if (pins.length < MAX_MAP_PINS) pins.push(pin);
}

// Map icon system: whitelist-based.
// It is NOT enough for entity_id to exist in stop_coords — rule-based checking is required.
// To prevent false matches, only rules defined here show a map icon.

// Rules whose entity_id is a stop_id (show a stop pin).
const STOP_ID_RULES = new Set([
  'STP_003','STP_004','STP_005','STP_006','STP_007','STP_008','STP_009',
  'STP_010','STP_011','STP_012','STP_013','STP_014','STP_015','STP_016',
  'STP_017','STP_018','STP_019','STP_020','STP_021','STP_022','STP_023',
  'STP_024','STP_026','STP_028','STP_029','STP_030',
  // NOTE: GEO_001/003/004/005 are retired, GEO_010/011 are only R4 display labels
  // (k7_reporting::r4_display_label — notice.rule_id remains SHP_014/016), and GEO_014
  // is feed-level (no entity_id) → none matched a real notice, so they were removed.
  'GEO_002','GEO_009','GEO_012','GEO_015','GEO_016','GEO_019','GEO_022',
  'PTH_012','PTH_013','PTH_015','PTH_016','PTH_017','PTH_018','PTH_019',
  'LVL_006',
  'SHP_022','SHP_024',
  'STP_033','STP_036',
  'VAT_002','VAT_007',
]);

// Rules whose entity_id is a trip_id — generic trip map.
const TRIP_ID_RULES = new Set([
  // TRP grubu
  'TRP_001','TRP_002','TRP_003','TRP_004','TRP_005','TRP_006','TRP_007','TRP_008',
  'TRP_010','TRP_011','TRP_012','TRP_013','TRP_014','TRP_015','TRP_016',
  'TRP_017','TRP_018','TRP_019','TRP_020',
  // FRQ grubu
  'FRQ_001','FRQ_002','FRQ_003','FRQ_004','FRQ_005','FRQ_006','FRQ_007','FRQ_008','FRQ_009',
  // STM group — rules without a dedicated handler (STM_008 moved to a dedicated handler).
  'STM_001','STM_002','STM_003','STM_004','STM_005','STM_006','STM_007',
  'STM_009','STM_010','STM_011','STM_015','STM_016','STM_018','STM_019',
  // STM_036 (row ordering) is not geographic — stop locations are correct, only the file row
  // order is wrong; the map would be misleading (empty/normal-looking). Deliberately no map.
  'STM_021','STM_022','STM_024','STM_028','STM_029','STM_030',
  'STM_031','STM_032','STM_034',
  // VAT group — trip-based.
  'VAT_003',
  // OPR group — trip-based.
  'OPR_006','OPR_017',
]);

// Rules whose entity_id is a route_id — generic route map.
const ROUTE_ID_RULES = new Set([
  // RTS grubu
  'RTS_001','RTS_002','RTS_003','RTS_004','RTS_005','RTS_006','RTS_007','RTS_008',
  'RTS_010','RTS_011','RTS_012','RTS_013','RTS_015','RTS_016','RTS_017',
  'RTS_018','RTS_019','RTS_020','RTS_021','RTS_022','RTS_023',
  // VAT group — route-based.
  'VAT_001',
  // OPR group — route-based.
  'OPR_001','OPR_002','OPR_003',
]);

// Trip-pattern view = the route's STRUCTURE (stop patterns). Only two rules genuinely
// benefit from it: viewing the structure of a route without shapes (RTS_017) and seeing
// which pattern a duration-outlier trip uses (VAT_003). Frequency (OPR_001/005 — the modal
// gives a route-wide average that conflicts with the rule's route+direction+calendar slice)
// and similarity (VAT_001 — a PAIR of routes, unrelated to a single-route pattern) are deliberately EXCLUDED.
const PATTERN_RULES = new Set([
  'VAT_003', // trip duration outlier — which pattern does the trip use?
  'RTS_017', // route without shapes — show the structure when no shape exists
]);

// "Trip pattern" icon — branching route (variant) diagram; distinct from the map (📍).
const PATTERN_ICON =
  '<svg width="15" height="15" viewBox="0 0 16 16" aria-hidden="true" style="vertical-align:middle">' +
  '<path d="M2.5 8h4l6-4M6.5 8l6 4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>' +
  '<circle cx="2.5" cy="8" r="1.7" fill="currentColor"/>' +
  '<circle cx="13" cy="4" r="1.7" fill="currentColor"/>' +
  '<circle cx="13" cy="12" r="1.7" fill="currentColor"/>' +
  '</svg>';

// Resolve the route_id for the pattern modal: RTS_017's entity is a route_id; VAT_003's
// entity is a trip_id → convert it to its route through trip_routes. Return null when unsuitable (no button).
function patternRouteId(notice: Notice, nameIndex: NameIndex): string | null {
  if (!PATTERN_RULES.has(notice.rule_id)) return null;
  const eid = notice.entity_id ?? '';
  if (!eid) return null;
  if (eid in nameIndex.routes) return eid;            // route entity (RTS_017)
  const r = nameIndex.trip_routes[eid];               // trip entity (VAT_003) → route
  return r && r in nameIndex.routes ? r : null;
}

function hasMapCoords(notice: Notice, nameIndex: NameIndex): boolean {
  const eid = notice.entity_id ?? '';

  // Stop-pin rules: entity_id is a stop_id and its coordinates are known.
  if (STOP_ID_RULES.has(notice.rule_id)) {
    return !!(eid && eid in nameIndex.stop_coords);
  }
  // Feed-level geographic rules — evidence is "the distribution of all stops": GEO_013 (summary),
  // GEO_018 (all within 200m), GEO_021 (shared coordinates). Show when stop_coords is populated.
  if (['GEO_013','GEO_018','GEO_021'].includes(notice.rule_id)) {
    return Object.keys(nameIndex.stop_coords).length > 0;
  }
  // GEO_017 / GEO_020: shape rules, but the pin comes from "lat,lon" in observed_value
  // (the map remains meaningful in deferred mode even when shape_coords is empty).
  if (notice.rule_id === 'GEO_017' || notice.rule_id === 'GEO_020') {
    return /^-?\d+(\.\d+)?,-?\d+(\.\d+)?$/.test((notice.observed_value ?? '').trim());
  }
  // SHP_017: bad_stop or ctx from details.
  if (notice.rule_id === 'SHP_017') {
    const sid  = notice.details?.['bad_stop'] ?? '';
    const ctxB = notice.details?.['ctx_b'] ?? '';
    const ctxA = notice.details?.['ctx_a'] ?? '';
    return !!(sid && sid in nameIndex.stop_coords) || ctxB.length > 0 || ctxA.length > 0;
  }
  // STM_014: speed violation between two stops + trip shape.
  if (notice.rule_id === 'STM_014') {
    const m = notice.observed_value?.match(/\(([^→ ]+)\s*→\s*([^)]+)\)/);
    if (m && (m[1] in nameIndex.stop_coords || m[2] in nameIndex.stop_coords)) return true;
    return !!(eid && eid in nameIndex.trip_shapes);
  }
  // STM_008 / STM_020: two problematic stops (departure/arrival) + trip shape.
  if (notice.rule_id === 'STM_020' || notice.rule_id === 'STM_008') {
    const sa = notice.details?.['stop_a'] ?? '';
    const sb = notice.details?.['stop_b'] ?? '';
    if (sa && sa in nameIndex.stop_coords) return true;
    if (sb && sb in nameIndex.stop_coords) return true;
    return !!(eid && eid in nameIndex.trip_shapes);
  }
  // STM_025: short segment — two stop pins + trip shape.
  if (notice.rule_id === 'STM_025') {
    const sa = notice.details?.['stop_a'] ?? '';
    const sb = notice.details?.['stop_b'] ?? '';
    if (sa && sa in nameIndex.stop_coords) return true;
    if (sb && sb in nameIndex.stop_coords) return true;
    return !!(eid && eid in nameIndex.trip_shapes);
  }
  // OPR_015: single-shape warning — shape_id is in details.
  if (notice.rule_id === 'OPR_015') {
    const shapeId = notice.details?.['shape_id'] ?? '';
    return !!(shapeId && shapeId in nameIndex.shape_coords);
  }
  // PTH_014: pathway between different stations — coordinates of both stations.
  if (notice.rule_id === 'PTH_014') {
    const fs = notice.details?.['from_station'] ?? '';
    const ts = notice.details?.['to_station'] ?? '';
    return !!(fs && fs in nameIndex.stop_coords && ts && ts in nameIndex.stop_coords);
  }
  // OPR_007/OPR_008/STM_017: entity_id = trip_id.
  // In addition to the shape polyline, the map can also be drawn from stop coordinates
  // (OPR_007 stops details, STM_017 trip_stops, OPR_008 bad_seg stops).
  // Therefore show the icon for feeds without shapes.txt when stop data is available.
  if (['OPR_007','OPR_008','STM_017'].includes(notice.rule_id)) {
    return !!(eid && (eid in nameIndex.trip_shapes || eid in nameIndex.trip_stops));
  }
  // Rules requiring shape coordinates: entity_id = shape_id.
  // In large-feed mode (map_data_deferred), shape_coords is empty; geometry is fetched
  // on demand when the map icon is clicked → show the icon when eid is present.
  if (['SHP_009','SHP_010','SHP_012','SHP_014','SHP_015','SHP_016','SHP_018','SHP_019','SHP_020','GEO_006','GEO_007'].includes(notice.rule_id)) {
    return !!(eid && (eid in nameIndex.shape_coords || nameIndex.map_data_deferred));
  }
  // Rules requiring a trip shape: entity_id = trip_id.
  if (['STM_012','STM_026','STM_033'].includes(notice.rule_id)) {
    return !!(eid && eid in nameIndex.trip_shapes) || !!(eid && eid in nameIndex.trip_stops);
  }
  // STM_035: the same stop twice in succession — repeated stop pin or trip shape.
  if (notice.rule_id === 'STM_035') {
    const stopId = notice.observed_value ?? '';
    return !!(stopId && stopId in nameIndex.stop_coords) || !!(eid && (eid in nameIndex.trip_shapes || eid in nameIndex.trip_stops));
  }
  // VAT_005: can also show isolated stops — when stop_coords is non-empty.
  if (notice.rule_id === 'VAT_005') {
    return Object.keys(nameIndex.stop_coords).length > 0;
  }
  // Generic trip map: entity_id trip_id — when the shape or stop list is known.
  if (TRIP_ID_RULES.has(notice.rule_id)) {
    return !!(eid && (eid in nameIndex.trip_shapes || eid in nameIndex.trip_stops));
  }
  // Generic route map: entity_id route_id — when at least one shape exists.
  if (ROUTE_ID_RULES.has(notice.rule_id)) {
    return !!(eid && (nameIndex.route_shapes[eid]?.length ?? 0) > 0);
  }
  return false;
}

// Extract the shape_id to fetch on demand for the map from a notice in large-feed mode.
// Rules whose entity_id = shape_id + SHP_017/SHP_022 carrying details.shape_id.
// Trip-context rules (entity_id = trip_id): resolve shape_id from trip_shapes.
// In large_feed_mode, trip_shapes is empty → return '' (the map is already empty by design).
const SHAPE_ENTITY_RULES = new Set([
  'SHP_009','SHP_010','SHP_012','SHP_014','SHP_015','SHP_016',
  'SHP_018','SHP_019','SHP_020','GEO_006','GEO_007',
]);
function shapeIdForNotice(notice: Notice, nameIndex: NameIndex): string {
  if (SHAPE_ENTITY_RULES.has(notice.rule_id)) return notice.entity_id ?? '';
  if (['SHP_017','SHP_022','GEO_009'].includes(notice.rule_id)) return notice.details?.['shape_id'] ?? '';
  // buildMapOptions draws the shape for these rules from trip_shapes[trip_id] (STM_014/008/020/025/
  // 012/026/033/035, OPR_007/008, STM_017, generic trip). Use the same path for on-demand deferred mode.
  const eid = notice.entity_id ?? '';
  if (eid && eid in nameIndex.trip_shapes) return nameIndex.trip_shapes[eid] ?? '';
  // Route rules: geometry is not serialized for large feeds; fetch the route's first shape
  // (the route map draws up to 4 shapes, but opens with one in deferred mode).
  if (ROUTE_ID_RULES.has(notice.rule_id)) return nameIndex.route_shapes[eid]?.[0] ?? '';
  return '';
}

// BASE line color for maps with a highlighted segment. Gray (#9ca3af) was tried and FAILED:
// the Google "terrain" basemap (lyrs=p) is also gray-toned → the line blended into the background
// and became invisible. Black is readable on every basemap and does not compete with the red error segment.
const SHAPE_BASE_COLOR = '#111827';

function buildMapOptions(notice: Notice, nameIndex: NameIndex): MapOptions {
  const entityId = notice.entity_id ?? '';

  // SHP_017: faulty stop (red) + ±3 context stops (blue) + shape polyline.
  if (notice.rule_id === 'SHP_017') {
    const sid    = notice.details?.['bad_stop'] ?? '';
    const shapeId = notice.details?.['shape_id'] ?? '';
    const ctxB   = (notice.details?.['ctx_b'] ?? '').split(',').filter(Boolean);
    const ctxA   = (notice.details?.['ctx_a'] ?? '').split(',').filter(Boolean);
    const seqB   = (notice.details?.['seq_b'] ?? '').split(',').filter(Boolean);
    const seqA   = (notice.details?.['seq_a'] ?? '').split(',').filter(Boolean);
    const errSeq = notice.observed_value ?? '';

    const pins: MapPin[] = [];
    // Previous 3 stops — blue (not primary).
    for (let i = 0; i < ctxB.length; i++) {
      const seq = seqB[i] ? `#${seqB[i]} ` : '';
      const p = stopPin(ctxB[i], nameIndex, true, `${seq}${t('fix.map.pin.prev')}`);
      if (p) pushMapPin(pins, p);
    }
    // Faulty stop — red.
    const seqLabel = errSeq ? `#${errSeq} ` : '';
    const errPin = sid ? stopPin(sid, nameIndex, false, `${seqLabel}${t('fix.map.pin.bad_seq')}`) : null;
    if (errPin) pushMapPin(pins, errPin);
    // Next 3 stops — blue.
    for (let i = 0; i < ctxA.length; i++) {
      const seq = seqA[i] ? `#${seqA[i]} ` : '';
      const p = stopPin(ctxA[i], nameIndex, true, `${seq}${t('fix.map.pin.next')}`);
      if (p) pushMapPin(pins, p);
    }

    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    legendItems.push({ color: '#2563eb', label: t('fix.map.neighbor_stops') });
    legendItems.push({ color: '#dc2626', label: t('fix.map.bad_seq_stop') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: polyline.length > 1 };
  }

  // STM_014: two pins + the trip's shape.
  if (notice.rule_id === 'STM_014') {
    const m = notice.observed_value?.match(/\(([^→ ]+)\s*→\s*([^)]+)\)/);
    const pinA = m ? stopPin(m[1].trim(), nameIndex, true, t('fix.map.pin.depart')) : null;
    const pinB = m ? stopPin(m[2].trim(), nameIndex, false, t('fix.map.pin.arrive')) : null;
    const pins = [pinA, pinB].filter((p): p is MapPin => p !== null);
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 1) {
      legendItems.push({ color: '#2563eb', label: t('fix.map.depart_stop') });
      legendItems.push({ color: '#dc2626', label: t('fix.map.arrive_stop') });
    }
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: polyline.length > 1 };
  }

  // STM_008 / STM_020: two stop pins (departure/arrival) + the trip's shape.
  if (notice.rule_id === 'STM_020' || notice.rule_id === 'STM_008') {
    const pinA = notice.details?.['stop_a'] ? stopPin(notice.details['stop_a'], nameIndex, true, t('fix.map.pin.depart')) : null;
    const pinB = notice.details?.['stop_b'] ? stopPin(notice.details['stop_b'], nameIndex, false, t('fix.map.pin.arrive')) : null;
    const pins = [pinA, pinB].filter((p): p is MapPin => p !== null);
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.depart_stop') });
    if (pins.length > 1) legendItems.push({ color: '#dc2626', label: t('fix.map.arrive_stop') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_010: repeated shape point — polyline + red pin.
  if (notice.rule_id === 'SHP_010') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const m = notice.observed_value?.match(/^\(([^,]+),([^)]+)\)$/);
    const pins: MapPin[] = [];
    if (m) {
      const lat = parseFloat(m[1]);
      const lon = parseFloat(m[2]);
      if (!isNaN(lat) && !isNaN(lon)) {
        pushMapPin(pins, { lat, lon, label: `${t('fix.map.pin.dup_coord')}<br><code>(${escHtml(m[1])}, ${escHtml(m[2])})</code>`, primary: false });
      }
    }
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) legendItems.push({ color: '#dc2626', label: t('fix.map.dup_point') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_022: stop ambiguous in the shape — stop pin + shape polyline.
  if (notice.rule_id === 'SHP_022') {
    const shapeId = notice.details?.['shape_id'] ?? '';
    const pin = stopPin(entityId, nameIndex, false, t('fix.map.pin.ambiguous'));
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const pins = pin ? [pin] : [];
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) legendItems.push({ color: '#dc2626', label: t('fix.map.ambiguous_stop') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_018 / SHP_019: orphan shape — polyline only, no stop pin.
  if (notice.rule_id === 'SHP_018' || notice.rule_id === 'SHP_019') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const legendItems = polyline.length > 1 ? [{ color: '#f59e0b', label: t('fix.map.route_shape_unused') }] : [];
    return { pins: [], polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // OPR_007: repeated stop — all stops + shape + direction arrows.
  if (notice.rule_id === 'OPR_007') {
    const dupStopId = notice.details?.['dup_stop'] ?? '';
    const stops = (notice.details?.['stops'] ?? '').split(',').filter(Boolean);
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];

    const seen = new Set<string>();
    const pins: MapPin[] = [];
    for (const id of stops.slice(0, MAX_MAP_PINS)) {
      if (seen.has(id)) continue;
      seen.add(id);
      const isDup = id === dupStopId;
      const p = stopPin(id, nameIndex, !isDup, isDup ? t('fix.map.pin.repeated') : undefined);
      if (p) { p.small = !isDup; pushMapPin(pins, p); }
    }
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 1) legendItems.push({ color: '#2563eb', label: t('fix.map.stops') });
    if (dupStopId) legendItems.push({ color: '#dc2626', label: t('fix.map.repeated_stop') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // STM_017: trip shape (polyline) + all stops in the trip.
  if (notice.rule_id === 'STM_017') {
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const stopIds = entityId ? (nameIndex.trip_stops[entityId] ?? []) : [];
    const pins: MapPin[] = stopIds
      .map(id => stopPin(id, nameIndex, true))
      .filter((p): p is MapPin => p !== null)
      .map(p => ({ ...p, small: true }));
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // GEO_006 / GEO_007: shape jump — shape gray, JUMPING SEGMENT red + two endpoint pins.
  // The jump was already drawn as part of the polyline; because it had the same color, it was
  // impossible to see which part was wrong. Draw the segment over it in a separate color and zoom to it.
  if (notice.rule_id === 'GEO_006' || notice.rule_id === 'GEO_007') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const m = notice.observed_value?.match(/segment\s+(\d+)→(\d+)/);
    const pins: MapPin[] = [];
    const extraPolylines: Array<{ coords: [number, number][]; color: string; weight: number; zoomTo: boolean }> = [];
    if (m && polyline.length > 0) {
      const idxA = parseInt(m[1], 10);
      const idxB = parseInt(m[2], 10);
      const ptA = polyline[idxA];
      const ptB = polyline[idxB];
      if (ptA) pushMapPin(pins, { lat: ptA[0], lon: ptA[1], label: `<strong>${t('fix.map.pin.jump_start')}</strong><br>Segment ${idxA}`, primary: true });
      if (ptB) pushMapPin(pins, { lat: ptB[0], lon: ptB[1], label: `<strong>${t('fix.map.pin.jump_end')}</strong><br>Segment ${idxB}`, primary: false });
      if (ptA && ptB) {
        // NO zoomTo (2026-07-25): zooming to the segment pushed the rest of the shape off-screen
        // and left a single line without context ("not visible on the map"). Fit the whole shape;
        // distinguish the jump with its color and width.
        extraPolylines.push({ coords: [[ptA[0], ptA[1]], [ptB[0], ptB[1]]], color: '#dc2626', weight: 7, zoomTo: false });
      }
    }
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: SHAPE_BASE_COLOR, label: t('fix.map.route_shape') });
    if (extraPolylines.length > 0) legendItems.push({ color: '#dc2626', label: t('fix.map.jump_segment') });
    if (pins.length > 0) {
      // Colors must match the pins: start is primary (blue), end is reference (red).
      legendItems.push({ color: '#2563eb', label: t('fix.map.jump_start') });
      legendItems.push({ color: '#dc2626', label: t('fix.map.jump_end') });
    }
    return {
      pins,
      polyline: polyline.length > 1 ? polyline : undefined,
      polylineColor: SHAPE_BASE_COLOR,
      extraPolylines,
      legendItems,
      showArrows: false,
    };
  }

  // SHP_020: non-consecutive repeated point — polyline + red pin.
  if (notice.rule_id === 'SHP_020') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    // observed_value: "nokta {i} ve {j}: ({lat},{lon})"
    const m = notice.observed_value?.match(/\(([^,]+),([^)]+)\)$/);
    const pins: MapPin[] = [];
    if (m) {
      const lat = parseFloat(m[1]);
      const lon = parseFloat(m[2]);
      if (!isNaN(lat) && !isNaN(lon)) {
        pushMapPin(pins, { lat, lon, label: `${t('fix.map.pin.repeat_coord')}<br><code>(${escHtml(m[1])}, ${escHtml(m[2])})</code>`, primary: false });
      }
    }
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) legendItems.push({ color: '#dc2626', label: t('fix.map.repeat_point') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_009: shape intersects itself — polyline + endpoints of the intersecting segment.
  if (notice.rule_id === 'SHP_009') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    // observed_value: "segment {a}-{b} ∩ {c}-{d}"
    const m = notice.observed_value?.match(/segment (\d+)-(\d+) ∩ (\d+)-(\d+)/);
    const pins: MapPin[] = [];
    if (m && polyline.length > 0) {
      const a = parseInt(m[1], 10), b = parseInt(m[2], 10);
      const c = parseInt(m[3], 10), d = parseInt(m[4], 10);
      const ptA = polyline[a], ptB = polyline[b];
      const ptC = polyline[c], ptD = polyline[d];
      if (ptA) pushMapPin(pins, { lat: ptA[0], lon: ptA[1], label: t('fix.map.pin.seg_start', { a: String(a), b: String(b) }), primary: true });
      if (ptB) pushMapPin(pins, { lat: ptB[0], lon: ptB[1], label: t('fix.map.pin.seg_end',   { a: String(a), b: String(b) }), primary: true });
      if (ptC) pushMapPin(pins, { lat: ptC[0], lon: ptC[1], label: t('fix.map.pin.seg_start', { a: String(c), b: String(d) }), primary: false });
      if (ptD) pushMapPin(pins, { lat: ptD[0], lon: ptD[1], label: t('fix.map.pin.seg_end',   { a: String(c), b: String(d) }), primary: false });
    }
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) {
      legendItems.push({ color: '#2563eb', label: t('fix.map.segment_1') });
      legendItems.push({ color: '#dc2626', label: t('fix.map.segment_2') });
    }
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // OPR_008: entire shape gray, all faulty segments red.
  if (notice.rule_id === 'OPR_008') {
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: SHAPE_BASE_COLOR, label: t('fix.map.route_shape') });

    // Collect all faulty segments (bad_seg_0_a/b, bad_seg_1_a/b, ...).
    const extraPolylines: Array<{ coords: [number, number][]; color: string; weight: number; zoomTo: boolean }> = [];
    const pins: import('../map-modal').MapPin[] = [];
    const seenStops = new Set<string>();

    for (let i = 0; ; i++) {
      const stopA = notice.details?.[`bad_seg_${i}_a`];
      const stopB = notice.details?.[`bad_seg_${i}_b`];
      if (!stopA || !stopB) break;
      const coordA = nameIndex.stop_coords[stopA];
      const coordB = nameIndex.stop_coords[stopB];
      if (coordA && coordB) {
        extraPolylines.push({
          coords: [[coordA[0], coordA[1]], [coordB[0], coordB[1]]],
          color: '#dc2626',
          weight: 5,
          zoomTo: i === 0,
        });
      }
      if (coordA && !seenStops.has(stopA)) {
        seenStops.add(stopA);
        pushMapPin(pins, { lat: coordA[0], lon: coordA[1], label: `<code>${escHtml(stopA)}</code>`, primary: true, small: false });
      }
      if (coordB && !seenStops.has(stopB)) {
        seenStops.add(stopB);
        pushMapPin(pins, { lat: coordB[0], lon: coordB[1], label: `<code>${escHtml(stopB)}</code>`, primary: true, small: false });
      }
    }

    if (extraPolylines.length > 0) legendItems.push({ color: '#dc2626', label: t('fix.map.bad_segment') });
    legendItems.push({ color: '#2563eb', label: t('fix.map.segment_ends') });

    return {
      pins,
      polyline: polyline.length > 1 ? polyline.map(p => [p[0], p[1]] as [number, number]) : undefined,
      polylineColor: SHAPE_BASE_COLOR,
      extraPolylines,
      legendItems,
      showArrows: false,
    };
  }

  // STM_025: short segment — shape (yellow) + all stops (small blue) + 2 problematic stops (blue/red).
  if (notice.rule_id === 'STM_025') {
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const stopIds = entityId ? (nameIndex.trip_stops[entityId] ?? []) : [];
    const stopA = notice.details?.['stop_a'] ?? '';
    const stopB = notice.details?.['stop_b'] ?? '';

    const pins: import('../map-modal').MapPin[] = [];
    for (const id of stopIds.slice(0, MAX_MAP_PINS)) {
      if (id === stopA || id === stopB) continue;
      const p = stopPin(id, nameIndex, true);
      if (p) pushMapPin(pins, { ...p, small: true });
    }
    const pinA = stopA ? stopPin(stopA, nameIndex, true, t('fix.map.pin.depart')) : null;
    if (pinA) pushMapPin(pins, pinA);
    const pinB = stopB ? stopPin(stopB, nameIndex, false, t('fix.map.pin.arrive_short')) : null;
    if (pinB) pushMapPin(pins, pinB);

    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (stopIds.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    legendItems.push({ color: '#dc2626', label: t('fix.map.short_segment_end') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // OPR_015: single-shape warning — shape polyline only.
  if (notice.rule_id === 'OPR_015') {
    const shapeId = notice.details?.['shape_id'] ?? '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const legendItems = polyline.length > 1 ? [{ color: '#f59e0b', label: t('fix.map.route_shape_single') }] : [];
    return { pins: [], polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // ── Shape rules: entity_id = shape_id, find stops through shape_trips ──

  // Helper: return the stops of trips using a shape as small blue pins.
  function shapeStopPins(shapeId: string): MapPin[] {
    const tripId = nameIndex.shape_trips[shapeId] ?? '';
    const stopIds = tripId ? (nameIndex.trip_stops[tripId] ?? []) : [];
    return stopIds.slice(0, MAX_MAP_PINS)
      .map(id => stopPin(id, nameIndex, true))
      .filter((p): p is MapPin => p !== null)
      .map(p => ({ ...p, small: true }));
  }

  // SHP_012: shape + stops; stops ABOVE THE THRESHOLD are red and large (details.far_stops).
  if (notice.rule_id === 'SHP_012') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const far = new Set((notice.details?.['far_stops'] ?? '').split(',').map(s => s.trim()).filter(Boolean));
    const tripId = nameIndex.shape_trips[entityId] ?? '';
    const stopIds = tripId ? (nameIndex.trip_stops[tripId] ?? []) : [];
    const pins: MapPin[] = [];
    for (const id of stopIds.slice(0, MAX_MAP_PINS)) {
      if (far.has(id)) continue;                       // far stops are added below, keep them on top
      const p = stopPin(id, nameIndex, true);
      if (p) pushMapPin(pins, { ...p, small: true });
    }
    let farDrawn = 0;
    for (const id of far) {
      const p = stopPin(id, nameIndex, false, t('fix.map.pin.far_from_shape'));
      if (p) { pushMapPin(pins, p); farDrawn++; }
    }
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > farDrawn) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    if (farDrawn > 0) legendItems.push({ color: '#dc2626', label: t('fix.map.far_stop_from_shape') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_014: first/last stop far from the shape endpoint — shape + stops + highlighted faulty stop.
  if (notice.rule_id === 'SHP_014') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const problemStopId = notice.details?.['problem_stop'] ?? '';
    const tripId = notice.details?.['trip_id'] ?? '';
    const stopIds = tripId ? (nameIndex.trip_stops[tripId] ?? []) : [];
    const pins: MapPin[] = [];
    for (const id of stopIds.slice(0, MAX_MAP_PINS)) {
      if (id === problemStopId) continue;
      const p = stopPin(id, nameIndex, true);
      if (p) pushMapPin(pins, { ...p, small: true });
    }
    const endpoint = notice.details?.['endpoint'] === 'start' ? t('fix.map.pin.first_far_start') : t('fix.map.pin.last_far_end');
    const errPin = problemStopId ? stopPin(problemStopId, nameIndex, false, endpoint) : null;
    if (errPin) pushMapPin(pins, errPin);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (stopIds.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    if (errPin) legendItems.push({ color: '#dc2626', label: t('fix.map.far_endpoint_stop') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_015: insufficient shape point density — shape + stops.
  if (notice.rule_id === 'SHP_015') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const pins = shapeStopPins(entityId);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_016: shape runs in reverse — shape + stops + first stop red (error point).
  if (notice.rule_id === 'SHP_016') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const firstStopId = notice.details?.['first_stop'] ?? '';
    const tripId = notice.details?.['trip_id'] ?? '';
    const stopIds = tripId ? (nameIndex.trip_stops[tripId] ?? []) : [];
    const pins: MapPin[] = [];
    for (const id of stopIds.slice(0, MAX_MAP_PINS)) {
      if (id === firstStopId) continue;
      const p = stopPin(id, nameIndex, true);
      if (p) pushMapPin(pins, { ...p, small: true });
    }
    const errPin = firstStopId ? stopPin(firstStopId, nameIndex, false, t('fix.map.pin.first_wrong')) : null;
    if (errPin) pushMapPin(pins, errPin);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape_rev') });
    if (stopIds.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    if (errPin) legendItems.push({ color: '#dc2626', label: t('fix.map.first_stop_wrong_end') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // ── Trip rules: entity_id = trip_id ─────────────────────────────────────────

  // STM_012: physically impossible speed — shape + all stops + 2 problematic stops.
  if (notice.rule_id === 'STM_012') {
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const stopIds = entityId ? (nameIndex.trip_stops[entityId] ?? []) : [];
    const stopA = notice.details?.['stop_a'] ?? '';
    const stopB = notice.details?.['stop_b'] ?? '';
    const pins: MapPin[] = [];
    for (const id of stopIds.slice(0, MAX_MAP_PINS)) {
      if (id === stopA || id === stopB) continue;
      const p = stopPin(id, nameIndex, true);
      if (p) pushMapPin(pins, { ...p, small: true });
    }
    const pinA = stopA ? stopPin(stopA, nameIndex, true, t('fix.map.pin.depart_speed')) : null;
    const pinB = stopB ? stopPin(stopB, nameIndex, false, t('fix.map.pin.arrive_speed')) : null;
    if (pinA) pushMapPin(pins, pinA);
    if (pinB) pushMapPin(pins, pinB);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (stopIds.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    legendItems.push({ color: '#dc2626', label: t('fix.map.impossible_speed') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // STM_026: distance between stops is too long — shape + all stops + 2 distant stops.
  if (notice.rule_id === 'STM_026') {
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const stopIds = entityId ? (nameIndex.trip_stops[entityId] ?? []) : [];
    const stopA = notice.details?.['stop_a'] ?? '';
    const stopB = notice.details?.['stop_b'] ?? '';
    const pins: MapPin[] = [];
    for (const id of stopIds.slice(0, MAX_MAP_PINS)) {
      if (id === stopA || id === stopB) continue;
      const p = stopPin(id, nameIndex, true);
      if (p) pushMapPin(pins, { ...p, small: true });
    }
    const pinA = stopA ? stopPin(stopA, nameIndex, true, t('fix.map.pin.depart')) : null;
    const pinB = stopB ? stopPin(stopB, nameIndex, false, t('fix.map.pin.arrive_far')) : null;
    if (pinA) pushMapPin(pins, pinA);
    if (pinB) pushMapPin(pins, pinB);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (stopIds.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    legendItems.push({ color: '#dc2626', label: t('fix.map.far_segment_end') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // STM_033: single-stop trip — shape + the single stop.
  if (notice.rule_id === 'STM_033') {
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const stopIds = entityId ? (nameIndex.trip_stops[entityId] ?? []) : [];
    const pins: MapPin[] = stopIds
      .map(id => stopPin(id, nameIndex, false, t('fix.map.pin.single_stop')))
      .filter((p): p is MapPin => p !== null);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    legendItems.push({ color: '#dc2626', label: t('fix.map.single_stop_trip') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: polyline.length > 1 };
  }

  // STM_035: same stop twice in succession — shape + all stops + repeated stop in red.
  if (notice.rule_id === 'STM_035') {
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const stopIds = entityId ? (nameIndex.trip_stops[entityId] ?? []) : [];
    const repeatStopId = notice.observed_value ?? '';
    const pins: MapPin[] = [];
    for (const id of stopIds.slice(0, MAX_MAP_PINS)) {
      if (id === repeatStopId) continue;
      const p = stopPin(id, nameIndex, true);
      if (p) pushMapPin(pins, { ...p, small: true });
    }
    const errPin = repeatStopId ? stopPin(repeatStopId, nameIndex, false, t('fix.map.pin.revisit')) : null;
    if (errPin) pushMapPin(pins, errPin);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (stopIds.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    if (errPin) legendItems.push({ color: '#dc2626', label: t('fix.map.revisit_stop') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: polyline.length > 1 };
  }

  // ── Pathway rules ─────────────────────────────────────────────────────────

  // PTH_014: pathway between different stations — two station pins + dashed connection.
  if (notice.rule_id === 'PTH_014') {
    const fromId = notice.details?.['from_station'] ?? '';
    const toId   = notice.details?.['to_station']   ?? '';
    const fromCoord = nameIndex.stop_coords[fromId];
    const toCoord   = nameIndex.stop_coords[toId];
    // hasMapCoords requires coordinates for both stations; this branch is defensive.
    if (!fromCoord || !toCoord) return { pins: [], legendItems: [], showArrows: false };
    const fromName = nameIndex.stops[fromId] ?? fromId;
    const toName   = nameIndex.stops[toId]   ?? toId;
    const pins: MapPin[] = [
      { lat: fromCoord[0], lon: fromCoord[1], label: t('fix.map.pin.station', { name: escHtml(fromName) }), primary: true },
      { lat: toCoord[0],   lon: toCoord[1],   label: t('fix.map.pin.station', { name: escHtml(toName) }),   primary: false },
    ];
    return {
      pins,
      extraPolylines: [{
        coords: [[fromCoord[0], fromCoord[1]], [toCoord[0], toCoord[1]]],
        color: '#dc2626',
        weight: 2,
        zoomTo: true,
      }],
      legendItems: [],
      showArrows: false,
    };
  }

  // ── Analytics / VAT rules ─────────────────────────────────────────────────

  // VAT_005: isolated stop cluster — connected stops (small blue) + isolated stops (red).
  if (notice.rule_id === 'VAT_005') {
    const isolatedSet = new Set((notice.details?.['isolated_stops'] ?? '').split(',').filter(Boolean));
    // Show only stops that actually occur in stop_times — stops without stop_times would appear
    // "connected" in blue, which would be misleading.
    const stopsInTrips = new Set<string>();
    // Aggregate {route set, trip count} for isolated stops (client-side).
    // Since an isolated cluster has ≤200 stops, accumulate only isolated stops.
    const stopRoutes = new Map<string, Set<string>>(); // stop_id → route_id set
    const stopTrips  = new Map<string, number>();       // stop_id → distinct trip count
    for (const [tripId, stops] of Object.entries(nameIndex.trip_stops).slice(0, MAX_MAP_PINS)) {
      const routeId = nameIndex.trip_routes[tripId];
      const seenInTrip = new Set<string>();
      for (const s of stops.slice(0, MAX_MAP_PINS)) {
        stopsInTrips.add(s);
        if (!isolatedSet.has(s) || seenInTrip.has(s)) continue;
        seenInTrip.add(s);
        stopTrips.set(s, (stopTrips.get(s) ?? 0) + 1);
        if (routeId) {
          let rset = stopRoutes.get(s);
          if (!rset) { rset = new Set<string>(); stopRoutes.set(s, rset); }
          rset.add(routeId);
        }
      }
    }
    const pins: MapPin[] = [];
    for (const [sid, coord] of Object.entries(nameIndex.stop_coords).slice(0, MAX_MAP_PINS)) {
      const isIsolated = isolatedSet.has(sid);
      const inTrips = stopsInTrips.has(sid);
      if (!isIsolated && !inTrips) continue; // hide stops without stop_times
      const name = nameIndex.stops[sid] ?? sid;
      let label: string;
      if (isIsolated) {
        const routeIds = Array.from(stopRoutes.get(sid) ?? []);
        const tripCount = stopTrips.get(sid) ?? 0;
        const routeCodes = routeIds
          .map(rid => nameIndex.routes[rid] ?? rid)
          .filter(Boolean);
        label = `<strong>${t('fix.map.pin.isolated')}</strong><br>${stopLabel(name, sid)}`;
        if (routeIds.length > 0 || tripCount > 0) {
          label += `<br><span>${escHtml(t('fix.map.pin.isolated_routes', {
            routes: String(routeIds.length),
            trips: String(tripCount),
          }))}</span>`;
        }
        if (routeCodes.length > 0) {
          label += `<br><code>${escHtml(routeCodes.join(', '))}</code>`;
        }
      } else {
        label = stopLabel(name, sid);
      }
      pushMapPin(pins, {
        lat: coord[0],
        lon: coord[1],
        label,
        primary: !isIsolated,
        small: !isIsolated,
      });
    }
    return {
      pins,
      legendItems: [
        { color: '#2563eb', label: t('fix.map.connected_stops') },
        { color: '#dc2626', label: t('fix.map.isolated_stops') },
      ],
    };
  }

  // VAT_007: terminus stop — terminal stop (red) + route shapes (different colors).
  if (notice.rule_id === 'VAT_007') {
    const routeIds = (notice.details?.['routes'] ?? '').split(',').filter(Boolean);
    const routeColors = ['#3b82f6','#10b981','#8b5cf6','#f97316','#06b6d4'];
    const coords = nameIndex.stop_coords[entityId];
    const stopName = nameIndex.stops[entityId] ?? entityId;
    const pins: MapPin[] = coords
      ? [{ lat: coords[0], lon: coords[1], label: `<strong>${t('fix.map.pin.terminal', { name: escHtml(stopName) })}</strong><br><code>${escHtml(entityId)}</code>`, primary: false }]
      : [];
    const extraPolylines: Array<{ coords: [number,number][]; color: string; weight: number; zoomTo: boolean }> = [];
    const legendItems: Array<{ color: string; label: string }> = [];
    if (pins.length > 0) legendItems.push({ color: '#dc2626', label: t('fix.map.terminal_stop') });
    let colorIdx = 0;
    for (const rId of routeIds.slice(0, 5)) {
      const shapeIds = nameIndex.route_shapes[rId] ?? [];
      const color = routeColors[colorIdx % routeColors.length];
      let added = false;
      for (const shapeId of shapeIds.slice(0, 2)) {
        const pts = nameIndex.shape_coords[shapeId] ?? [];
        if (pts.length > 1) {
          extraPolylines.push({ coords: pts as [number,number][], color, weight: 3, zoomTo: false });
          added = true;
        }
      }
      if (added) {
        const routeName = nameIndex.routes[rId] ?? rId;
        legendItems.push({ color, label: escHtml(routeName) });
        colorIdx++;
      }
    }
    return { pins, extraPolylines, legendItems, showArrows: false };
  }

  // VAT_002: transfer hub undefined — stop (red) + passing routes in different colors.
  if (notice.rule_id === 'VAT_002') {
    const routeIds = (notice.details?.['routes'] ?? '').split(',').filter(Boolean);
    const routeColors = ['#3b82f6', '#10b981', '#8b5cf6', '#f97316', '#06b6d4', '#eab308'];
    const coords = nameIndex.stop_coords[entityId];
    const stopName = nameIndex.stops[entityId] ?? entityId;
    const pins: MapPin[] = coords
      ? [{ lat: coords[0], lon: coords[1], label: `<strong>${escHtml(stopName)}</strong><br><code>${escHtml(entityId)}</code>`, primary: false }]
      : [];
    const extraPolylines: Array<{ coords: [number,number][]; color: string; weight: number; zoomTo: boolean }> = [];
    const legendItems: Array<{ color: string; label: string }> = [];
    if (pins.length > 0) legendItems.push({ color: '#dc2626', label: t('fix.map.transfer_hub_stop') });
    let colorIdx = 0;
    for (const rId of routeIds.slice(0, 6)) {
      const shapeIds = nameIndex.route_shapes[rId] ?? [];
      const color = routeColors[colorIdx % routeColors.length];
      let added = false;
      for (const shapeId of shapeIds.slice(0, 2)) {
        const pts = nameIndex.shape_coords[shapeId] ?? [];
        if (pts.length > 1) {
          extraPolylines.push({ coords: pts as [number,number][], color, weight: 3, zoomTo: false });
          added = true;
        }
      }
      if (added) {
        const routeName = nameIndex.routes[rId] ?? rId;
        legendItems.push({ color, label: escHtml(routeName) });
        colorIdx++;
      }
    }
    return { pins, extraPolylines, legendItems, showArrows: false };
  }

  // VAT_001: likely duplicate routes — draw the two routes in different colors + show each route's
  // first/last stop in the same route color (the endpoints remain distinguishable even when shapes overlap).
  if (notice.rule_id === 'VAT_001') {
    const routeIds = (notice.details?.['routes'] ?? '').split(',').filter(Boolean);
    const routeColors = ['#3b82f6', '#f97316']; // mavi vs turuncu
    const extraPolylines: Array<{ coords: [number,number][]; color: string; weight: number; zoomTo: boolean }> = [];
    const pins: MapPin[] = [];
    const legendItems: Array<{ color: string; label: string }> = [];
    let colorIdx = 0;
    for (const rId of routeIds.slice(0, 2)) {
      const shapeIds = nameIndex.route_shapes[rId] ?? [];
      const color = routeColors[colorIdx % routeColors.length];
      let added = false;
      for (const shapeId of shapeIds.slice(0, 3)) {
        const pts = nameIndex.shape_coords[shapeId] ?? [];
        if (pts.length > 1) {
          extraPolylines.push({ coords: pts as [number,number][], color, weight: 3, zoomTo: true });
          added = true;
        }
      }
      if (added) {
        const routeName = nameIndex.routes[rId] ?? rId;
        legendItems.push({ color, label: escHtml(routeName) });

        // Find the route's first/last stop from a trip using its first shape.
        const firstShape = shapeIds[0] ?? '';
        const tripId = firstShape ? (nameIndex.shape_trips[firstShape] ?? '') : '';
        const stopIds = tripId ? (nameIndex.trip_stops[tripId] ?? []) : [];
        if (stopIds.length > 0) {
          const depId = stopIds[0];
          const arrId = stopIds[stopIds.length - 1];
          const depCoord = nameIndex.stop_coords[depId];
          const arrCoord = nameIndex.stop_coords[arrId];
          if (depCoord) {
            const depName = nameIndex.stops[depId] ?? depId;
            pushMapPin(pins, { lat: depCoord[0], lon: depCoord[1], color, primary: true,
              label: `<strong>${escHtml(routeName)} — ${t('fix.map.pin.depart')}</strong><br>${stopLabel(depName, depId)}` });
          }
          if (arrCoord && arrId !== depId) {
            const arrName = nameIndex.stops[arrId] ?? arrId;
            pushMapPin(pins, { lat: arrCoord[0], lon: arrCoord[1], color, primary: true, small: true,
              label: `<strong>${escHtml(routeName)} — ${t('fix.map.pin.arrive')}</strong><br>${stopLabel(arrName, arrId)}` });
          }
        }
        colorIdx++;
      }
    }
    if (pins.length > 0) {
      legendItems.push({ color: '#111827', label: t('fix.map.depart_arrive_stops') });
    }
    return { pins, extraPolylines, legendItems, showArrows: false };
  }

  // GEO_017 / GEO_020: shape geometry error — red pin at the coordinates in observed_value.
  // In GEO_020, the shape's ALL points are at the same location, so the polyline is invisible
  // even when drawn (zero length); one pin is the correct display. Draw the shape behind it for GEO_017 when present.
  if (notice.rule_id === 'GEO_017' || notice.rule_id === 'GEO_020') {
    const m = (notice.observed_value ?? '').trim().match(/^(-?\d+(?:\.\d+)?),(-?\d+(?:\.\d+)?)$/);
    const pins: MapPin[] = [];
    if (m) {
      const plat = parseFloat(m[1]);
      const plon = parseFloat(m[2]);
      if (!isNaN(plat) && !isNaN(plon)) {
        const label = notice.rule_id === 'GEO_020'
          ? t('fix.map.degenerate_shape')
          : t('fix.map.null_island_point');
        pushMapPin(pins, { lat: plat, lon: plon, label: `<strong>${label}</strong><br><code>${escHtml(entityId)}</code><br><code>${escHtml(m[1])}, ${escHtml(m[2])}</code>`, primary: false });
      }
    }
    const polyline = notice.rule_id === 'GEO_017' && entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) {
      legendItems.push({ color: '#dc2626', label: notice.rule_id === 'GEO_020' ? t('fix.map.degenerate_shape') : t('fix.map.null_island_point') });
    }
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems };
  }

  // GEO_013 / GEO_018 / GEO_021: show all stops in the feed as small pins.
  // For all three, the evidence is the distribution itself: summary (013), all within 200m (018),
  // and coordinates piled on top of one another (021).
  if (['GEO_013','GEO_018','GEO_021'].includes(notice.rule_id)) {
    const pins: MapPin[] = Object.entries(nameIndex.stop_coords).slice(0, MAX_MAP_PINS).map(([stopId, coord]) => ({
      lat: coord[0],
      lon: coord[1],
      label: nameIndex.stops[stopId]
        ? stopLabel(nameIndex.stops[stopId], stopId)
        : `<code>${escHtml(stopId)}</code>`,
      primary: true,
      small: true,
    }));
    return {
      pins,
      legendItems: [{ color: '#2563eb', label: t('fix.map.n_stops', { n: String(pins.length) }) }],
    };
  }

  // ── Generic trip map: entity_id = trip_id ─────────────────────────────────
  if (TRIP_ID_RULES.has(notice.rule_id)) {
    const shapeId  = nameIndex.trip_shapes[entityId] ?? '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const stopIds  = nameIndex.trip_stops[entityId] ?? [];
    const routeId  = nameIndex.trip_routes[entityId] ?? '';
    const routeName = routeId ? (nameIndex.routes[routeId] ?? '') : '';

    const pins: MapPin[] = stopIds.flatMap((sid, idx) => {
      const p = stopPin(sid, nameIndex, true, `#${idx + 1}`);
      return p ? [{ ...p, small: true as const }] : [];
    });

    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: routeName ? t('fix.map.route_shape_named', { name: escHtml(routeName) }) : t('fix.map.route_shape') });
    if (pins.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: polyline.length > 1 };
  }

  // ── Generic route map: entity_id = route_id ───────────────────────────────
  if (ROUTE_ID_RULES.has(notice.rule_id)) {
    const shapeIds  = nameIndex.route_shapes[entityId] ?? [];
    const routeName = nameIndex.routes[entityId] ?? entityId;
    const extraPolylines: Array<{ coords: [number,number][]; color: string; weight: number; zoomTo: boolean }> = [];
    for (const shapeId of shapeIds.slice(0, 4)) {
      const pts = nameIndex.shape_coords[shapeId] ?? [];
      if (pts.length > 1) extraPolylines.push({ coords: pts as [number,number][], color: '#f59e0b', weight: 3, zoomTo: extraPolylines.length === 0 });
    }
    if (extraPolylines.length === 0) return { pins: [] };
    return {
      pins: [],
      extraPolylines,
      legendItems: [{ color: '#f59e0b', label: t('fix.map.route_shapes_named', { name: escHtml(routeName) }) }],
      showArrows: false,
    };
  }

  const coords = nameIndex.stop_coords[entityId];
  if (!coords) return { pins: [] };
  const [lat, lon] = coords;
  const stopName = nameIndex.stops[entityId] ?? entityId;

  // STP_029: child + parent istasyon
  if (notice.rule_id === 'STP_029') {
    const parentId = notice.details?.['parent_id'] ?? '';
    const pins: MapPin[] = [
      { lat, lon, label: stopLabel(stopName, entityId), primary: true },
    ];
    const parentPin = parentId ? stopPin(parentId, nameIndex, false, t('fix.map.pin.parent')) : null;
    if (parentPin) pushMapPin(pins, parentPin);
    const legendItems = pins.length > 1
      ? [{ color: '#2563eb', label: t('fix.map.stop') }, { color: '#dc2626', label: t('fix.map.parent_station_label') }]
      : [];
    return { pins, legendItems };
  }

  // STP_016: two stops at the same coordinates.
  if (notice.rule_id === 'STP_016') {
    const secondId = notice.observed_value?.match(/== '([^']+)'/)?.[1] ?? '';
    const secondName = secondId ? (nameIndex.stops[secondId] ?? secondId) : '';
    const label = secondName
      ? `<strong>${escHtml(stopName)}</strong> = <strong>${escHtml(secondName)}</strong><br><code>${escHtml(entityId)}</code> ve <code>${escHtml(secondId)}</code>`
      : stopLabel(stopName, entityId);
    return { pins: [{ lat, lon, label, primary: true }] };
  }

  // STP_017: two nearby stops — the second comes from observed_value.
  if (notice.rule_id === 'STP_017') {
    const secondId = notice.observed_value?.match(/to '([^']+)'/)?.[1] ?? '';
    const pins: MapPin[] = [{ lat, lon, label: stopLabel(stopName, entityId), primary: true }];
    const pinB = secondId ? stopPin(secondId, nameIndex, false) : null;
    if (pinB) pushMapPin(pins, pinB);
    const legendItems = pins.length > 1
      ? [{ color: '#2563eb', label: `${t('fix.map.stop')} 1` }, { color: '#dc2626', label: `${t('fix.map.stop')} 2` }]
      : [];
    return { pins, legendItems };
  }

  // GEO_002: distant stop + feed median + line showing the distance between them.
  // One pin was meaningless: the rule says "X km from the feed median", but the reference point
  // haritada yoktu. med_lat/med_lon notice.details'tan gelir (k6_analytics).
  if (notice.rule_id === 'GEO_002') {
    const medLat = parseFloat(notice.details?.['med_lat'] ?? '');
    const medLon = parseFloat(notice.details?.['med_lon'] ?? '');
    const distKm = notice.details?.['dist_km'] ?? '';
    const pins: MapPin[] = [
      { lat, lon, label: `<strong>${escHtml(stopName)}</strong><br><code>${escHtml(entityId)}</code>${distKm ? `<br>${t('fix.map.far_from_median', { km: distKm })}` : ''}`, primary: false },
    ];
    const extraPolylines: Array<{ coords: [number, number][]; color: string; weight: number; zoomTo: boolean }> = [];
    const legendItems: Array<{ color: string; label: string }> = [
      { color: '#dc2626', label: t('fix.map.far_stop') },
    ];
    if (!isNaN(medLat) && !isNaN(medLon)) {
      pushMapPin(pins, { lat: medLat, lon: medLon, label: `<strong>${t('fix.map.feed_median')}</strong>`, primary: true });
      extraPolylines.push({ coords: [[lat, lon], [medLat, medLon]], color: '#dc2626', weight: 2, zoomTo: false });
      legendItems.push({ color: '#2563eb', label: t('fix.map.feed_median') });
    }
    return { pins, extraPolylines, legendItems };
  }

  // GEO_009: stop pin + shape polyline. shape_id comes from details; the observed_value regex
  // remains as a fallback for compatibility with older reports.
  if (notice.rule_id === 'GEO_009') {
    const shapeId = notice.details?.['shape_id']
      ?? notice.observed_value?.match(/\(shape '([^']+)'\)/)?.[1] ?? '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const legendItems: Array<{ color: string; label: string }> = [
      { color: '#2563eb', label: t('fix.map.stop') },
    ];
    if (polyline.length > 0) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    return {
      pins: [{ lat, lon, label: stopLabel(stopName, entityId), primary: true }],
      polyline: polyline.length > 1 ? polyline : undefined,
      legendItems,
    };
  }

  // Other stop rules — single pin.
  return { pins: [{ lat, lon, label: stopLabel(stopName, entityId), primary: true }] };
}

export function attachFixListeners(root: HTMLElement, result?: ValidationResult, cappedTotals?: Record<string, number>): void {
  // Column-info (ℹ) tooltips — hover + click + keyboard (instead of native title).
  cinfoWire(root);

  attachHiddenRuleListeners(root);

  // R9 expandable rows — blocks[] context.
  root.querySelectorAll<HTMLTableRowElement>('.r9-main-row').forEach(row => {
    row.addEventListener('click', () => {
      const idx = row.dataset['idx'];
      const detail = root.querySelector<HTMLTableRowElement>(`.r9-detail-row[data-for="${idx}"]`);
      if (!detail) return;
      detail.hidden = !detail.hidden;
      const arrow = row.querySelector<HTMLSpanElement>('.r9-arrow');
      if (arrow) arrow.textContent = detail.hidden ? '▶' : '▼';
    });
  });

  // R9 column sorting + SEVERITY filter.
  const r9Table = root.querySelector<HTMLTableElement>('#r9-table');
  const r9Tbody = r9Table?.querySelector('tbody');
  if (r9Table && r9Tbody) {
    let sortCol = -1, sortDir = 1;
    const cellTxt = (row: HTMLTableRowElement, col: number) =>
      (row.children[col] as HTMLElement | undefined)?.textContent?.trim() ?? '';
    r9Table.querySelectorAll<HTMLTableCellElement>('th.r9-sortable').forEach(th => {
      th.addEventListener('click', e => {
        if ((e.target as HTMLElement).closest('.col-info')) return; // tooltip icon click
        const col = parseInt(th.dataset['col'] ?? '0', 10);
        const type = th.dataset['type'] ?? 'text';
        sortDir = sortCol === col ? -sortDir : 1;
        sortCol = col;
        const mains = Array.from(r9Tbody.querySelectorAll<HTMLTableRowElement>('.r9-main-row'));
        mains.sort((a, b) => {
          if (type === 'num') {
            const na = parseFloat((a.children[col] as HTMLElement).dataset['val'] ?? '0');
            const nb = parseFloat((b.children[col] as HTMLElement).dataset['val'] ?? '0');
            return (na - nb) * sortDir;
          }
          if (type === 'sev') {
            return ((SEVERITY_RANK[(a.dataset['sev'] ?? 'INFO') as Severity] ?? 9) - (SEVERITY_RANK[(b.dataset['sev'] ?? 'INFO') as Severity] ?? 9)) * sortDir;
          }
          return cellTxt(a, col).localeCompare(cellTxt(b, col), intlLocale()) * sortDir;
        });
        for (const main of mains) {
          const detail = r9Tbody.querySelector<HTMLTableRowElement>(`.r9-detail-row[data-for="${main.dataset['idx']}"]`);
          r9Tbody.appendChild(main);
          if (detail) r9Tbody.appendChild(detail);
        }
        r9Table.querySelectorAll('.sort-ind').forEach(s => { s.textContent = ''; });
        const ind = th.querySelector('.sort-ind');
        if (ind) ind.textContent = sortDir > 0 ? '▲' : '▼';
      });
    });

    // SEVERITY filter: click a chip → show selected severities (all when none are selected), multi-select.
    const sevFilter = root.querySelector('#r9-sev-filter');
    if (sevFilter) {
      const active = new Set<string>();
      sevFilter.querySelectorAll<HTMLButtonElement>('.sev-chip').forEach(chip => {
        chip.addEventListener('click', () => {
          const sev = chip.dataset['sev'] ?? '';
          if (active.has(sev)) { active.delete(sev); chip.classList.remove('active'); }
          else { active.add(sev); chip.classList.add('active'); }
          let visible = 0;
          r9Tbody.querySelectorAll<HTMLTableRowElement>('.r9-main-row').forEach(main => {
            const show = active.size === 0 || active.has(main.dataset['sev'] ?? 'INFO');
            main.style.display = show ? '' : 'none';
            if (show) visible++;
            const detail = r9Tbody.querySelector<HTMLTableRowElement>(`.r9-detail-row[data-for="${main.dataset['idx']}"]`);
            if (detail) detail.style.display = show ? '' : 'none';
          });
          const badge = r9Table.closest('.card')?.querySelector('.count-badge');
          if (badge) badge.textContent = String(visible);
        });
      });
    }
  }

  // R2 column sorting (same logic as R9; R2 rows carry data-severity).
  const r2Table = root.querySelector<HTMLTableElement>('#r2-table');
  const r2Tbody = r2Table?.querySelector('tbody');
  if (r2Table && r2Tbody) {
    let r2Col = -1, r2Dir = 1;
    const txt2 = (row: HTMLTableRowElement, col: number) =>
      (row.children[col] as HTMLElement | undefined)?.textContent?.trim() ?? '';
    r2Table.querySelectorAll<HTMLTableCellElement>('th.r2-sortable').forEach(th => {
      th.addEventListener('click', e => {
        if ((e.target as HTMLElement).closest('.col-info')) return;
        const col = parseInt(th.dataset['col'] ?? '0', 10);
        const type = th.dataset['type'] ?? 'text';
        r2Dir = r2Col === col ? -r2Dir : 1;
        r2Col = col;
        const rows2 = Array.from(r2Tbody.querySelectorAll<HTMLTableRowElement>('tr'));
        rows2.sort((a, b) => {
          if (type === 'sev') {
            return ((SEVERITY_RANK[(a.dataset['severity'] ?? 'INFO') as Severity] ?? 9) - (SEVERITY_RANK[(b.dataset['severity'] ?? 'INFO') as Severity] ?? 9)) * r2Dir;
          }
          if (type === 'num') {
            const na = parseFloat(txt2(a, col).replace(/[^\d.-]/g, '')) || 0;
            const nb = parseFloat(txt2(b, col).replace(/[^\d.-]/g, '')) || 0;
            return (na - nb) * r2Dir;
          }
          return txt2(a, col).localeCompare(txt2(b, col), intlLocale()) * r2Dir;
        });
        for (const r of rows2) r2Tbody.appendChild(r);
        r2Table.querySelectorAll('.sort-ind').forEach(s => { s.textContent = ''; });
        const ind = th.querySelector('.sort-ind');
        if (ind) ind.textContent = r2Dir > 0 ? '▲' : '▼';
      });
    });
  }

  // R2 map buttons.
  if (result) {
    const noticeMap = new Map<string, Notice>(result.notices.map(n => [n.id, n]));
    root.querySelectorAll<HTMLButtonElement>('.map-pin-btn').forEach(btn => {
      btn.addEventListener('click', async e => {
        e.stopPropagation();
        const noticeId = btn.dataset['noticeId'] ?? '';
        const notice = noticeMap.get(noticeId);
        if (!notice) return;
        let opts = buildMapOptions(notice, result.name_index);
        // In large-feed mode, shape geometry is not serialized up front.
        // After fetching the geometry from the worker and writing it to name_index,
        // run buildMapOptions AGAIN: rule-specific markers
        // (GEO_006/007 jump segment + endpoint pins, SHP_009/010/020 pins) are DERIVED
        // from the polyline; injecting only opts.polyline left them empty
        // (VBB showed the map line but did not mark the error).
        if (result.name_index.map_data_deferred && (!opts.polyline || opts.polyline.length < 2)) {
          const shapeId = shapeIdForNotice(notice, result.name_index);
          if (shapeId) {
            const coords = await requestShapeCoords(shapeId);
            if (coords.length > 1) {
              result.name_index.shape_coords[shapeId] = coords; // session cache
              opts = buildMapOptions(notice, result.name_index);
              // Draw it anyway when the rule's own branch does not use geometry (generic paths).
              if (!opts.polyline || opts.polyline.length < 2) {
                opts.polyline = coords;
                opts.showArrows = true;
                const hasShapeLegend = (opts.legendItems ?? []).some(l => l.color === '#f59e0b');
                if (!hasShapeLegend) {
                  opts.legendItems = [{ color: '#f59e0b', label: t('fix.map.route_shape') }, ...(opts.legendItems ?? [])];
                }
              }
            }
          }
        }
        if (opts.pins.length === 0 && !opts.polyline && !(opts.extraPolylines?.length)) return;
        const shapeIdRules = new Set(['SHP_009','SHP_010','SHP_012','SHP_014','SHP_015','SHP_016','SHP_018','SHP_019','SHP_020','GEO_006','GEO_007']);
        const eid = notice.entity_id ?? '';
        let entityLabel: string;
        if (shapeIdRules.has(notice.rule_id)) {
          entityLabel = `${t('fix.map.shape')}: ${eid}`;
        } else if (TRIP_ID_RULES.has(notice.rule_id)) {
          const rId = result.name_index.trip_routes[eid] ?? '';
          const rName = rId ? (result.name_index.routes[rId] ?? '') : '';
          const headsign = result.name_index.trips[eid] ?? '';
          entityLabel = rName ? `${rName}${headsign ? ` — ${headsign}` : ''}` : (headsign || eid);
        } else if (ROUTE_ID_RULES.has(notice.rule_id)) {
          entityLabel = result.name_index.routes[eid] ?? eid;
        } else {
          entityLabel = result.name_index.stops[eid] ?? eid;
        }
        openMapModal(`${notice.rule_id} — ${entityLabel}`, opts);
      });
    });

    // Pattern buttons: route-level notice → modal with that route's trip patterns.
    root.querySelectorAll<HTMLButtonElement>('.pattern-btn').forEach(btn => {
      btn.addEventListener('click', e => {
        e.stopPropagation();
        const notice = noticeMap.get(btn.dataset['noticeId'] ?? '');
        if (!notice) return;
        const rid = patternRouteId(notice, result.name_index);
        if (!rid) return;
        const label = result.name_index.routes[rid] ?? rid;
        openPatternModal(rid, label, result.name_index);
      });
    });
  }

  // R2 severity + class (chip, multi-select) + rule + file (dropdown) filters.
  const sevChipBox = root.querySelector('#r2-sev-filter');
  const clsChipBox = root.querySelector('#r2-cls-filter');
  const ruleFilter = root.querySelector<HTMLSelectElement>('#rule-filter');
  const fileFilterEl = root.querySelector<HTMLSelectElement>('#file-filter');
  const table      = root.querySelector<HTMLTableElement>('#r2-table');
  if (!table) return;

  const allRuleOpts: [string, string][] = ruleFilter
    ? Array.from(ruleFilter.options).filter(o => o.value !== '').map(o => [o.value, o.text])
    : [];
  const allFileOpts: [string, string][] = fileFilterEl
    ? Array.from(fileFilterEl.options).filter(o => o.value !== '').map(o => [o.value, o.text])
    : [];

  // Severity/class: multi-select (empty = all). Class may already be active when arriving with an external filter.
  const sevActive = new Set<string>();
  const clsActive = new Set<string>();
  clsChipBox?.querySelectorAll<HTMLButtonElement>('.cls-chip.active')
    .forEach(c => clsActive.add(c.dataset['cls'] ?? ''));

  function applyFilters(): void {
    const rule = ruleFilter?.value ?? '';
    const file = fileFilterEl?.value ?? '';
    const rows = Array.from(table!.querySelectorAll<HTMLTableRowElement>('tbody tr'));
    // Same cross-filtering as the planner page: each list counted under the other filters.
    const facets = crossFilter(rows.map(row => ({
      sev: row.dataset['severity'] ?? '',
      cls: row.dataset['class'] ?? '',
      rule: row.dataset['rule'] ?? '',
      file: row.dataset['file'] ?? '',
    })), { sev: sevActive, cls: clsActive, rule, file });
    const total = rows.length;
    let visible = 0;
    rows.forEach((row, i) => {
      const show = facets.visible[i];
      row.style.display = show ? '' : 'none';
      if (show) visible++;
    });

    const anyFilter = sevActive.size > 0 || clsActive.size > 0 || rule || file;
    const counter = root.querySelector<HTMLSpanElement>('#filter-count');
    if (counter) counter.textContent = anyFilter ? t('fix.filter.count', { visible, total }) : '';

    const badge = root.querySelector<HTMLSpanElement>('#r2-card h2 .count-badge');
    if (badge) badge.textContent = anyFilter ? String(visible) : String(total);

    const capWarn = root.querySelector<HTMLDivElement>('#r2-cap-warning');
    if (capWarn) {
      const realTotal = rule && cappedTotals ? cappedTotals[rule] : undefined;
      if (realTotal != null) {
        capWarn.hidden = false;
        capWarn.textContent = `⚠ ${t('fix.cap_warning', { shown: visible, total: realTotal })}`;
      } else {
        capWarn.hidden = true;
      }
    }

    // Options with rows left, each with its count; a chosen option stays (with 0) so the user
    // sees why the table is empty instead of the choice silently falling back to "All".
    const fillSelect = (select: HTMLSelectElement | null, options: [string, string][], counts: Map<string, number>, current: string) => {
      if (!select) return;
      select.innerHTML = `<option value="">${t('fix.filter.all')}</option>` +
        options.filter(([v]) => (counts.get(v) ?? 0) > 0 || v === current)
          .map(([v, l]) => `<option value="${escHtml(v)}"${v === current ? ' selected' : ''}>${escHtml(l)} (${(counts.get(v) ?? 0).toLocaleString(intlLocale())})</option>`)
          .join('');
      select.value = current;
    };
    fillSelect(ruleFilter, allRuleOpts, facets.rules, rule);
    fillSelect(fileFilterEl, allFileOpts, facets.files, file);
  }

  sevChipBox?.querySelectorAll<HTMLButtonElement>('.sev-chip').forEach(chip => {
    chip.addEventListener('click', () => {
      const s = chip.dataset['sev'] ?? '';
      if (sevActive.has(s)) { sevActive.delete(s); chip.classList.remove('active'); }
      else { sevActive.add(s); chip.classList.add('active'); }
      applyFilters();
    });
  });
  clsChipBox?.querySelectorAll<HTMLButtonElement>('.cls-chip').forEach(chip => {
    chip.addEventListener('click', () => {
      const c = chip.dataset['cls'] ?? '';
      if (clsActive.has(c)) { clsActive.delete(c); chip.classList.remove('active'); }
      else { clsActive.add(c); chip.classList.add('active'); }
      applyFilters();
    });
  });
  ruleFilter?.addEventListener('change', applyFilters);
  fileFilterEl?.addEventListener('change', applyFilters);

  // If arriving with a filter from the files page (file filter) or a score-component card
  // (class filter), apply it immediately and scroll to R2.
  // Counts in the lists from the first paint.
  applyFilters();
  if ((fileFilterEl && fileFilterEl.value) || clsActive.size > 0) {
    requestAnimationFrame(() => {
      root.querySelector('#r2-card')?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    });
  }
}

// ── Kolon-bilgi (ℹ) tooltip ─────────────────────────────────────────────────
// Because native `title` works only on hover (with a delay, and not at all on mobile/click),
// use a custom tooltip opened by hover + click + keyboard.
let cinfoTipEl: HTMLElement | null = null;
let cinfoTarget: HTMLElement | null = null;
let cinfoPinned = false;
let cinfoGlobalsBound = false;

function cinfoEnsureEl(): HTMLElement {
  if (cinfoTipEl) return cinfoTipEl;
  const el = document.createElement('div');
  el.className = 'col-info-pop';
  el.setAttribute('role', 'tooltip');
  el.hidden = true;
  document.body.appendChild(el);
  cinfoTipEl = el;
  return el;
}

function cinfoShow(target: HTMLElement, pinned: boolean): void {
  const text = target.getAttribute('data-tip') ?? '';
  if (!text) return;
  const tip = cinfoEnsureEl();
  tip.textContent = text;
  tip.hidden = false;
  cinfoTarget = target;
  cinfoPinned = pinned;

  // Position below the icon; fit above/to the side if it would leave the viewport.
  const r = target.getBoundingClientRect();
  const m = 6;
  const tw = tip.offsetWidth;
  const th = tip.offsetHeight;
  let left = r.left + r.width / 2 - tw / 2;
  left = Math.max(m, Math.min(left, window.innerWidth - tw - m));
  let top = r.bottom + m;
  if (top + th > window.innerHeight - m) top = r.top - th - m;
  tip.style.left = `${Math.round(left)}px`;
  tip.style.top = `${Math.round(Math.max(m, top))}px`;
}

function cinfoHide(): void {
  if (cinfoTipEl) cinfoTipEl.hidden = true;
  cinfoTarget = null;
  cinfoPinned = false;
}

function cinfoWire(root: HTMLElement): void {
  // Accessible name (screen reader) — derive from data-tip.
  root.querySelectorAll<HTMLElement>('.col-info').forEach(el => {
    if (!el.getAttribute('aria-label')) {
      el.setAttribute('aria-label', el.getAttribute('data-tip') ?? '');
    }
  });

  // Hover: show/hide temporarily (when not pinned).
  root.addEventListener('mouseover', e => {
    const el = (e.target as HTMLElement).closest<HTMLElement>('.col-info');
    if (el && !cinfoPinned) cinfoShow(el, false);
  });
  root.addEventListener('mouseout', e => {
    const el = (e.target as HTMLElement).closest<HTMLElement>('.col-info');
    if (el && !cinfoPinned) cinfoHide();
  });

  // Click: open/close (pin).
  root.addEventListener('click', e => {
    const el = (e.target as HTMLElement).closest<HTMLElement>('.col-info');
    if (!el) return;
    e.stopPropagation();
    if (cinfoPinned && cinfoTarget === el) cinfoHide();
    else cinfoShow(el, true);
  });

  // Keyboard: open/close with Enter/Space.
  root.addEventListener('keydown', e => {
    const ke = e as KeyboardEvent;
    const el = (ke.target as HTMLElement).closest<HTMLElement>('.col-info');
    if (!el) return;
    if (ke.key === 'Enter' || ke.key === ' ') {
      ke.preventDefault();
      if (cinfoPinned && cinfoTarget === el) cinfoHide();
      else cinfoShow(el, true);
    }
  });

  // Global close — bind only once.
  if (!cinfoGlobalsBound) {
    cinfoGlobalsBound = true;
    document.addEventListener('click', ev => {
      if (!cinfoPinned) return;
      const tgt = ev.target as HTMLElement;
      if (tgt.closest('.col-info') || tgt.closest('.col-info-pop')) return;
      cinfoHide();
    });
    document.addEventListener('keydown', ev => {
      if ((ev as KeyboardEvent).key === 'Escape') cinfoHide();
    });
    window.addEventListener('scroll', () => cinfoHide(), true);
    window.addEventListener('resize', () => cinfoHide());
  }
}
