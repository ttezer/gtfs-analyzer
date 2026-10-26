// Üretici özeti: motorun mevcut sonucunu (R1 kararı, R9 önceliği, NameIndex adları) tek
// ekranda toplar. Yeni bulgu, skor ya da kök neden hesaplamaz; yalnız gruplar ve sıralar.
import type { ValidationResult, Notice, Severity } from '../types';
import { t, tMsg, tRemediation, severityForLocale, getLocale } from '../i18n';
import { setPage } from '../state';
import { escHtml } from '../escape';
import { buildProducerModel, stopUsage, type RuleGroup } from './producer-helpers';
import { renderHiddenRulesBar, attachHiddenRuleListeners } from '../hidden-rules';

function countLabel(g: RuleGroup): string {
  if (g.notices[0]?.entity_type === 'Feed') return t('producer.all_feed');
  return t('producer.count', { n: g.count });
}

function sevLabel(s: Severity): string {
  return severityForLocale(getLocale(), s);
}

function where(n: Notice): string {
  const parts = [n.file, n.line != null ? t('producer.line', { n: n.line }) : null, n.field].filter(Boolean);
  return parts.join(' · ');
}

function visibleValue(value: string | null): string {
  if (!value) return '';
  return value.replace(/ /g, '␠').replace(/\t/g, '⇥').replace(/\r?\n/g, '↵');
}

interface PlannerCardCopy {
  title: string;
  context?: string;
  what: string;
  action: string;
  observed?: string;
  usage?: string;
}

function agencyLabel(n: Notice, result: ValidationResult): string {
  const agency = result.feed_summary?.agencies?.find(a => a.agency_id === n.entity_id);
  return agency?.agency_name || n.entity_id || t('producer.card.unnamed_agency');
}

function tripContext(n: Notice, result: ValidationResult): { route?: string; headsign?: string; departure?: string } {
  const tripId = n.entity_id;
  const routeId = tripId ? result.name_index.trip_routes?.[tripId] : undefined;
  return {
    route: routeId ? result.name_index.routes?.[routeId] : undefined,
    headsign: tripId ? result.name_index.trips?.[tripId] : undefined,
    departure: tripId ? result.name_index.trip_first_dep?.[tripId] : undefined,
  };
}

function tripContextText(context: ReturnType<typeof tripContext>, includeHeadsign = true): string {
  const parts = [
    context.route ? t('producer.card.route_context', { route: context.route }) : '',
    includeHeadsign && context.headsign ? t('producer.card.headsign_context', { headsign: context.headsign }) : '',
    context.departure ? t('producer.card.departure_context', { departure: context.departure }) : '',
  ].filter(Boolean);
  return parts.join(t('producer.card.context_separator'));
}

function speedStopPair(n: Notice, result: ValidationResult): string {
  const stopA = n.details?.stop_a ? result.name_index.stops?.[n.details.stop_a] : undefined;
  const stopB = n.details?.stop_b ? result.name_index.stops?.[n.details.stop_b] : undefined;
  return stopA && stopB
    ? t('producer.card.stop_pair', { from: stopA, to: stopB })
    : t('producer.card.adjacent_stops');
}

function localizedNumber(value: string): string {
  const number = Number(value);
  return Number.isFinite(number)
    ? new Intl.NumberFormat(getLocale(), { maximumFractionDigits: 1 }).format(number)
    : value;
}

function stm012Observed(n: Notice): string | undefined {
  const details = n.details;
  const distance = details?.distance_km;
  const duration = details?.scheduled_gap_seconds;
  if (distance && duration === '0') {
    return t('producer.card.zero_duration_value', {
      distance: localizedNumber(distance),
    });
  }
  const speed = details?.speed_kmh;
  if (speed) {
    return t('producer.card.speed_value', { speed: localizedNumber(speed) });
  }
  return /^\s*\d+(?:\.\d+)?\s+km\/h\s*$/i.test(n.observed_value || '')
    ? n.observed_value || undefined
    : undefined;
}

