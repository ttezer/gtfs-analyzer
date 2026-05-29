import type { Severity, RuleClass, FatalCode } from './types';
import tr from './locales/tr';
import en from './locales/en';

// ── Locale registry — yeni dil = buraya import + LOCALES'e ekle ──────────────

const LOCALES = { tr, en };
export type Locale = keyof typeof LOCALES;

// ── Locale state ──────────────────────────────────────────────────────────────

let _locale: Locale = (() => {
  const s = localStorage.getItem('gtfs-locale');
  return (s && s in LOCALES) ? s as Locale : 'tr';
})();

export function getLocale(): Locale { return _locale; }

export function setLocale(l: Locale): void {
  _locale = l;
  localStorage.setItem('gtfs-locale', l);
  _syncDicts();
}

// ── Locale-aware dicts (mutable — synced via Object.assign on locale change) ──
// Fix.ts ve rules.ts bu nesneleri doğrudan kullanır; locale değişiminde
// Object.assign ile güncellenir, yeniden render gerekmez.

export const SEVERITY_TR: Record<Severity, string>  = { ...tr.severity };
export const RULE_CLASS_TR: Record<RuleClass, string> = { ...tr.ruleClass };
export const FATAL_CODE_TR: Record<FatalCode, string> = { ...tr.fatalCode };

// ── Colors (locale-independent) ───────────────────────────────────────────────

export const SEVERITY_COLOR: Record<Severity, string> = {
  CRITICAL : 'var(--color-critical)',
  HIGH     : 'var(--color-high)',
  MEDIUM   : 'var(--color-medium)',
  LOW      : 'var(--color-low)',
  INFO     : 'var(--color-info)',
};

// ── t() — translate with optional {param} interpolation ──────────────────────

export function t(key: string, params?: Record<string, string | number>): string {
  if (key.startsWith('rule.')) {
    const ruleId = key.slice(5);
    return LOCALES[_locale].ruleTitles[ruleId] ?? LOCALES.tr.ruleTitles[ruleId] ?? ruleId;
  }
  let s = LOCALES[_locale].ui[key] ?? LOCALES.tr.ui[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      s = s.split(`{${k}}`).join(String(v));
    }
  }
  return s;
}

// ── tMsg() — notice message translate ─────────────────────────────────────────

export interface NoticeLike {
  rule_id: string;
  message: string;
  entity_id?: string | null;
  observed_value?: string | null;
  expected_value?: string | null;
  file?: string | null;
  field?: string | null;
  line?: number | null;
  details?: Record<string, string> | null;
}

function _noticeParams(n: NoticeLike): Record<string, string> {
  return {
    entity_id:      n.entity_id      ?? '',
    observed_value: n.observed_value ?? '',
    expected_value: n.expected_value ?? '',
    file:           n.file           ?? '',
    field:          n.field          ?? '',
    line:           String(n.line    ?? ''),
    ...(n.details ?? {}),
  };
}

export function tMsg(n: NoticeLike): string {
  if (_locale === 'tr') return n.message;
  const tpl = LOCALES.en.ruleMessages[n.rule_id];
  if (!tpl) return n.message;
  const params = _noticeParams(n);
  return tpl.replace(/\{(\w+)\}/g, (_, k) => params[k] ?? '');
}

export function tRemediation(n: { rule_id: string; remediation?: string | null }): string {
  if (_locale === 'tr') return n.remediation ?? '';
  return LOCALES.en.ruleRemediations[n.rule_id] ?? n.remediation ?? '';
}

// ── Internal sync ─────────────────────────────────────────────────────────────

function _syncDicts(): void {
  const l = LOCALES[_locale];
  Object.assign(SEVERITY_TR,   l.severity);
  Object.assign(RULE_CLASS_TR, l.ruleClass);
  Object.assign(FATAL_CODE_TR, l.fatalCode);
}

_syncDicts();
