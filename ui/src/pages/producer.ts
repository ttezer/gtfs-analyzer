// Üretici özeti: motorun mevcut sonucunu (R1 kararı, R9 önceliği, NameIndex adları) tek
// ekranda toplar. Yeni bulgu, skor ya da kök neden hesaplamaz; yalnız gruplar ve sıralar.
import type { ValidationResult, Notice, Severity } from '../types';
import { t, tMsg, tRemediation, severityForLocale, getLocale } from '../i18n';
import { setPage } from '../state';
import { escHtml } from '../escape';
import { buildProducerModel, type RuleGroup } from './producer-helpers';
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

function blockerCard(g: RuleGroup): string {
  const n = g.notices[0];
  const location = where(n);
  const remediation = tRemediation(n);
  return `<article class="producer-card" data-rule="${escHtml(g.ruleId)}">
    <div class="producer-card-top">
      <span class="producer-kicker">${escHtml(g.ruleId)}</span>
      <span class="producer-muted">${escHtml(sevLabel(g.severity))}</span>
    </div>
    <p class="producer-card-message">${escHtml(tMsg(n))}</p>
    ${location ? `<p class="producer-where">${escHtml(location)}</p>` : ''}
    ${remediation ? `<p class="producer-card-remediation">${escHtml(remediation)}</p>` : ''}
    <div class="producer-actions">
      <button type="button" class="btn btn-ghost producer-open-fix">${t('producer.open_fix')}</button>
    </div>
  </article>`;
}

function improvementRow(g: RuleGroup): string {
  const n = g.notices[0];
  return `<details class="producer-row">
    <summary><span class="producer-row-title">${escHtml(t(`rule.${g.ruleId}`))}</span>
      <span class="producer-muted">${escHtml(sevLabel(g.severity))} · ${escHtml(countLabel(g))}</span></summary>
    <div class="producer-row-body"><p>${escHtml(tMsg(n))}</p><p class="producer-muted">${escHtml(tRemediation(n))}</p></div>
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
    ? `<h3 class="producer-section">${t('producer.blockers_title')}</h3>${m.blockers.map(blockerCard).join('')}`
    : '';
  const infos = m.infos.length
    ? `<details class="producer-row producer-infos"><summary><span class="producer-row-title">${t('producer.infos', { n: m.infos.length })}</span></summary>
        <div class="producer-row-body">${m.infos.map(improvementRow).join('')}</div></details>`
    : '';

  root.innerHTML = `<section class="producer-page">
    ${renderHiddenRulesBar()}${verdict}${coverage}${blockers}
    <h3 class="producer-section">${t('producer.improvements_title')} <span class="producer-muted">${t('producer.improvements_hint')}</span></h3>
    <div class="producer-list">${m.improvements.map(improvementRow).join('')}${infos}</div>
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