function plannerCardCopy(g: RuleGroup, result: ValidationResult): PlannerCardCopy | null {
  const n = g.notices[0];
  switch (g.ruleId) {
    case 'STM_012': {
      const context = tripContext(n, result);
      return {
        title: t('producer.card.STM_012.title'),
        context: tripContextText(context),
        what: t('producer.card.STM_012.what', {
          stops: speedStopPair(n, result),
        }),
        action: t('producer.card.STM_012.action'),
        observed: stm012Observed(n),
      };
    }
    case 'STP_001': {
      const stop = n.entity_id || n.observed_value || '';
      return {
        title: t('producer.card.STP_001.title'),
        context: t('producer.card.STP_001.context', { stop }),
        what: t('producer.card.STP_001.what'),
        action: t('producer.card.STP_001.action'),
      };
    }
    case 'STP_012': {
      const stop = n.entity_id || n.observed_value || '';
      return {
        title: t('producer.card.STP_012.title'),
        context: t('producer.card.STP_012.context', { stop }),
        what: t('producer.card.STP_012.what'),
        action: t('producer.card.STP_012.action'),
      };
    }
    case 'STM_015': {
      const context = tripContext(n, result);
      return {
        title: t('producer.card.STM_015.title'),
        context: tripContextText(context),
        what: t('producer.card.STM_015.what'),
        action: t('producer.card.STM_015.action'),
      };
    }
    case 'STM_016': {
      const context = tripContext(n, result);
      return {
        title: t('producer.card.STM_016.title'),
        context: tripContextText(context),
        what: t('producer.card.STM_016.what'),
        action: t('producer.card.STM_016.action'),
      };
    }
    case 'ARC_031':
      return {
        title: t('producer.card.ARC_031.title'),
        what: t('producer.card.ARC_031.what'),
        action: t('producer.card.ARC_031.action'),
      };
    case 'ARC_004':
      return {
        title: t('producer.card.ARC_004.title'),
        what: t('producer.card.ARC_004.what'),
        action: t('producer.card.ARC_004.action'),
      };
    case 'ARC_008':
      return {
        title: t('producer.card.ARC_008.title'),
        what: t('producer.card.ARC_008.what'),
        action: t('producer.card.ARC_008.action'),
      };
    case 'ARC_012':
      return {
        title: t('producer.card.ARC_012.title'),
        what: t('producer.card.ARC_012.what', { file: n.file || t('producer.card.unknown_file') }),
        action: t('producer.card.ARC_012.action'),
      };
    case 'ARC_025':
      return {
        title: t('producer.card.ARC_025.title'),
        what: t('producer.card.ARC_025.what', {
          file: n.file || t('producer.card.unknown_file'),
          field: n.field || t('producer.card.unknown_field'),
        }),
        action: t('producer.card.ARC_025.action'),
      };
    case 'ARC_035':
      return {
        title: t('producer.card.ARC_035.title'),
        what: t('producer.card.ARC_035.what', { file: n.file || t('producer.card.unknown_file') }),
        action: t('producer.card.ARC_035.action'),
      };
    case 'AGN_003':
      return {
        title: t('producer.card.AGN_003.title'),
        what: t('producer.card.AGN_003.what', { agency: agencyLabel(n, result) }),
        action: t('producer.card.AGN_003.action'),
        observed: visibleValue(n.observed_value),
      };
    case 'STM_002': {
      const stop = n.details?.stop_id || n.entity_id || n.observed_value || '';
      const usage = stop
        ? stopUsage(stop, result.name_index)
          .map(u => t('producer.card.route_usage', { route: u.route, trips: u.trips }))
          .join(', ')
        : '';
      return {
        title: t('producer.card.STM_002.title'),
        context: t('producer.card.STM_002.context', { stop }),
        what: t('producer.card.STM_002.what'),
        action: t('producer.card.STM_002.action'),
        usage,
      };
    }
    case 'STM_047': {
      const routeLabels = [...new Set(g.notices
        .map(item => item.entity_id ? result.name_index.trip_routes?.[item.entity_id] : null)
        .filter((routeId): routeId is string => Boolean(routeId))
        .map(routeId => result.name_index.routes?.[routeId] || routeId))];
      const departures = [...new Set(g.notices
        .map(item => item.entity_id ? result.name_index.trip_first_dep?.[item.entity_id] : null)
        .filter((departure): departure is string => Boolean(departure)))];
      const contextParts = [
        routeLabels.length ? t('producer.card.route_context', { route: routeLabels.join(', ') }) : '',
        departures.length
          ? t('producer.card.departure_list_context', {
            departure: departures.slice(0, 3).join(', ') + (departures.length > 3 ? '…' : ''),
          })
          : '',
        t('producer.card.trip_count_context', { count: g.count }),
      ].filter(Boolean);
      return {
        title: t('producer.card.STM_047.title'),
        context: contextParts.join(t('producer.card.context_separator')),
        what: t('producer.card.STM_047.what'),
        action: t('producer.card.STM_047.action'),
      };
    }
    default:
      return null;
  }
}

