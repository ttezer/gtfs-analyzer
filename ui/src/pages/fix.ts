import type { ValidationResult, Notice, R9Item, NameIndex, Severity } from '../types';
import { SEVERITY_TR, SEVERITY_COLOR, RULE_CLASS_TR, t, tMsg, tRemediation } from '../i18n';
import { openMapModal, type MapPin, type MapOptions } from '../map-modal';
import { requestShapeCoords } from '../validator-client';
import { openPatternModal } from '../pattern-modal';
import { escHtml } from '../escape';
import { SEVERITY_ORDER, SEVERITY_RANK } from '../severity-order';

// EN/JA parite: route-scoped bulgularda entity_id = route_id ve mesaj şablonu {route_label}
// kullanır. name_index.routes[route_id] = route_short_name (yoksa long_name) → details'e enjekte
// edilir ki en/ja şablonları Türkçe ile aynı dostça hat kodunu göstersin (Türkçe runtime message
// zaten route_short_name içerir; bu yalnız en/ja şablon yolu için). Idempotent; route olmayan
// entity'lere dokunmaz.
export function augmentRouteLabels(notices: Notice[], nameIndex: NameIndex): void {
  for (const n of notices) {
    const eid = n.entity_id ?? '';
    if (!eid || !(eid in nameIndex.routes)) continue;
    n.details = { ...(n.details ?? {}), route_label: nameIndex.routes[eid] || eid };
  }
}

