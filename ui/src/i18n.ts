import type { Severity, RuleClass, FatalCode } from './types';
import tr from './locales/tr';
import en from './locales/en';
import ja from './locales/ja';

// ── Locale registry — yeni dil = buraya import + LOCALES'e ekle ──────────────

const LOCALES = { tr, en, ja };
export type Locale = keyof typeof LOCALES;

// ── Locale state ──────────────────────────────────────────────────────────────

let _locale: Locale = (() => {
  if (typeof localStorage === 'undefined') return 'tr';
  const s = localStorage.getItem('gtfs-locale');
  return (s && s in LOCALES) ? s as Locale : 'tr';
})();

// Belge dilini locale ile senkronla — CSS text-transform:uppercase doğru
// dil kurallarını kullansın (örn. EN'de "VALIDITY", TR'de "GEÇERLİLİK";
// aksi halde EN metni Türkçe noktalı-İ ile büyür).
if (typeof document !== 'undefined') document.documentElement.lang = _locale;

export function getLocale(): Locale { return _locale; }

export function setLocale(l: Locale): void {
  _locale = l;
  if (typeof localStorage !== 'undefined') localStorage.setItem('gtfs-locale', l);
  if (typeof document !== 'undefined') document.documentElement.lang = l;
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
  return tForLocale(_locale, key, params);
}

/** Translate without mutating the application's active locale. */
export function tForLocale(locale: Locale, key: string, params?: Record<string, string | number>): string {
  if (key.startsWith('rule.')) {
    const ruleId = key.slice(5);
    return LOCALES[locale].ruleTitles[ruleId] ?? LOCALES.en.ruleTitles[ruleId] ?? LOCALES.tr.ruleTitles[ruleId] ?? ruleId;
  }
  let s = LOCALES[locale].ui[key] ?? LOCALES.en.ui[key] ?? LOCALES.tr.ui[key] ?? key;
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
  return tMsgForLocale(_locale, n);
}

/** Translate a notice without mutating the application's active locale. */
export function tMsgForLocale(locale: Locale, n: NoticeLike): string {
  if (locale === 'tr') return n.message;
  // JA: kendi mesajı yoksa EN şablonuna fallback
  const tpl = LOCALES[locale].ruleMessages[n.rule_id] ?? LOCALES.en.ruleMessages[n.rule_id];
  if (!tpl) return n.message;
  const params = _noticeParams(n);
  return tpl.replace(/\{(\w+)\}/g, (_, k) => params[k] ?? '');
}

export function tRemediation(n: { rule_id: string; remediation?: string | null }): string {
  return tRemediationForLocale(_locale, n);
}

/** Translate remediation text without mutating the application's active locale. */
export function tRemediationForLocale(locale: Locale, n: { rule_id: string; remediation?: string | null }): string {
  if (locale === 'tr') return n.remediation ?? '';
  // Mevcut locale → EN → TR (Rust mesajı)
  return LOCALES[locale].ruleRemediations[n.rule_id]
    ?? LOCALES.en.ruleRemediations[n.rule_id]
    ?? n.remediation ?? '';
}

export function severityForLocale(locale: Locale, severity: Severity): string {
  return LOCALES[locale].severity[severity] ?? severity;
}

export function ruleClassForLocale(locale: Locale, ruleClass: RuleClass): string {
  return LOCALES[locale].ruleClass[ruleClass] ?? ruleClass;
}

// ── Internal sync ─────────────────────────────────────────────────────────────

function _syncDicts(): void {
  const l = LOCALES[_locale];
  Object.assign(SEVERITY_TR,   l.severity);
  Object.assign(RULE_CLASS_TR, l.ruleClass);
  Object.assign(FATAL_CODE_TR, l.fatalCode);
}

_syncDicts();