function blockerCard(g: RuleGroup, result: ValidationResult): string {
  const n = g.notices[0];
  const planner = plannerCardCopy(g, result);
  const title = planner?.title || t(`rule.${g.ruleId}`);
  const what = planner?.what || tMsg(n);
  const action = planner?.action || tRemediation(n);
  const location = where(n);
  const usage = planner?.usage
    ? `<p class="producer-card-usage"><span class="producer-card-label">${t('producer.card.usage')}:</span> ${escHtml(planner.usage)}</p>`
    : '';
  const observed = planner?.observed
    ? `<p class="producer-card-observed"><span class="producer-card-label">${t('producer.card.observed')}:</span> <code>${escHtml(planner.observed)}</code></p>`
    : '';
  const technical = [
    `<span>${t('producer.card.rule')}:</span> <code>${escHtml(g.ruleId)}</code>`,
    location ? `<span>${t('producer.card.location')}:</span> <code>${escHtml(location)}</code>` : '',
  ].filter(Boolean).join(' · ');
  const severityClass = g.severity.toLowerCase();
  return `<article class="producer-card producer-card-${severityClass}" data-rule="${escHtml(g.ruleId)}">
    <div class="producer-card-rail" aria-hidden="true"></div>
    <div class="producer-card-content">
      <header class="producer-card-header">
        <div>
          <div class="producer-card-kicker">
            <span class="producer-card-severity"><span class="producer-card-severity-mark" aria-hidden="true">!</span>${escHtml(sevLabel(g.severity))}</span>
            <span class="producer-card-count">${escHtml(countLabel(g))}</span>
          </div>
          <h4 class="producer-card-title">${escHtml(title)}</h4>
        </div>
      </header>
      ${planner?.context ? `<p class="producer-card-context">${escHtml(planner.context)}</p>` : ''}
      <div class="producer-card-main">
        <div class="producer-card-evidence">
          <span class="producer-card-label">${t('producer.card.what')}</span>
          <p class="producer-card-message">${escHtml(what)}</p>
          ${observed || usage ? `<div class="producer-card-facts">${observed}${usage}</div>` : ''}
        </div>
        <div class="producer-card-action">
          <span class="producer-card-label">${t('producer.card.action')}</span>
          <p class="producer-card-remediation">${escHtml(action)}</p>
        </div>
      </div>
      <footer class="producer-card-footer">
        <details class="producer-card-details"><summary>${t('producer.card.technical')}</summary><p>${technical}</p></details>
        <div class="producer-actions">
          <button type="button" class="btn btn-primary producer-open-fix">${t('producer.open_fix')} <span aria-hidden="true">→</span></button>
        </div>
      </footer>
    </div>
  </article>`;
}

function improvementRow(g: RuleGroup, result: ValidationResult): string {
  const n = g.notices[0];
  const planner = plannerCardCopy(g, result);
  const title = planner?.title || t(`rule.${g.ruleId}`);
  const body = planner
    ? `${planner.context ? `<p class="producer-card-context">${escHtml(planner.context)}</p>` : ''}
        ${planner.observed ? `<p class="producer-card-observed"><span class="producer-card-label">${t('producer.card.observed')}:</span> <code>${escHtml(planner.observed)}</code></p>` : ''}
        <p><span class="producer-card-label">${t('producer.card.what')}</span> ${escHtml(planner.what)}</p>
        <p class="producer-muted"><span class="producer-card-label">${t('producer.card.action')}</span> ${escHtml(planner.action)}</p>`
    : `<p>${escHtml(tMsg(n))}</p><p class="producer-muted">${escHtml(tRemediation(n))}</p>`;
  return `<details class="producer-row">
    <summary><span class="producer-row-title">${escHtml(title)}</span>
      <span class="producer-muted">${escHtml(sevLabel(g.severity))} · ${escHtml(countLabel(g))}</span></summary>
    <div class="producer-row-body">${body}</div>
  </details>`;
}

function coverageNotice(complete: boolean): string {
  if (complete) return '';
  return `<div class="producer-coverage">${t('producer.coverage_incomplete')}</div>`;
}

export function renderProducer(root: HTMLElement, result: ValidationResult): void {
  const m = buildProducerModel(result);
  const verdict = m.publishable
    ? `<div class="producer-verdict ok"><h2>${t('producer.publishable')}</h2><p>${t('producer.improvements_count', { n: m.improvements.length })}</p></div>`
    : `<div class="producer-verdict bad"><h2>${t('producer.not_publishable', { n: m.blockers.length })}</h2><p>${t('producer.not_publishable_desc')}</p></div>`;
  const coverage = coverageNotice(m.coverageComplete);
  const blockers = m.blockers.length
    ? `<h3 class="producer-section">${t('producer.blockers_title')}</h3>${m.blockers.map(g => blockerCard(g, result)).join('')}`
    : '';
  const infos = m.infos.length
    ? `<details class="producer-row producer-infos"><summary><span class="producer-row-title">${t('producer.infos', { n: m.infos.length })}</span></summary>
        <div class="producer-row-body">${m.infos.map(g => improvementRow(g, result)).join('')}</div></details>`
    : '';

  root.innerHTML = `<section class="producer-page">
    ${renderHiddenRulesBar()}${verdict}${coverage}${blockers}
    <h3 class="producer-section">${t('producer.improvements_title')} <span class="producer-muted">${t('producer.improvements_hint')}</span></h3>
    <div class="producer-list">${m.improvements.map(g => improvementRow(g, result)).join('')}${infos}</div>
    <p class="producer-foot">${t('producer.footer')}</p>
  </section>`;

  attachHiddenRuleListeners(root);

  root.querySelectorAll<HTMLButtonElement>('.producer-open-fix').forEach(btn => {
    btn.addEventListener('click', () => {
      setPage('fix');
      window.dispatchEvent(new CustomEvent('gtfs-navigate'));
    });
  });
}
