import type { ValidationResult, Notice, RuleClass, Severity } from '../types';
import { SEVERITY_TR, SEVERITY_COLOR, RULE_CLASS_TR, t } from '../i18n';

const SEV_ORDER: Record<string, number> = {
  CRITICAL: 0, HIGH: 1, MEDIUM: 2, LOW: 3, INFO: 4,
};

interface RuleGroup {
  rule_id: string;
  max_severity: Severity;
  rule_class: RuleClass;
  notices: Notice[];
}

export function renderRules(root: HTMLElement, result: ValidationResult): void {
  const groups = buildGroups(result.notices);

  const total = result.notices.length;
  root.innerHTML = `
    <div class="rules-page">
      <div class="rules-summary-bar">
        <span class="rules-summary-text">${t('rules.summary', { total, rules: groups.length })}</span>
      </div>
      <div class="rules-accordion">
        ${groups.map(g => renderRuleGroup(g)).join('')}
      </div>
    </div>`;

  attachListeners(root);
}

function buildGroups(notices: Notice[]): RuleGroup[] {
  const map = new Map<string, RuleGroup>();

  for (const n of notices) {
    if (!map.has(n.rule_id)) {
      map.set(n.rule_id, {
        rule_id: n.rule_id,
        max_severity: n.severity,
        rule_class: n.rule_class,
        notices: [],
      });
    }
    const g = map.get(n.rule_id)!;
    g.notices.push(n);
    if ((SEV_ORDER[n.severity] ?? 9) < (SEV_ORDER[g.max_severity] ?? 9)) {
      g.max_severity = n.severity;
    }
  }

  return Array.from(map.values()).sort((a, b) => {
    const sd = (SEV_ORDER[a.max_severity] ?? 9) - (SEV_ORDER[b.max_severity] ?? 9);
    return sd !== 0 ? sd : b.notices.length - a.notices.length;
  });
}

function renderRuleGroup(g: RuleGroup): string {
  const sevColor   = SEVERITY_COLOR[g.max_severity];
  const sevLabel   = SEVERITY_TR[g.max_severity];
  const classLabel = RULE_CLASS_TR[g.rule_class] ?? g.rule_class;
  const classCss   = g.rule_class.toLowerCase();

  const sorted = [...g.notices].sort((a, b) =>
    (SEV_ORDER[a.severity] ?? 9) - (SEV_ORDER[b.severity] ?? 9)
  );

  const rows = sorted.map(n => {
    const raw = formatMessage(n.message);
    const msg = raw.length > 160 ? raw.slice(0, 160) + '…' : raw;
    const sc  = SEVERITY_COLOR[n.severity] ?? '#666';
    const typeLabel = t(`entity.${n.entity_type}`) !== `entity.${n.entity_type}`
      ? t(`entity.${n.entity_type}`)
      : n.entity_type;
    const entityStr = n.entity_id
      ? `<span class="muted-text">${escHtml(typeLabel)}</span> <code class="entity-id">${escHtml(n.entity_id)}</code>`
      : `<span class="muted-text">${escHtml(typeLabel)}</span>`;
    return `
      <tr>
        <td style="color:${sc};white-space:nowrap">${SEVERITY_TR[n.severity]}</td>
        <td>${entityStr}</td>
        <td class="msg-cell">${escHtml(msg)}</td>
      </tr>`;
  }).join('');

  const desc = g.notices[0]?.remediation ?? '';

  return `
    <div class="rule-acc-section">
      <button class="rule-acc-header" type="button" aria-expanded="false">
        <span class="acc-arrow">▶</span>
        <span class="rule-title-block">
          <code class="rule-code rule-code-lg">${escHtml(g.rule_id)}</code>
          ${desc ? `<span class="rule-desc-text">${escHtml(desc)}</span>` : ''}
        </span>
        <span class="badge badge-${classCss}">${escHtml(classLabel)}</span>
        <span class="rule-sev-badge" style="color:${sevColor}">${escHtml(sevLabel)}</span>
        <span class="acc-count rule-notice-count">${g.notices.length}</span>
      </button>
      <div class="rule-acc-body" hidden>
        <div class="table-scroll">
          <table class="data-table">
            <thead><tr>
              <th>${t('rules.th.severity')}</th>
              <th>${t('rules.th.entity')}</th>
              <th>${t('rules.th.message')}</th>
            </tr></thead>
            <tbody>${rows}</tbody>
          </table>
        </div>
      </div>
    </div>`;
}

function attachListeners(root: HTMLElement): void {
  root.querySelectorAll<HTMLButtonElement>('.rule-acc-header').forEach(header => {
    header.addEventListener('click', () => {
      const section = header.closest<HTMLElement>('.rule-acc-section')!;
      const body    = section.querySelector<HTMLElement>('.rule-acc-body')!;
      const arrow   = header.querySelector<HTMLElement>('.acc-arrow')!;
      const isOpen  = !body.hidden;
      body.hidden = isOpen;
      arrow.textContent = isOpen ? '▶' : '▼';
      header.setAttribute('aria-expanded', String(!isOpen));
    });
  });
}

function formatMessage(msg: string): string {
  return msg
    .replace(/\btrip_id\b/g, 'sefer')
    .replace(/\bstop_sequence\b/g, 'Durak Sırası')
    .replace(/\barrival_time\b/g, 'Varış zamanı')
    .replace(/\bdeparture_time\b/g, 'Kalkış zamanı')
    .replace(/\bheadway\b/g, 'Sefer Aralığı');
}

function escHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
