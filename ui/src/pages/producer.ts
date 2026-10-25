// Üretici özeti: motorun mevcut sonucunu (R1 kararı, R9 önceliği, NameIndex adları) tek
// ekranda toplar. Yeni bulgu, skor ya da kök neden hesaplamaz; yalnız gruplar ve sıralar.
import type { ValidationResult, Notice, NameIndex, Severity } from '../types';
import { t, tMsg, tRemediation, severityForLocale, getLocale } from '../i18n';
import { setPage } from '../state';
import { escHtml } from '../escape';
import { buildProducerModel, stopUsage, entityLabel, type RuleGroup } from './producer-helpers';
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

function copyText(g: RuleGroup, ni: NameIndex): string {
  const n = g.notices[0];
  return [
    `${g.ruleId} — ${t(`rule.${g.ruleId}`)}`,
    tMsg(n),
    where(n),
    entityLabel(n, ni) ?? '',
    tRemediation(n),
  ].filter(Boolean).join('\n');
}

function blockerCard(g: RuleGroup, i: number, total: number, ni: NameIndex): string {
  const n = g.notices[0];
  const usage = n.entity_type === 'Stop' && n.entity_id ? stopUsage(n.entity_id, ni) : [];
  const usageHtml = usage.length
    ? `<p class="producer-usage">${t('producer.used_by')}: ${usage.map(u => `<strong>${escHtml(u.route)}</strong> (${t('producer.trips', { n: u.trips })})`).join(', ')}</p>`
    : '';
  const label = entityLabel(n, ni);
  return `<article class="producer-card">
    <div class="producer-card-top">
      <span class="producer-kicker">${t('producer.blocker_n', { i: i + 1, total })}</span>
      <span class="producer-muted">${escHtml(countLabel(g))}</span>
    </div>
    <h3>${escHtml(t(`rule.${g.ruleId}`))}</h3>
    <p>${escHtml(tMsg(n))}</p>
    ${usageHtml}
    <div class="producer-todo"><strong>${t('producer.what_to_do')}</strong><span>${escHtml(tRemediation(n))}</span></div>
    <div class="producer-actions">
      <button type="button" class="btn btn-secondary producer-copy" data-rule="${escHtml(g.ruleId)}">${t('producer.copy')}</button>
      <button type="button" class="btn btn-ghost producer-open-fix">${t('producer.open_fix')}</button>
      <span class="producer-where">${escHtml([label, where(n)].filter(Boolean).join(' · '))}</span>
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

function coverageNotice(result: ValidationResult, complete: boolean): string {
  if (complete) return '';
  const reasons = result.partial?.root_structural_errors ?? [];
  const reasonList = reasons.length
    ? `<ul>${reasons.map(reason => `<li>${escHtml(reason)}</li>`).join('')}</ul>`
    : '';
  return `<div class="producer-coverage">${t('producer.coverage_incomplete')}${reasonList}</div>`;
}

export function renderProducer(root: HTMLElement, result: ValidationResult): void {
  const m = buildProducerModel(result);
  const ni = result.name_index;
  const verdict = m.publishable
    ? `<div class="producer-verdict ok"><h2>${t('producer.publishable')}</h2><p>${t('producer.improvements_count', { n: m.improvements.length })}</p></div>`
    : `<div class="producer-verdict bad"><h2>${t('producer.not_publishable', { n: m.blockers.length })}</h2><p>${t('producer.not_publishable_desc')}</p></div>`;
  const coverage = coverageNotice(result, m.coverageComplete);
  const blockers = m.blockers.length
    ? `<h3 class="producer-section">${t('producer.blockers_title')}</h3>${m.blockers.map((g, i) => blockerCard(g, i, m.blockers.length, ni)).join('')}`
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

  root.querySelectorAll<HTMLButtonElement>('.producer-copy').forEach(btn => {
    btn.addEventListener('click', () => {
      const g = m.blockers.find(b => b.ruleId === btn.dataset['rule']);
      if (!g) return;
      void navigator.clipboard?.writeText(copyText(g, ni)).then(() => { btn.textContent = t('producer.copied'); });
    });
  });
  root.querySelectorAll<HTMLButtonElement>('.producer-open-fix').forEach(btn => {
    btn.addEventListener('click', () => {
      setPage('fix');
      window.dispatchEvent(new CustomEvent('gtfs-navigate'));
    });
  });
}
