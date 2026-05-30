import type { ValidationResult, Notice, R9Item, NameIndex } from '../types';
import { SEVERITY_TR, SEVERITY_COLOR, RULE_CLASS_TR, t, tMsg, tRemediation } from '../i18n';
import { openMapModal, type MapPin, type MapOptions } from '../map-modal';

export function renderFix(root: HTMLElement, result: ValidationResult): void {
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
      ${renderR2(result, noticeMap, deltaMap, result.name_index)}
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
      <tr class="r9-main-row" data-idx="${i}">
        <td>
          <span class="r9-arrow">▶</span> <code>${escHtml(item.rule_id)}</code>
          ${notice ? `<span class="r9-rule-title">${escHtml(t('rule.' + notice.rule_id))}</span>` : ''}
        </td>
        <td style="color:${SEVERITY_COLOR[severity]}">${SEVERITY_TR[severity]}</td>
        <td>${badgeHtml}</td>
        <td>${shownCount.toLocaleString('tr-TR')}</td>
        <td>${totalHtml}</td>
        <td class="score">${item.priority_score.toFixed(1)}</td>
        <td class="score-delta-cell">${pubHtml}</td>
        <td class="score-delta-cell">${qualHtml}</td>
        <td>${item.realized_dependent_count}</td>
        <td>${item.fix_effort.toFixed(1)}</td>
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

  return `
    <div class="card">
      <h2>${t('fix.r9_title')} <span class="count-badge">${items.length}</span></h2>
      <p class="hint">${t('fix.r9_hint')}</p>
      <div class="table-scroll">
        <table class="data-table" id="r9-table">
          <thead><tr>
            <th>${t('fix.th.rule')}</th>
            <th>${t('fix.th.severity')}</th>
            <th>${t('fix.th.label')}</th>
            <th>${t('fix.th.count')} <span class="col-info" title="${t('fix.th.count.tip')}">ℹ</span></th>
            <th>${t('fix.th.total')} <span class="col-info" title="${t('fix.th.total.tip')}">ℹ</span></th>
            <th class="score">${t('fix.th.score')} <span class="col-info" title="${t('fix.th.score.tip')}">ℹ</span></th>
            <th class="score-delta-cell">${t('fix.th.pub')} <span class="col-info" title="${t('fix.th.pub.tip')}">ℹ</span></th>
            <th class="score-delta-cell">${t('fix.th.quality')} <span class="col-info" title="${t('fix.th.quality.tip')}">ℹ</span></th>
            <th>${t('fix.th.dependent')} <span class="col-info" title="${t('fix.th.dependent.tip')}">ℹ</span></th>
            <th>${t('fix.th.effort')} <span class="col-info" title="${t('fix.th.effort.tip')}">ℹ</span></th>
          </tr></thead>
          <tbody>${rowPairs}</tbody>
        </table>
      </div>
    </div>`;
}