export function renderFix(root: HTMLElement, result: ValidationResult, fileFilter?: string, classFilter?: string): void {
  augmentRouteLabels(result.notices, result.name_index);
  const noticeMap = new Map<string, Notice>(result.notices.map(n => [n.id, n]));

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
      ${renderR9(result.reports.r9.items, noticeMap, normFactor, pubNormFactor, result.capped_totals)}
      ${renderR2(result, noticeMap, deltaMap, result.name_index, fileFilter, classFilter)}
    </section>`;
}

function renderR9(items: R9Item[], noticeMap: Map<string, Notice>, normFactor: number, pubNormFactor: number, cappedTotals: Record<string, number>): string {
  if (items.length === 0) {
    return `<div class="card"><h2>${t('fix.r9_title')}</h2><p class="empty">${t('fix.r9_empty')}</p></div>`;
  }

  // result.notices = display_notices (cap sonrası), kural başına kaç notice gösterildiğini hesapla
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
    // Toplam: sadece cap'e çarpan kurullarda gerçek toplam
    const totalHtml = realTotal != null
      ? `<span class="cap-total">${realTotal.toLocaleString('tr-TR')}</span>`
      : '<span class="muted-text">—</span>';
    // Hata: R2 tablosunda görünen sayı (cap sonrası)
    const shownCount = shownCounts[item.rule_id] ?? item.affected_instance_count;

    const mainRow = `
      <tr class="r9-main-row" data-idx="${i}" data-sev="${severity}">
        <td>
          <span class="r9-arrow">▶</span> <code>${escHtml(item.rule_id)}</code>
          ${notice ? `<span class="r9-rule-title">${escHtml(t('rule.' + notice.rule_id))}</span>` : ''}
        </td>
        <td style="color:${SEVERITY_COLOR[severity]}">${SEVERITY_TR[severity]}</td>
        <td>${badgeHtml}</td>
        <td data-val="${shownCount}">${shownCount.toLocaleString('tr-TR')}</td>
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


// Bir bulgu için TEK GTFS spec linki. Öncelik: GTFS-JP profil kuralları / _jp dosyaları → gtfs.jp;
// aksi halde notice.file → gtfs.org schedule reference çapası (dosya adındaki nokta silinir:
// stop_times.txt → #stop_timestxt). Dosyası olmayan feed-seviyesi bulgular → null (link yok).
// Çoklu-dosya kuralında bile notice.file "sorunun raporlandığı asıl dosya"dır → tek link yeter.
function specUrl(notice: Notice): string | null {
  const file = notice.file ?? '';
  if (notice.rule_id.startsWith('JPN_') || file.endsWith('_jp.txt')) {
    return 'https://www.gtfs.jp/testsite/fix/format-reference_style/developpers-guide/format-reference.html';
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

  // Dosya filtresi için benzersiz dosya adları
  const uniqueFiles = [...new Set(
    items
      .filter(item => noticeMap.has(item.notice_id))
      .map(item => noticeMap.get(item.notice_id)!.file)
      .filter((f): f is string => !!f)
  )].sort();
  const fileOptions = uniqueFiles
    .map(f => `<option value="${escHtml(f)}"${fileFilter && fileFilter === f ? ' selected' : ''}>${escHtml(f)}</option>`)
    .join('');

  // Önem ve sınıf: R9'daki gibi çoklu seçilebilir chip filtresi (sadece mevcut değerler)
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
    // Desenler: yalnızca desen görmenin işe yaradığı hat-yapısı/servis kurallarında
    // (PATTERN_RULES) ve entity gerçekten bir hat (route_id) ise göster.
    const patLabel = t('fix.pattern_btn');
    const patBtn   = patternRouteId(notice, nameIndex)
      ? `<button class="pattern-btn" data-notice-id="${escHtml(notice.id)}" title="${patLabel}" aria-label="${patLabel}">${PATTERN_ICON}</button>`
      : '';
    const specHref = specUrl(notice);
    const ruleCell = specHref
      ? `<a href="${escHtml(specHref)}" target="_blank" rel="noopener" class="spec-link" title="${t('fix.spec_link')}">${escHtml(item.display_label)}</a>`
      : escHtml(item.display_label);
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
  // name ve id feed kaynaklı → Leaflet bindPopup HTML olarak render ettiği için escape şart.
  const label = `<strong>${escHtml(name)}</strong>${suffix ? ` ${escHtml(suffix)}` : ''}<br><code>${escHtml(id)}</code>`;
  return { lat: coords[0], lon: coords[1], label, primary };
}

// Feed kaynaklı ad + id'den güvenli (escape'li) harita popup etiketi üretir.
// Harita etiketleri bindPopup/innerHTML ile HTML olarak render edildiğinden,
// feed değerleri tek bir yerde escape edilir (tekrar eden <strong>/<code> kalıbı).
function stopLabel(name: string, id: string): string {
  return `<strong>${escHtml(name)}</strong><br><code>${escHtml(id)}</code>`;
}

// Harita ikon sistemi: whitelist tabanlı.
// entity_id'nin stop_coords'ta bulunması yeterli DEĞİL — rule bazlı kontrol şart.
// Yanlış eşleşmeyi önlemek için sadece burada tanımlı kurallar harita ikonu gösterir.

// entity_id stop_id olan kural listesi (stop pin gösterilir)
const STOP_ID_RULES = new Set([
  'STP_003','STP_004','STP_005','STP_006','STP_007','STP_008','STP_009',
  'STP_010','STP_011','STP_012','STP_013','STP_014','STP_015','STP_016',
  'STP_017','STP_018','STP_019','STP_020','STP_021','STP_022','STP_023',
  'STP_024','STP_025','STP_026','STP_027','STP_028','STP_029','STP_030',
  // NOT: GEO_001/003/004/005 emekli, GEO_010/011 yalnız R4 görüntü etiketi
  // (k7_reporting::r4_display_label — notice.rule_id hep SHP_014/016 kalır), GEO_014
  // feed seviyesi (entity_id yok) → hiçbiri gerçek bir notice ile eşleşmiyordu, kaldırıldı.
  'GEO_002','GEO_009','GEO_012','GEO_015','GEO_016','GEO_019','GEO_022',
  'PTH_012','PTH_013','PTH_015','PTH_016','PTH_017','PTH_018','PTH_019',
  'LVL_006',
  'SHP_022','SHP_024',
  'STP_033','STP_036',
  'VAT_002','VAT_007',
]);

// entity_id trip_id olan kural listesi — generic sefer haritası
const TRIP_ID_RULES = new Set([
  // TRP grubu
  'TRP_001','TRP_002','TRP_003','TRP_004','TRP_005','TRP_006','TRP_007','TRP_008',
  'TRP_010','TRP_011','TRP_012','TRP_013','TRP_014','TRP_015','TRP_016',
  'TRP_017','TRP_018','TRP_019','TRP_020',
  // FRQ grubu
  'FRQ_001','FRQ_002','FRQ_003','FRQ_004','FRQ_005','FRQ_006','FRQ_007','FRQ_008','FRQ_009',
  // STM grubu — özel handler olmayan kurallar (STM_008 özel handler'a taşındı)
  'STM_001','STM_002','STM_003','STM_004','STM_005','STM_006','STM_007',
  'STM_009','STM_010','STM_011','STM_013','STM_015','STM_016','STM_018','STM_019',
  // STM_036 (satır sıralaması) coğrafi değil — durak konumları doğru, yalnız dosya satır
  // sırası bozuk; harita yanıltıcı (boş/normal görünür). Bilinçli olarak haritasız.
  'STM_021','STM_022','STM_024','STM_027','STM_028','STM_029','STM_030',
  'STM_031','STM_032','STM_034',
  // VAT grubu — sefer bazlı
  'VAT_003',
  // OPR grubu — sefer bazlı
  'OPR_006','OPR_017',
]);

// entity_id route_id olan kural listesi — generic hat haritası
const ROUTE_ID_RULES = new Set([
  // RTS grubu
  'RTS_001','RTS_002','RTS_003','RTS_004','RTS_005','RTS_006','RTS_007','RTS_008',
  'RTS_010','RTS_011','RTS_012','RTS_013','RTS_015','RTS_016','RTS_017',
  'RTS_018','RTS_019','RTS_020','RTS_021','RTS_022','RTS_023',
  // VAT grubu — hat bazlı
  'VAT_001',
  // OPR grubu — hat bazlı
  'OPR_001','OPR_002','OPR_003',
]);

// Sefer-deseni görünümü = hattın YAPISI (durak desenleri). Yalnızca bunun gerçekten
// işe yaradığı iki kural: shape'siz hatta yapıyı görmek (RTS_017) ve süre-aykırı
// seferin hangi desende olduğunu görmek (VAT_003). Sıklık (OPR_001/005 — modal hat-geneli
// ort. verir, kuralın hat+yön+takvim dilimiyle çelişir) ve benzerlik (VAT_001 — hat ÇİFTİ,
// tek-hat deseniyle alakasız) bilinçli olarak HARİÇ.
const PATTERN_RULES = new Set([
  'VAT_003', // sefer süresi aykırı — sefer hangi desende?
  'RTS_017', // shape'siz hat — shape yokken yapıyı göster
]);

// "Sefer deseni" ikonu — dallanan hat (varyant) şeması; haritadan (📍) ayırt edilir.
const PATTERN_ICON =
  '<svg width="15" height="15" viewBox="0 0 16 16" aria-hidden="true" style="vertical-align:middle">' +
  '<path d="M2.5 8h4l6-4M6.5 8l6 4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>' +
  '<circle cx="2.5" cy="8" r="1.7" fill="currentColor"/>' +
  '<circle cx="13" cy="4" r="1.7" fill="currentColor"/>' +
  '<circle cx="13" cy="12" r="1.7" fill="currentColor"/>' +
  '</svg>';

// Desen modalının açılacağı route_id'yi çöz: RTS_017 entity'si route_id'dir; VAT_003
// entity'si trip_id'dir → trip_routes ile hattına çevrilir. Uygun değilse null (buton çıkmaz).
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

  // Stop pin kuralları: entity_id stop_id olup koordinatları biliniyorsa
  if (STOP_ID_RULES.has(notice.rule_id)) {
    return !!(eid && eid in nameIndex.stop_coords);
  }
  // Feed seviyesi coğrafi kurallar — kanıt "tüm durakların dağılımı": GEO_013 (özet),
  // GEO_018 (hepsi 200m içinde), GEO_021 (koordinat paylaşımı). stop_coords doluysa göster.
  if (['GEO_013','GEO_018','GEO_021'].includes(notice.rule_id)) {
    return Object.keys(nameIndex.stop_coords).length > 0;
  }
  // GEO_017 / GEO_020: shape kuralları ama pin observed_value'daki "lat,lon"dan gelir
  // (deferred modda shape_coords boş olsa bile harita anlamlı).
  if (notice.rule_id === 'GEO_017' || notice.rule_id === 'GEO_020') {
    return /^-?\d+(\.\d+)?,-?\d+(\.\d+)?$/.test((notice.observed_value ?? '').trim());
  }
  // SHP_017: details'tan bad_stop veya ctx
  if (notice.rule_id === 'SHP_017') {
    const sid  = notice.details?.['bad_stop'] ?? '';
    const ctxB = notice.details?.['ctx_b'] ?? '';
    const ctxA = notice.details?.['ctx_a'] ?? '';
    return !!(sid && sid in nameIndex.stop_coords) || ctxB.length > 0 || ctxA.length > 0;
  }
  // STM_014: hız ihlali iki durak arası + trip shape
  if (notice.rule_id === 'STM_014') {
    const m = notice.observed_value?.match(/\(([^→ ]+)\s*→\s*([^)]+)\)/);
    if (m && (m[1] in nameIndex.stop_coords || m[2] in nameIndex.stop_coords)) return true;
    return !!(eid && eid in nameIndex.trip_shapes);
  }
  // STM_008 / STM_020: iki sorunlu durak (kalkış/varış) + trip shape
  if (notice.rule_id === 'STM_020' || notice.rule_id === 'STM_008') {
    const sa = notice.details?.['stop_a'] ?? '';
    const sb = notice.details?.['stop_b'] ?? '';
    if (sa && sa in nameIndex.stop_coords) return true;
    if (sb && sb in nameIndex.stop_coords) return true;
    return !!(eid && eid in nameIndex.trip_shapes);
  }
  // STM_025: kısa segment — iki durak pini + trip shape
  if (notice.rule_id === 'STM_025') {
    const sa = notice.details?.['stop_a'] ?? '';
    const sb = notice.details?.['stop_b'] ?? '';
    if (sa && sa in nameIndex.stop_coords) return true;
    if (sb && sb in nameIndex.stop_coords) return true;
    return !!(eid && eid in nameIndex.trip_shapes);
  }
  // OPR_015: tek shape uyarısı — shape_id details'ta
  if (notice.rule_id === 'OPR_015') {
    const shapeId = notice.details?.['shape_id'] ?? '';
    return !!(shapeId && shapeId in nameIndex.shape_coords);
  }
  // SHP_027: shape birden çok durak desenine atanmış — shape geometrisi veya
  // temsili trip duraklarından biri biliniyorsa harita gösterilebilir.
  if (notice.rule_id === 'SHP_027') {
    if (eid && eid in nameIndex.shape_coords) return true;
    const reps = (notice.details?.['pattern_trips'] ?? '').split(',').filter(Boolean);
    return reps.some(tid => tid in nameIndex.trip_stops);
  }
  // PTH_014: farklı istasyonlar arası pathway — iki istasyon koordinatı
  if (notice.rule_id === 'PTH_014') {
    const fs = notice.details?.['from_station'] ?? '';
    const ts = notice.details?.['to_station'] ?? '';
    return !!(fs && fs in nameIndex.stop_coords && ts && ts in nameIndex.stop_coords);
  }
  // OPR_007/OPR_008/STM_017: entity_id = trip_id.
  // Harita shape polyline'a ek olarak durak koordinatlarından da çizilebilir
  // (OPR_007 stops details, STM_017 trip_stops, OPR_008 bad_seg durakları).
  // Bu yüzden shapes.txt'i olmayan feed'lerde de durak verisi varsa ikon gösterilir.
  if (['OPR_007','OPR_008','STM_017'].includes(notice.rule_id)) {
    return !!(eid && (eid in nameIndex.trip_shapes || eid in nameIndex.trip_stops));
  }
  // Shape koordinatı gerektiren kurallar: entity_id = shape_id
  // Büyük feed modunda (map_data_deferred) shape_coords boştur; geometri harita
  // ikonuna tıklandığında on-demand çekilir → eid varsa ikon yine gösterilir.
  if (['SHP_009','SHP_010','SHP_012','SHP_014','SHP_015','SHP_016','SHP_018','SHP_019','SHP_020','GEO_006','GEO_007'].includes(notice.rule_id)) {
    return !!(eid && (eid in nameIndex.shape_coords || nameIndex.map_data_deferred));
  }
  // Trip shape gerektiren kurallar: entity_id = trip_id
  if (['STM_012','STM_026','STM_033'].includes(notice.rule_id)) {
    return !!(eid && eid in nameIndex.trip_shapes) || !!(eid && eid in nameIndex.trip_stops);
  }
  // STM_035: aynı durak ardışık iki kez — tekrar eden durak pini veya trip shape
  if (notice.rule_id === 'STM_035') {
    const stopId = notice.observed_value ?? '';
    return !!(stopId && stopId in nameIndex.stop_coords) || !!(eid && (eid in nameIndex.trip_shapes || eid in nameIndex.trip_stops));
  }
  // VAT_005: izole duraksız da gösterebiliriz — stop_coords boş değilse
  if (notice.rule_id === 'VAT_005') {
    return Object.keys(nameIndex.stop_coords).length > 0;
  }
  // Generic sefer haritası: entity_id trip_id — shape veya stop listesi biliniyorsa
  if (TRIP_ID_RULES.has(notice.rule_id)) {
    return !!(eid && (eid in nameIndex.trip_shapes || eid in nameIndex.trip_stops));
  }
  // Generic hat haritası: entity_id route_id — en az bir shape varsa
  if (ROUTE_ID_RULES.has(notice.rule_id)) {
    return !!(eid && (nameIndex.route_shapes[eid]?.length ?? 0) > 0);
  }
  return false;
}