function renderR2(result: ValidationResult, noticeMap: Map<string, Notice>, deltaMap: Map<string, { qualDelta: number; pubDelta: number; count: number }>, nameIndex: NameIndex): string {
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

  const all = t('fix.filter.all');
  const filterBar = `
    <div class="filter-bar">
      <label>${t('fix.filter.severity')}
        <select id="sev-filter">
          <option value="">${all}</option>
          <option value="CRITICAL">${SEVERITY_TR['CRITICAL']}</option>
          <option value="HIGH">${SEVERITY_TR['HIGH']}</option>
          <option value="MEDIUM">${SEVERITY_TR['MEDIUM']}</option>
          <option value="LOW">${SEVERITY_TR['LOW']}</option>
          <option value="INFO">${SEVERITY_TR['INFO']}</option>
        </select>
      </label>
      <label>${t('fix.filter.class')}
        <select id="cls-filter">
          <option value="">${all}</option>
          <option value="SPEC">${RULE_CLASS_TR['SPEC']}</option>
          <option value="INTEROP">${RULE_CLASS_TR['INTEROP']}</option>
          <option value="QUALITY">${RULE_CLASS_TR['QUALITY']}</option>
          <option value="ANALYTICS">${RULE_CLASS_TR['ANALYTICS']}</option>
        </select>
      </label>
      <label>${t('fix.filter.rule')}
        <select id="rule-filter">
          <option value="">${all}</option>
          ${ruleOptions}
        </select>
      </label>
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
    return `
      <tr data-severity="${notice.severity}" data-class="${notice.rule_class}" data-rule="${notice.rule_id}">
        <td>${escHtml(item.display_label)}</td>
        <td style="color:${SEVERITY_COLOR[notice.severity]}">${SEVERITY_TR[notice.severity]}</td>
        <td>${RULE_CLASS_TR[notice.rule_class]}</td>
        <td>${escHtml(tMsg(notice))}</td>
        <td>${notice.file ? escHtml(notice.file) : '—'}</td>
        <td>${notice.line ?? '—'}</td>
        <td>${notice.field ? escHtml(notice.field) : '—'}</td>
        <td class="score-delta-cell">${pubHtml}</td>
        <td class="score-delta-cell">${qualHtml}</td>
        <td class="map-btn-cell">${mapBtn}</td>
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
            <th>${t('fix.r2.th.rule')}</th><th>${t('fix.r2.th.severity')}</th><th>${t('fix.r2.th.class')}</th><th>${t('fix.r2.th.message')}</th>
            <th>${t('fix.r2.th.file')}</th><th>${t('fix.r2.th.row')}</th><th>${t('fix.r2.th.field')}</th>
            <th class="score-delta-cell">${t('fix.r2.th.pub')} <span class="col-info" title="${t('fix.r2.th.pub.tip')}">ℹ</span></th>
            <th class="score-delta-cell">${t('fix.r2.th.quality')} <span class="col-info" title="${t('fix.r2.th.quality.tip')}">ℹ</span></th>
            <th class="map-btn-cell"></th>
          </tr></thead>
          <tbody>${rows}</tbody>
        </table>
      </div>
    </div>`;
}

function escHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function stopPin(id: string, nameIndex: NameIndex, primary: boolean, suffix?: string): MapPin | null {
  const coords = nameIndex.stop_coords[id];
  if (!coords) return null;
  const name = nameIndex.stops[id] ?? id;
  const label = `<strong>${name}</strong>${suffix ? ` ${suffix}` : ''}<br><code>${id}</code>`;
  return { lat: coords[0], lon: coords[1], label, primary };
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
  'GEO_001','GEO_002','GEO_003','GEO_004','GEO_005',
  'GEO_009','GEO_010','GEO_011','GEO_012','GEO_014','GEO_015',
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
  'TRP_009','TRP_010','TRP_011','TRP_012','TRP_013','TRP_014','TRP_015','TRP_016',
  'TRP_017','TRP_018','TRP_019','TRP_020',
  // FRQ grubu
  'FRQ_001','FRQ_002','FRQ_003','FRQ_004','FRQ_005','FRQ_006','FRQ_007','FRQ_008','FRQ_009',
  // STM grubu — özel handler olmayan kurallar
  'STM_001','STM_002','STM_003','STM_004','STM_005','STM_006','STM_007','STM_008',
  'STM_009','STM_010','STM_011','STM_013','STM_015','STM_016','STM_018','STM_019',
  'STM_021','STM_022','STM_023','STM_024','STM_027','STM_028','STM_029','STM_030',
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
  'RTS_009','RTS_010','RTS_011','RTS_012','RTS_013','RTS_015','RTS_016','RTS_017',
  'RTS_018','RTS_019','RTS_020','RTS_021','RTS_022','RTS_023',
  // VAT grubu — hat bazlı
  'VAT_001','VAT_004',
  // OPR grubu — hat bazlı
  'OPR_001','OPR_002','OPR_003',
]);

function hasMapCoords(notice: Notice, nameIndex: NameIndex): boolean {
  const eid = notice.entity_id ?? '';

  // Stop pin kuralları: entity_id stop_id olup koordinatları biliniyorsa
  if (STOP_ID_RULES.has(notice.rule_id)) {
    return !!(eid && eid in nameIndex.stop_coords);
  }
  // GEO_013: feed seviyesi — stop_coords tablosu boş değilse ikon göster
  if (notice.rule_id === 'GEO_013') {
    return Object.keys(nameIndex.stop_coords).length > 0;
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
  // STM_020: tam dakika filtresi iki durak + trip shape
  if (notice.rule_id === 'STM_020') {
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
  // PTH_014: farklı istasyonlar arası pathway — iki istasyon koordinatı
  if (notice.rule_id === 'PTH_014') {
    const fs = notice.details?.['from_station'] ?? '';
    const ts = notice.details?.['to_station'] ?? '';
    return !!(fs && fs in nameIndex.stop_coords && ts && ts in nameIndex.stop_coords);
  }
  // Trip shape gerektiren kurallar: entity_id = trip_id
  if (['OPR_007','OPR_008','STM_017'].includes(notice.rule_id)) {
    return !!(eid && eid in nameIndex.trip_shapes);
  }
  // Shape koordinatı gerektiren kurallar: entity_id = shape_id
  if (['SHP_007','SHP_009','SHP_010','SHP_012','SHP_014','SHP_015','SHP_016','SHP_018','SHP_019','SHP_020','GEO_006','GEO_007'].includes(notice.rule_id)) {
    return !!(eid && eid in nameIndex.shape_coords);
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

  // STM_020: iki durak pini + trip shape'i
  if (notice.rule_id === 'STM_020') {
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

  // GEO_006: shape atlama — tam polyline + atlama noktaları kırmızı pin
  if (notice.rule_id === 'GEO_006') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const m = notice.observed_value?.match(/segment\s+(\d+)→(\d+)/);
    const pins: MapPin[] = [];
    if (m && polyline.length > 0) {
      const idxA = parseInt(m[1], 10);
      const idxB = parseInt(m[2], 10);
      const ptA = polyline[idxA];
      const ptB = polyline[idxB];
      if (ptA) pins.push({ lat: ptA[0], lon: ptA[1], label: `<strong>${t('fix.map.pin.jump_start')}</strong><br>Segment ${idxA}`, primary: true });
      if (ptB) pins.push({ lat: ptB[0], lon: ptB[1], label: `<strong>${t('fix.map.pin.jump_end')}</strong><br>Segment ${idxB}`, primary: false });
    }
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) {
      legendItems.push({ color: '#dc2626', label: t('fix.map.jump_start') });
      legendItems.push({ color: '#2563eb', label: t('fix.map.jump_end') });
    }
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: false };
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
        pins.push({ lat: coordA[0], lon: coordA[1], label: `<code>${stopA}</code>`, primary: true, small: false });
      }
      if (coordB && !seenStops.has(stopB)) {
        seenStops.add(stopB);
        pins.push({ lat: coordB[0], lon: coordB[1], label: `<code>${stopB}</code>`, primary: true, small: false });
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

  // SHP_007: az noktalı shape (1-2 nokta) — shape + duraklar
  if (notice.rule_id === 'SHP_007') {
    const polyline = entityId ? (nameIndex.shape_coords[entityId] ?? []) : [];
    const pins = shapeStopPins(entityId);
    const legendItems: Array<{ color: string; label: string }> = [];
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    if (pins.length > 0) legendItems.push({ color: '#2563eb', label: t('fix.map.trip_stops') });
    return { pins, polyline: polyline.length > 1 ? polyline : undefined, legendItems, showArrows: polyline.length > 1 };
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
    if (!fromCoord || !toCoord) return defaultMapOptions(notice, nameIndex);
    const fromName = nameIndex.stops[fromId] ?? fromId;
    const toName   = nameIndex.stops[toId]   ?? toId;
    const center: [number, number] = [
      (fromCoord[0] + toCoord[0]) / 2,
      (fromCoord[1] + toCoord[1]) / 2,
    ];
    return {
      center,
      zoom: 15,
      markers: [
        { lat: fromCoord[0], lon: fromCoord[1], color: '#ef4444', label: fromName, title: t('fix.map.pin.station', { name: fromName }) },
        { lat: toCoord[0],   lon: toCoord[1],   color: '#3b82f6', label: toName,   title: t('fix.map.pin.station', { name: toName }) },
      ],
      polylines: [{
        coords: [[fromCoord[0], fromCoord[1]], [toCoord[0], toCoord[1]]],
        color: '#ef4444',
        dashArray: '6,4',
        weight: 2,
      }],
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
    for (const stops of Object.values(nameIndex.trip_stops)) {
      for (const s of stops) stopsInTrips.add(s);
    }
    const pins: MapPin[] = [];
    for (const [sid, coord] of Object.entries(nameIndex.stop_coords)) {
      const isIsolated = isolatedSet.has(sid);
      const inTrips = stopsInTrips.has(sid);
      if (!isIsolated && !inTrips) continue; // stop_times'sız durakları gizle
      const name = nameIndex.stops[sid] ?? sid;
      pins.push({
        lat: coord[0],
        lon: coord[1],
        label: isIsolated
          ? `<strong>${t('fix.map.pin.isolated')}</strong><br><strong>${name}</strong><br><code>${sid}</code>`
          : `<strong>${name}</strong><br><code>${sid}</code>`,
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
      ? [{ lat: coords[0], lon: coords[1], label: `<strong>${t('fix.map.pin.terminal', { name: stopName })}</strong><br><code>${entityId}</code>`, primary: false }]
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
        legendItems.push({ color, label: routeName });
        colorIdx++;
      }
    }
    return { pins, extraPolylines, legendItems, showArrows: false };
  }

  // GEO_013: feed'deki tüm durakları küçük pinlerle göster
  if (notice.rule_id === 'GEO_013') {
    const pins: MapPin[] = Object.entries(nameIndex.stop_coords).map(([stopId, coord]) => ({
      lat: coord[0],
      lon: coord[1],
      label: nameIndex.stops[stopId]
        ? `<strong>${nameIndex.stops[stopId]}</strong><br><code>${stopId}</code>`
        : `<code>${stopId}</code>`,
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
    if (polyline.length > 1) legendItems.push({ color: '#f59e0b', label: routeName ? t('fix.map.route_shape_named', { name: routeName }) : t('fix.map.route_shape') });
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
      legendItems: [{ color: '#f59e0b', label: t('fix.map.route_shapes_named', { name: routeName }) }],
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
      { lat, lon, label: `<strong>${stopName}</strong><br><code>${entityId}</code>`, primary: true },
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
      ? `<strong>${stopName}</strong> = <strong>${secondName}</strong><br><code>${entityId}</code> ve <code>${secondId}</code>`
      : `<strong>${stopName}</strong><br><code>${entityId}</code>`;
    return { pins: [{ lat, lon, label, primary: true }] };
  }

  // STP_017: iki yakın durak — ikincisi observed_value'dan
  if (notice.rule_id === 'STP_017') {
    const secondId = notice.observed_value?.match(/to '([^']+)'/)?.[1] ?? '';
    const pins: MapPin[] = [{ lat, lon, label: `<strong>${stopName}</strong><br><code>${entityId}</code>`, primary: true }];
    const pinB = secondId ? stopPin(secondId, nameIndex, false) : null;
    if (pinB) pins.push(pinB);
    const legendItems = pins.length > 1
      ? [{ color: '#2563eb', label: `${t('fix.map.stop')} 1` }, { color: '#dc2626', label: `${t('fix.map.stop')} 2` }]
      : [];
    return { pins, legendItems };
  }

  // GEO_009: durak pin + shape polyline
  if (notice.rule_id === 'GEO_009') {
    const shapeId = notice.observed_value?.match(/\(shape '([^']+)'\)/)?.[1] ?? '';
    const polyline = shapeId ? (nameIndex.shape_coords[shapeId] ?? []) : [];
    const legendItems: Array<{ color: string; label: string }> = [
      { color: '#2563eb', label: t('fix.map.stop') },
    ];
    if (polyline.length > 0) legendItems.push({ color: '#f59e0b', label: t('fix.map.route_shape') });
    return {
      pins: [{ lat, lon, label: `<strong>${stopName}</strong><br><code>${entityId}</code>`, primary: true }],
      polyline: polyline.length > 1 ? polyline : undefined,
      legendItems,
    };
  }

  // Diğer durak kuralları — tek pin
  return { pins: [{ lat, lon, label: `<strong>${stopName}</strong><br><code>${entityId}</code>`, primary: true }] };
}

export function attachFixListeners(root: HTMLElement, result?: ValidationResult, cappedTotals?: Record<string, number>): void {
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

  // R2 harita butonları
  if (result) {
    const noticeMap = new Map<string, Notice>(result.notices.map(n => [n.id, n]));
    root.querySelectorAll<HTMLButtonElement>('.map-pin-btn').forEach(btn => {
      btn.addEventListener('click', e => {
        e.stopPropagation();
        const noticeId = btn.dataset['noticeId'] ?? '';
        const notice = noticeMap.get(noticeId);
        if (!notice) return;
        const opts = buildMapOptions(notice, result.name_index);
        if (opts.pins.length === 0 && !opts.polyline && !(opts.extraPolylines?.length)) return;
        const shapeIdRules = new Set(['SHP_007','SHP_009','SHP_010','SHP_012','SHP_014','SHP_015','SHP_016','SHP_018','SHP_019','SHP_020','GEO_006','GEO_007']);
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
  }

  // R2 severity + class + rule filter
  const sevFilter  = root.querySelector<HTMLSelectElement>('#sev-filter');
  const clsFilter  = root.querySelector<HTMLSelectElement>('#cls-filter');
  const ruleFilter = root.querySelector<HTMLSelectElement>('#rule-filter');
  const table      = root.querySelector<HTMLTableElement>('#r2-table');
  if (!sevFilter || !clsFilter || !table) return;

  const allRuleOpts: [string, string][] = ruleFilter
    ? Array.from(ruleFilter.options).filter(o => o.value !== '').map(o => [o.value, o.text])
    : [];

  const SEV_OPTS: [string, string][] = [
    ['CRITICAL', SEVERITY_TR['CRITICAL']], ['HIGH', SEVERITY_TR['HIGH']], ['MEDIUM', SEVERITY_TR['MEDIUM']],
    ['LOW', SEVERITY_TR['LOW']], ['INFO', SEVERITY_TR['INFO']],
  ];
  const CLS_OPTS: [string, string][] = [
    ['SPEC', RULE_CLASS_TR['SPEC']], ['INTEROP', RULE_CLASS_TR['INTEROP']],
    ['QUALITY', RULE_CLASS_TR['QUALITY']], ['ANALYTICS', RULE_CLASS_TR['ANALYTICS']],
  ];

  function rebuildOpts(sel: HTMLSelectElement, cur: string, available: Set<string>, opts: [string, string][]): void {
    sel.innerHTML = `<option value="">${t('fix.filter.all')}</option>` +
      opts.filter(([v]) => available.has(v))
          .map(([v, l]) => `<option value="${v}"${v === cur ? ' selected' : ''}>${l}</option>`)
          .join('');
    if (cur && !available.has(cur)) sel.value = '';
  }

  function applyFilters(): void {
    const sev  = sevFilter!.value;
    const cls  = clsFilter!.value;
    const rule = ruleFilter?.value ?? '';
    let visible = 0, total = 0;
    const sevSet  = new Set<string>();
    const clsSet  = new Set<string>();
    const ruleSet = new Set<string>();

    table!.querySelectorAll<HTMLTableRowElement>('tbody tr').forEach(row => {
      total++;
      const rowSev  = row.dataset['severity'] ?? '';
      const rowCls  = row.dataset['class']    ?? '';
      const rowRule = row.dataset['rule']      ?? '';
      const sevMatch  = !sev  || rowSev  === sev;
      const clsMatch  = !cls  || rowCls  === cls;
      const ruleMatch = !rule || rowRule === rule;
      row.style.display = (sevMatch && clsMatch && ruleMatch) ? '' : 'none';
      if (sevMatch && clsMatch && ruleMatch) visible++;
      if (clsMatch && ruleMatch) sevSet.add(rowSev);
      if (sevMatch && ruleMatch) clsSet.add(rowCls);
      if (sevMatch && clsMatch) ruleSet.add(rowRule);
    });

    const anyFilter = sev || cls || rule;
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

    rebuildOpts(sevFilter!, sev, sevSet, SEV_OPTS);
    rebuildOpts(clsFilter!, cls, clsSet, CLS_OPTS);
    if (ruleFilter) {
      ruleFilter.innerHTML = `<option value="">${t('fix.filter.all')}</option>` +
        allRuleOpts.filter(([v]) => ruleSet.has(v))
                   .map(([v, l]) => `<option value="${v}"${v === rule ? ' selected' : ''}>${escHtml(l)}</option>`)
                   .join('');
      if (rule && !ruleSet.has(rule)) ruleFilter.value = '';
    }
  }

  sevFilter.addEventListener('change', applyFilters);
  clsFilter.addEventListener('change', applyFilters);
  ruleFilter?.addEventListener('change', applyFilters);
}