// Büyük feed modunda haritaya on-demand çekilecek shape_id'yi notice'tan çıkarır.
// entity_id = shape_id olan kurallar + details.shape_id taşıyan SHP_017/SHP_022.
// Trip-bağlamlı kurallar (entity_id = trip_id): shape_id trip_shapes'ten çözülür.
// large_feed_mode'da trip_shapes boştur → '' döner (harita zaten boş, by-design).
const SHAPE_ENTITY_RULES = new Set([
  'SHP_009','SHP_010','SHP_012','SHP_014','SHP_015','SHP_016',
  'SHP_018','SHP_019','SHP_020','GEO_006','GEO_007',
]);
function shapeIdForNotice(notice: Notice, nameIndex: NameIndex): string {
  if (SHAPE_ENTITY_RULES.has(notice.rule_id)) return notice.entity_id ?? '';
  if (['SHP_017','SHP_022','GEO_009'].includes(notice.rule_id)) return notice.details?.['shape_id'] ?? '';
  // buildMapOptions bu kurallarda shape'i trip_shapes[trip_id] ile çiziyor (STM_014/008/020/025/
  // 012/026/033/035, OPR_007/008, STM_017, generic trip). Deferred modda on-demand için aynı yolu izle.
  const eid = notice.entity_id ?? '';
  if (eid && eid in nameIndex.trip_shapes) return nameIndex.trip_shapes[eid] ?? '';
  return '';
}

function buildMapOptions(notice: Notice, nameIndex: NameIndex): MapOptions {
  const entityId = notice.entity_id ?? '';

  // SHP_017: hatalı durak (kırmızı) + ±3 bağlam durağı (mavi) + shape polyline
  if (notice.rule_id === 'SHP_017') {
    const sid    = notice.details?.['bad_stop'] ?? '';
    const shapeId = notice.details?.['shape_id'] ?? '';
    const ctxB   = (notice.details?.['ctx_b'] ?? '').split(',').filter(Boolean);
    const ctxA   = (notice.details?.['ctx_a'] ?? '').split(',').filter(Boolean);
    const seqB   = (notice.details?.['seq_b'] ?? '').split(',').filter(Boolean);
    const seqA   = (notice.details?.['seq_a'] ?? '').split(',').filter(Boolean);
    const errSeq = notice.observed_value ?? '';

    const pins: MapPin[] = [];
    // Önceki 3 durak — mavi (birincil olmayan)
    for (let i = 0; i < ctxB.length; i++) {
      const seq = seqB[i] ? `#${seqB[i]} ` : '';
      const p = stopPin(ctxB[i], nameIndex, true, `${seq}${t('fix.map.pin.prev')}`);
      if (p) pins.push(p);
    }
    // Hatalı durak — kırmızı
    const seqLabel = errSeq ? `#${errSeq} ` : '';
    const errPin = sid ? stopPin(sid, nameIndex, false, `${seqLabel}${t('fix.map.pin.bad_seq')}`) : null;
    if (errPin) pins.push(errPin);
    // Sonraki 3 durak — mavi
    for (let i = 0; i < ctxA.length; i++) {
      const seq = seqA[i] ? `#${seqA[i]} ` : '';
      const p = stopPin(ctxA[i], nameIndex, true, `${seq}${t('fix.map.pin.next')}`);
      if (p) pins.push(p);
    }

    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    legendItems.push({ color: '#2563eb', label: t('fix.map.neighbor_stops') });
    legendItems.push({ color: '#dc2626', label: t('fix.map.bad_seq_stop') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: polyline.length > 1 };
  }

  // STM_014: iki pin + trip'in shape'i
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

  // STM_008 / STM_020: iki durak pini (kalkış/varış) + trip shape'i
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

  // SHP_010: tekrarlanan shape noktası — polyline + kırmızı pin
  if (notice.rule_id === 'SHP_010') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const m = notice.observed_value?.match(/^\(([^,]+),([^)]+)\)$/);
    const pins: MapPin[] = [];
    if (m) {
      const lat = parseFloat(m[1]);
      const lon = parseFloat(m[2]);
      if (!isNaN(lat) && !isNaN(lon)) {
        pins.push({ lat, lon, label: `${t('fix.map.pin.dup_coord')}<br><code>(${m[1]}, ${m[2]})</code>`, primary: false });
      }
    }
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) legendItems.push({ color: '#dc2626', label: t('fix.map.dup_point') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_022: durak güzergah şeklinde belirsiz — stop pin + shape polyline
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

  // SHP_018 / SHP_019: orphan shape — sadece polyline, durak pini yok
  if (notice.rule_id === 'SHP_018' || notice.rule_id === 'SHP_019') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const legendItems = polyline.length > 1 ? [{ color: '#f59e0b', label: t('fix.map.route_shape_unused') }] : [];
    return { pins: [], polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // OPR_007: tekrar eden durak — tüm duraklar + shape + yön okları
  if (notice.rule_id === 'OPR_007') {
    const dupStopId = notice.details?.['dup_stop'] ?? '';
    const stops = (notice.details?.['stops'] ?? '').split(',').filter(Boolean);
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];

    const seen = new Set<string>();
    const pins: MapPin[] = [];
    for (const id of stops) {
      if (seen.has(id)) continue;
      seen.add(id);
      const isDup = id === dupStopId;
      const p = stopPin(id, nameIndex, !isDup, isDup ? t('fix.map.pin.repeated') : undefined);
      if (p) { p.small = !isDup; pins.push(p); }
    }
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 1) legendItems.push({ color: '#2563eb', label: t('fix.map.stops') });
    if (dupStopId) legendItems.push({ color: '#dc2626', label: t('fix.map.repeated_stop') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // STM_017: sefer güzergahı (polyline) + tüm seferin durakları
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

  // GEO_006 / GEO_007: shape atlaması — shape gri, ATLAYAN SEGMENT kırmızı + iki uç pin.
  // Atlama zaten polyline'ın bir parçası olarak çiziliyordu; aynı renkte olduğu için hangi
  // parçanın hata olduğu görülmüyordu. Segment ayrı renkle üstüne çizilip ona zoom yapılır.
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
      if (ptA) pins.push({ lat: ptA[0], lon: ptA[1], label: `<strong>${t('fix.map.pin.jump_start')}</strong><br>Segment ${idxA}`, primary: true });
      if (ptB) pins.push({ lat: ptB[0], lon: ptB[1], label: `<strong>${t('fix.map.pin.jump_end')}</strong><br>Segment ${idxB}`, primary: false });
      if (ptA && ptB) {
        extraPolylines.push({ coords: [[ptA[0], ptA[1]], [ptB[0], ptB[1]]], color: '#dc2626', weight: 5, zoomTo: true });
      }
    }
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#9ca3af', label: t('fix.map.route_shape') });
    if (extraPolylines.length > 0) legendItems.push({ color: '#dc2626', label: t('fix.map.jump_segment') });
    if (pins.length > 0) {
      // Renkler pinlerle aynı olmalı: başlangıç primary(mavi), bitiş referans(kırmızı).
      legendItems.push({ color: '#2563eb', label: t('fix.map.jump_start') });
      legendItems.push({ color: '#dc2626', label: t('fix.map.jump_end') });
    }
    return {
      pins,
      polyline: polyline.length > 1 ? polyline : undefined,
      polylineColor: '#9ca3af',
      extraPolylines,
      legendItems,
      showArrows: false,
    };
  }

  // SHP_020: ardışık olmayan tekrarlayan nokta — polyline + kırmızı pin
  if (notice.rule_id === 'SHP_020') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    // observed_value: "nokta {i} ve {j}: ({lat},{lon})"
    const m = notice.observed_value?.match(/\(([^,]+),([^)]+)\)$/);
    const pins: MapPin[] = [];
    if (m) {
      const lat = parseFloat(m[1]);
      const lon = parseFloat(m[2]);
      if (!isNaN(lat) && !isNaN(lon)) {
        pins.push({ lat, lon, label: `${t('fix.map.pin.repeat_coord')}<br><code>(${m[1]}, ${m[2]})</code>`, primary: false });
      }
    }
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) legendItems.push({ color: '#dc2626', label: t('fix.map.repeat_point') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_009: güzergah şekli kendisiyle kesişiyor — polyline + kesişen segment uçları
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
      if (ptA) pins.push({ lat: ptA[0], lon: ptA[1], label: t('fix.map.pin.seg_start', { a: String(a), b: String(b) }), primary: true });
      if (ptB) pins.push({ lat: ptB[0], lon: ptB[1], label: t('fix.map.pin.seg_end',   { a: String(a), b: String(b) }), primary: true });
      if (ptC) pins.push({ lat: ptC[0], lon: ptC[1], label: t('fix.map.pin.seg_start', { a: String(c), b: String(d) }), primary: false });
      if (ptD) pins.push({ lat: ptD[0], lon: ptD[1], label: t('fix.map.pin.seg_end',   { a: String(c), b: String(d) }), primary: false });
    }
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) {
      legendItems.push({ color: '#2563eb', label: t('fix.map.segment_1') });
      legendItems.push({ color: '#dc2626', label: t('fix.map.segment_2') });
    }
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // OPR_008: tüm shape gri, tüm bozuk segmentler kırmızı
  if (notice.rule_id === 'OPR_008') {
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#9ca3af', label: t('fix.map.route_shape') });

    // Tüm bozuk segmentleri topla (bad_seg_0_a/b, bad_seg_1_a/b, ...)
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
        pins.push({ lat: coordA[0], lon: coordA[1], label: `<code>${escHtml(stopA)}</code>`, primary: true, small: false });
      }
      if (coordB && !seenStops.has(stopB)) {
        seenStops.add(stopB);
        pins.push({ lat: coordB[0], lon: coordB[1], label: `<code>${escHtml(stopB)}</code>`, primary: true, small: false });
      }
    }

    if (extraPolylines.length > 0) legendItems.push({ color: '#dc2626', label: t('fix.map.bad_segment') });
    legendItems.push({ color: '#2563eb', label: t('fix.map.segment_ends') });

    return {
      pins,
      polyline: polyline.length > 1 ? polyline.map(p => [p[0], p[1]] as [number, number]) : undefined,
      extraPolylines,
      legendItems,
      showArrows: false,
    };
  }

  // STM_025: kısa segment — shape (sarı) + tüm duraklar (küçük mavi) + 2 sorunlu durak (mavi/kırmızı)
  if (notice.rule_id === 'STM_025') {
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const stopIds = entityId ? (nameIndex.trip_stops[entityId] ?? []) : [];
    const stopA = notice.details?.['stop_a'] ?? '';
    const stopB = notice.details?.['stop_b'] ?? '';

    const pins: import('../map-modal').MapPin[] = [];
    for (const id of stopIds) {
      if (id === stopA || id === stopB) continue;
      const p = stopPin(id, nameIndex, true);
      if (p) pins.push({ ...p, small: true });
    }
    const pinA = stopA ? stopPin(stopA, nameIndex, true, t('fix.map.pin.depart')) : null;
    if (pinA) pins.push(pinA);
    const pinB = stopB ? stopPin(stopB, nameIndex, false, t('fix.map.pin.arrive_short')) : null;
    if (pinB) pins.push(pinB);

    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (stopIds.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    legendItems.push({ color: '#dc2626', label: t('fix.map.short_segment_end') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // OPR_015: tek shape uyarısı — sadece shape polyline
  if (notice.rule_id === 'OPR_015') {
    const shapeId = notice.details?.['shape_id'] ?? '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const legendItems = polyline.length > 1 ? [{ color: '#f59e0b', label: t('fix.map.route_shape_single') }] : [];
    return { pins: [], polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // ── Shape kuralları: entity_id = shape_id, durakları shape_trips üzerinden bul ──

  // Yardımcı: shape'e ait sefer duraklarını küçük mavi pinler olarak döndür
  function shapeStopPins(shapeId: string): MapPin[] {
    const tripId = nameIndex.shape_trips[shapeId] ?? '';
    const stopIds = tripId ? (nameIndex.trip_stops[tripId] ?? []) : [];
    return stopIds
      .map(id => stopPin(id, nameIndex, true))
      .filter((p): p is MapPin => p !== null)
      .map(p => ({ ...p, small: true }));
  }

  // SHP_012: shape, durak konumlarından uzak — shape + duraklar
  if (notice.rule_id === 'SHP_012') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const pins = shapeStopPins(entityId);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_014: ilk/son durak shape ucundan uzak — shape + duraklar + hatalı durak vurgulu
  if (notice.rule_id === 'SHP_014') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const problemStopId = notice.details?.['problem_stop'] ?? '';
    const tripId = notice.details?.['trip_id'] ?? '';
    const stopIds = tripId ? (nameIndex.trip_stops[tripId] ?? []) : [];
    const pins: MapPin[] = [];
    for (const id of stopIds) {
      if (id === problemStopId) continue;
      const p = stopPin(id, nameIndex, true);
      if (p) pins.push({ ...p, small: true });
    }
    const endpoint = notice.details?.['endpoint'] === 'start' ? t('fix.map.pin.first_far_start') : t('fix.map.pin.last_far_end');
    const errPin = problemStopId ? stopPin(problemStopId, nameIndex, false, endpoint) : null;
    if (errPin) pins.push(errPin);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (stopIds.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    if (errPin) legendItems.push({ color: '#dc2626', label: t('fix.map.far_endpoint_stop') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_015: shape nokta yoğunluğu yetersiz — shape + duraklar
  if (notice.rule_id === 'SHP_015') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const pins = shapeStopPins(entityId);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_016: shape ters yönde — shape + duraklar + ilk durak kırmızı (hata noktası)
  if (notice.rule_id === 'SHP_016') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const firstStopId = notice.details?.['first_stop'] ?? '';
    const tripId = notice.details?.['trip_id'] ?? '';
    const stopIds = tripId ? (nameIndex.trip_stops[tripId] ?? []) : [];
    const pins: MapPin[] = [];
    for (const id of stopIds) {
      if (id === firstStopId) continue;
      const p = stopPin(id, nameIndex, true);
      if (p) pins.push({ ...p, small: true });
    }
    const errPin = firstStopId ? stopPin(firstStopId, nameIndex, false, t('fix.map.pin.first_wrong')) : null;
    if (errPin) pins.push(errPin);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape_rev') });
    if (stopIds.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    if (errPin) legendItems.push({ color: '#dc2626', label: t('fix.map.first_stop_wrong_end') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // SHP_027: shape birden çok durak desenine (pattern) atanmış — shape (amber) +
  // her farklı desen ayrı renkte polyline. Temsili trip'ler details.pattern_trips'te.
  if (notice.rule_id === 'SHP_027') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const reps = (notice.details?.['pattern_trips'] ?? '').split(',').filter(Boolean);
    const palette = ['#2563eb','#dc2626','#16a34a','#9333ea','#ea580c','#0891b2','#ca8a04','#db2777','#4f46e5','#0d9488','#b91c1c','#7c3aed'];
    const extraPolylines: Array<{ coords: [number, number][]; color: string; weight?: number }> = [];
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    reps.forEach((tid, i) => {
      const stopIds = nameIndex.trip_stops[tid] ?? [];
      const coords: [number, number][] = [];
      for (const sid of stopIds) {
        const c = nameIndex.stop_coords[sid];
        if (c) coords.push([c[0], c[1]]);
      }
      if (coords.length > 1) {
        const color = palette[i % palette.length];
        extraPolylines.push({ coords, color, weight: 4 });
        legendItems.push({ color, label: `${t('fix.map.pattern')} ${i + 1}` });
      }
    });
    return {
      pins: [],
      polyline: polyline.length > 1 ? polyline : undefined,
      extraPolylines: extraPolylines.length ? extraPolylines : undefined,
      legendItems,
      showArrows: false,
    };
  }

  // ── Trip kuralları: entity_id = trip_id ───────────────────────────────────

  // STM_012: fiziksel olarak imkansız hız — shape + tüm duraklar + 2 sorunlu durak
  if (notice.rule_id === 'STM_012') {
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const stopIds = entityId ? (nameIndex.trip_stops[entityId] ?? []) : [];
    const stopA = notice.details?.['stop_a'] ?? '';
    const stopB = notice.details?.['stop_b'] ?? '';
    const pins: MapPin[] = [];
    for (const id of stopIds) {
      if (id === stopA || id === stopB) continue;
      const p = stopPin(id, nameIndex, true);
      if (p) pins.push({ ...p, small: true });
    }
    const pinA = stopA ? stopPin(stopA, nameIndex, true, t('fix.map.pin.depart_speed')) : null;
    const pinB = stopB ? stopPin(stopB, nameIndex, false, t('fix.map.pin.arrive_speed')) : null;
    if (pinA) pins.push(pinA);
    if (pinB) pins.push(pinB);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (stopIds.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    legendItems.push({ color: '#dc2626', label: t('fix.map.impossible_speed') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // STM_026: durak arası mesafe çok uzun — shape + tüm duraklar + 2 uzak durak
  if (notice.rule_id === 'STM_026') {
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const stopIds = entityId ? (nameIndex.trip_stops[entityId] ?? []) : [];
    const stopA = notice.details?.['stop_a'] ?? '';
    const stopB = notice.details?.['stop_b'] ?? '';
    const pins: MapPin[] = [];
    for (const id of stopIds) {
      if (id === stopA || id === stopB) continue;
      const p = stopPin(id, nameIndex, true);
      if (p) pins.push({ ...p, small: true });
    }
    const pinA = stopA ? stopPin(stopA, nameIndex, true, t('fix.map.pin.depart')) : null;
    const pinB = stopB ? stopPin(stopB, nameIndex, false, t('fix.map.pin.arrive_far')) : null;
    if (pinA) pins.push(pinA);
    if (pinB) pins.push(pinB);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (stopIds.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    legendItems.push({ color: '#dc2626', label: t('fix.map.far_segment_end') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: true };
  }

  // STM_033: tek duraklı sefer — shape + tek durak
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

  // STM_035: aynı durak ardışık iki kez — shape + tüm duraklar + tekrar eden durak kırmızı
  if (notice.rule_id === 'STM_035') {
    const shapeId = entityId ? (nameIndex.trip_shapes[entityId] ?? '') : '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const stopIds = entityId ? (nameIndex.trip_stops[entityId] ?? []) : [];
    const repeatStopId = notice.observed_value ?? '';
    const pins: MapPin[] = [];
    for (const id of stopIds) {
      if (id === repeatStopId) continue;
      const p = stopPin(id, nameIndex, true);
      if (p) pins.push({ ...p, small: true });
    }
    const errPin = repeatStopId ? stopPin(repeatStopId, nameIndex, false, t('fix.map.pin.revisit')) : null;
    if (errPin) pins.push(errPin);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (stopIds.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    if (errPin) legendItems.push({ color: '#dc2626', label: t('fix.map.revisit_stop') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: polyline.length > 1 };
  }

  // ── Pathway kuralları ────────────────────────────────────────────────────

  // PTH_014: farklı istasyonlar arası pathway — iki istasyon pini + kesik çizgi bağlantı
  if (notice.rule_id === 'PTH_014') {
    const fromId = notice.details?.['from_station'] ?? '';
    const toId   = notice.details?.['to_station']   ?? '';
    const fromCoord = nameIndex.stop_coords[fromId];
    const toCoord   = nameIndex.stop_coords[toId];
    // hasMapCoords her iki istasyon koordinatını da şart koşar; bu dal savunma amaçlı.
    if (!fromCoord || !toCoord) return { pins: [], legendItems: [], showArrows: false };
    const fromName = nameIndex.stops[fromId] ?? fromId;
    const toName   = nameIndex.stops[toId]   ?? toId;
    const pins: MapPin[] = [
      { lat: fromCoord[0], lon: fromCoord[1], label: t('fix.map.pin.station', { name: fromName }), primary: true },
      { lat: toCoord[0],   lon: toCoord[1],   label: t('fix.map.pin.station', { name: toName }),   primary: false },
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

  // ── Analytics / VAT kuralları ─────────────────────────────────────────────

  // VAT_005: izole durak kümesi — bağlı duraklar (mavi küçük) + izole duraklar (kırmızı)
  if (notice.rule_id === 'VAT_005') {
    const isolatedSet = new Set((notice.details?.['isolated_stops'] ?? '').split(',').filter(Boolean));
    // Sadece gerçekten stop_times'ta geçen durakları göster — stop_times'sız duraklar mavi
    // "bağlı" gibi görünür ki bu yanıltıcı olur.
    const stopsInTrips = new Set<string>();
    // İzole duraklar için durak→{hat seti, sefer sayısı} aggregate (client-side).
    // İzole küme ≤200 durak olduğundan, yalnızca izole duraklar için biriktir.
    const stopRoutes = new Map<string, Set<string>>(); // stop_id → route_id set
    const stopTrips  = new Map<string, number>();       // stop_id → distinct trip count
    for (const [tripId, stops] of Object.entries(nameIndex.trip_stops)) {
      const routeId = nameIndex.trip_routes[tripId];
      const seenInTrip = new Set<string>();
      for (const s of stops) {
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
    for (const [sid, coord] of Object.entries(nameIndex.stop_coords)) {
      const isIsolated = isolatedSet.has(sid);
      const inTrips = stopsInTrips.has(sid);
      if (!isIsolated && !inTrips) continue; // stop_times'sız durakları gizle
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
      pins.push({
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

  // VAT_007: terminus durağı — terminal stop (kırmızı) + hat şekilleri (farklı renkler)
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

  // VAT_002: aktarma merkezi tanımsız — durak (kırmızı) + geçen hatlar farklı renkte
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

  // VAT_001: muhtemel kopya hat — iki hattı farklı renkte çiz + her hattın kalkış/varış
  // durağını aynı hat renginde göster (güzergahlar üst üste binse bile uçlar ayırt edilir).
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

        // Hattın kalkış/varış durağını ilk shape'in seferinden bul.
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
            pins.push({ lat: depCoord[0], lon: depCoord[1], color, primary: true,
              label: `<strong>${escHtml(routeName)} — ${t('fix.map.pin.depart')}</strong><br>${stopLabel(depName, depId)}` });
          }
          if (arrCoord && arrId !== depId) {
            const arrName = nameIndex.stops[arrId] ?? arrId;
            pins.push({ lat: arrCoord[0], lon: arrCoord[1], color, primary: true, small: true,
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

  // GEO_017 / GEO_020: shape geometri hatası — observed_value'daki koordinata kırmızı pin.
  // GEO_020'de shape'in TÜM noktaları aynı yerde olduğu için polyline çizilse bile
  // görünmez (sıfır uzunluk); tek pin doğru gösterim. GEO_017'de shape varsa arkaya çizilir.
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
        pins.push({ lat: plat, lon: plon, label: `<strong>${label}</strong><br><code>${escHtml(entityId)}</code><br><code>${escHtml(m[1])}, ${escHtml(m[2])}</code>`, primary: false });
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

  // GEO_013 / GEO_018 / GEO_021: feed'deki tüm durakları küçük pinlerle göster.
  // Üçünün de kanıtı dağılımın kendisi: özet (013), hepsi 200m içinde (018),
  // koordinatların üst üste yığılması (021).
  if (['GEO_013','GEO_018','GEO_021'].includes(notice.rule_id)) {
    const pins: MapPin[] = Object.entries(nameIndex.stop_coords).map(([stopId, coord]) => ({
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

  // ── Generic sefer haritası: entity_id = trip_id ──────────────────────────
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

  // ── Generic hat haritası: entity_id = route_id ───────────────────────────
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
    if (parentPin) pins.push(parentPin);
    const legendItems = pins.length > 1
      ? [{ color: '#2563eb', label: t('fix.map.stop') }, { color: '#dc2626', label: t('fix.map.parent_station_label') }]
      : [];
    return { pins, legendItems };
  }

  // STP_016: aynı koordinatta iki durak
  if (notice.rule_id === 'STP_016') {
    const secondId = notice.observed_value?.match(/== '([^']+)'/)?.[1] ?? '';
    const secondName = secondId ? (nameIndex.stops[secondId] ?? secondId) : '';
    const label = secondName
      ? `<strong>${escHtml(stopName)}</strong> = <strong>${escHtml(secondName)}</strong><br><code>${escHtml(entityId)}</code> ve <code>${escHtml(secondId)}</code>`
      : stopLabel(stopName, entityId);
    return { pins: [{ lat, lon, label, primary: true }] };
  }

  // STP_017: iki yakın durak — ikincisi observed_value'dan
  if (notice.rule_id === 'STP_017') {
    const secondId = notice.observed_value?.match(/to '([^']+)'/)?.[1] ?? '';
    const pins: MapPin[] = [{ lat, lon, label: stopLabel(stopName, entityId), primary: true }];
    const pinB = secondId ? stopPin(secondId, nameIndex, false) : null;
    if (pinB) pins.push(pinB);
    const legendItems = pins.length > 1
      ? [{ color: '#2563eb', label: `${t('fix.map.stop')} 1` }, { color: '#dc2626', label: `${t('fix.map.stop')} 2` }]
      : [];
    return { pins, legendItems };
  }

  // GEO_002: uzak durak + feed medianı + aradaki mesafe çizgisi.
  // Tek pin anlamsızdı: kural "feed medianından X km" diyor ama referans nokta
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
      pins.push({ lat: medLat, lon: medLon, label: `<strong>${t('fix.map.feed_median')}</strong>`, primary: true });
      extraPolylines.push({ coords: [[lat, lon], [medLat, medLon]], color: '#dc2626', weight: 2, zoomTo: false });
      legendItems.push({ color: '#2563eb', label: t('fix.map.feed_median') });
    }
    return { pins, extraPolylines, legendItems };
  }

  // GEO_009: durak pin + shape polyline. shape_id details'tan gelir; eski raporlarla
  // uyum için observed_value regex'i fallback olarak durur.
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

  // Diğer durak kuralları — tek pin
  return { pins: [{ lat, lon, label: stopLabel(stopName, entityId), primary: true }] };
}

export function attachFixListeners(root: HTMLElement, result?: ValidationResult, cappedTotals?: Record<string, number>): void {
  // Kolon-bilgi (ℹ) tooltip'leri — hover + tıklama + klavye (native title yerine)
  cinfoWire(root);

  // R9 expandable rows — blocks[] bağlamı
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

  // R9 sütun sıralaması + ÖNEM filtresi
  const r9Table = root.querySelector<HTMLTableElement>('#r9-table');
  const r9Tbody = r9Table?.querySelector('tbody');
  if (r9Table && r9Tbody) {
    let sortCol = -1, sortDir = 1;
    const cellTxt = (row: HTMLTableRowElement, col: number) =>
      (row.children[col] as HTMLElement | undefined)?.textContent?.trim() ?? '';
    r9Table.querySelectorAll<HTMLTableCellElement>('th.r9-sortable').forEach(th => {
      th.addEventListener('click', e => {
        if ((e.target as HTMLElement).closest('.col-info')) return; // tooltip ikonu tıklaması
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
          return cellTxt(a, col).localeCompare(cellTxt(b, col), 'tr') * sortDir;
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

    // ÖNEM filtresi: chip'e tıkla → seçili önemleri göster (hiçbiri seçili değilse tümü), çoklu seçim.
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

  // R2 sütun sıralaması (R9 ile aynı mantık; R2 satırları data-severity taşır)
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
          return txt2(a, col).localeCompare(txt2(b, col), 'tr') * r2Dir;
        });
        for (const r of rows2) r2Tbody.appendChild(r);
        r2Table.querySelectorAll('.sort-ind').forEach(s => { s.textContent = ''; });
        const ind = th.querySelector('.sort-ind');
        if (ind) ind.textContent = r2Dir > 0 ? '▲' : '▼';
      });
    });
  }

  // R2 harita butonları
  if (result) {
    const noticeMap = new Map<string, Notice>(result.notices.map(n => [n.id, n]));
    root.querySelectorAll<HTMLButtonElement>('.map-pin-btn').forEach(btn => {
      btn.addEventListener('click', async e => {
        e.stopPropagation();
        const noticeId = btn.dataset['noticeId'] ?? '';
        const notice = noticeMap.get(noticeId);
        if (!notice) return;
        let opts = buildMapOptions(notice, result.name_index);
        // Büyük feed modunda shape geometrisi peşinen serialize edilmez. Geometriyi
        // worker'dan çekip name_index'e yazdıktan sonra buildMapOptions'ı YENİDEN
        // çalıştırıyoruz: kural-özel işaretler (GEO_006/007 atlama segmenti + uç pinleri,
        // SHP_009/010/020 pinleri) polyline'dan TÜRETİLİYOR; yalnızca opts.polyline'ı
        // enjekte etmek onları boş bırakıyordu (VBB'de harita çizgi gösterip hatayı
        // işaretlemiyordu).
        if (result.name_index.map_data_deferred && (!opts.polyline || opts.polyline.length < 2)) {
          const shapeId = shapeIdForNotice(notice, result.name_index);
          if (shapeId) {
            const coords = await requestShapeCoords(shapeId);
            if (coords.length > 1) {
              result.name_index.shape_coords[shapeId] = coords; // oturum içi cache
              opts = buildMapOptions(notice, result.name_index);
              // Kuralın kendi dalı geometriyi kullanmıyorsa (generic yollar) yine de çiz.
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
        const shapeIdRules = new Set(['SHP_009','SHP_010','SHP_012','SHP_014','SHP_015','SHP_016','SHP_018','SHP_019','SHP_020','SHP_027','GEO_006','GEO_007']);
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

    // Desen butonları: hat-seviyesi notice → o hattın sefer desenleri modalı
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

  // R2 önem + sınıf (chip, çoklu) + kural + dosya (dropdown) filtresi
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

  // Önem/sınıf: çoklu seçim (boş = tümü). Sınıf, dış filtreli gelişte önceden aktif olabilir.
  const sevActive = new Set<string>();
  const clsActive = new Set<string>();
  clsChipBox?.querySelectorAll<HTMLButtonElement>('.cls-chip.active')
    .forEach(c => clsActive.add(c.dataset['cls'] ?? ''));

  function applyFilters(): void {
    const rule = ruleFilter?.value ?? '';
    const file = fileFilterEl?.value ?? '';
    let visible = 0, total = 0;
    const ruleSet = new Set<string>();
    const fileSet = new Set<string>();

    table!.querySelectorAll<HTMLTableRowElement>('tbody tr').forEach(row => {
      total++;
      const rowSev  = row.dataset['severity'] ?? '';
      const rowCls  = row.dataset['class']    ?? '';
      const rowRule = row.dataset['rule']      ?? '';
      const rowFile = row.dataset['file']      ?? '';
      const sevMatch  = sevActive.size === 0 || sevActive.has(rowSev);
      const clsMatch  = clsActive.size === 0 || clsActive.has(rowCls);
      const ruleMatch = !rule || rowRule === rule;
      const fileMatch = !file || rowFile === file;
      const show = sevMatch && clsMatch && ruleMatch && fileMatch;
      row.style.display = show ? '' : 'none';
      if (show) visible++;
      // Kural/dosya dropdown'ları çapraz filtreleme korur: seçenekleri diğer
      // filtrelerin uygulandığı satır kümesinden türetilir.
      if (sevMatch && clsMatch && fileMatch) ruleSet.add(rowRule);
      if (sevMatch && clsMatch && ruleMatch) fileSet.add(rowFile);
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

    if (ruleFilter) {
      ruleFilter.innerHTML = `<option value="">${t('fix.filter.all')}</option>` +
        allRuleOpts.filter(([v]) => ruleSet.has(v))
                   .map(([v, l]) => `<option value="${v}"${v === rule ? ' selected' : ''}>${escHtml(l)}</option>`)
                   .join('');
      if (rule && !ruleSet.has(rule)) ruleFilter.value = '';
    }
    if (fileFilterEl) {
      fileFilterEl.innerHTML = `<option value="">${t('fix.filter.all')}</option>` +
        allFileOpts.filter(([v]) => fileSet.has(v))
                   .map(([v, l]) => `<option value="${v}"${v === file ? ' selected' : ''}>${escHtml(l)}</option>`)
                   .join('');
      if (file && !fileSet.has(file)) fileFilterEl.value = '';
    }
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

  // files sayfasından (dosya filtresi) veya skor bileşeni kartından (sınıf filtresi)
  // filtreli gelindiyse hemen uygula ve R2'ye scroll et
  if ((fileFilterEl && fileFilterEl.value) || clsActive.size > 0) {
    applyFilters();
    requestAnimationFrame(() => {
      root.querySelector('#r2-card')?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    });
  }
}

// ── Kolon-bilgi (ℹ) tooltip ─────────────────────────────────────────────────
// Native `title` yalnızca hover'da (gecikmeli, mobilde/tıklamada hiç) çalıştığı
// için hover + tıklama + klavye ile açılan özel bir tooltip kullanıyoruz.
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

  // İkonun altına konumla; viewport dışına taşarsa üste/yana sığdır.
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
  // Erişilebilir ad (screen reader) — data-tip'ten türet.
  root.querySelectorAll<HTMLElement>('.col-info').forEach(el => {
    if (!el.getAttribute('aria-label')) {
      el.setAttribute('aria-label', el.getAttribute('data-tip') ?? '');
    }
  });

  // Hover: geçici göster/gizle (sabitlenmemişse).
  root.addEventListener('mouseover', e => {
    const el = (e.target as HTMLElement).closest<HTMLElement>('.col-info');
    if (el && !cinfoPinned) cinfoShow(el, false);
  });
  root.addEventListener('mouseout', e => {
    const el = (e.target as HTMLElement).closest<HTMLElement>('.col-info');
    if (el && !cinfoPinned) cinfoHide();
  });

  // Tıklama: aç/kapat (sabitle).
  root.addEventListener('click', e => {
    const el = (e.target as HTMLElement).closest<HTMLElement>('.col-info');
    if (!el) return;
    e.stopPropagation();
    if (cinfoPinned && cinfoTarget === el) cinfoHide();
    else cinfoShow(el, true);
  });

  // Klavye: Enter/Space ile aç/kapat.
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

  // Global kapatma — yalnızca bir kez bağlanır.
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
