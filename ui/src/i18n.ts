import type { Severity, RuleClass, FatalCode, FatalError } from './types';
import tr, { trFragmentTemplates, trMessageTemplates, trMessageTexts, trRemediationTemplates, trRemediationTexts } from './locales/tr';
import en, { enEngineMessageRules } from './locales/en';
import ja from './locales/ja';
import fr from './locales/fr';

// ── Locale registry — new language = import here and add to LOCALES ─────────

const LOCALES = { tr, en, ja, fr };
export type Locale = keyof typeof LOCALES;

// ── Locale state ──────────────────────────────────────────────────────────────

let _locale: Locale = (() => {
  if (typeof localStorage === 'undefined') return 'tr';
  const s = localStorage.getItem('gtfs-locale');
  return (s && Object.hasOwn(LOCALES, s)) ? s as Locale : 'tr';
})();

// Synchronize the document language with the locale so CSS text-transform:uppercase uses
// the correct language rules (e.g. "VALIDITY" in EN, "GEÇERLİLİK" in TR;
// otherwise English text is uppercased with Turkish dotted İ).
if (typeof document !== 'undefined') document.documentElement.lang = _locale;

export function getLocale(): Locale { return _locale; }

// ── Intl tags ───────────────────────────────────────────────────────────────
// BCP-47 equivalent of the application language; the ONE source for number/date/sort
// formatting. Because this is `Record<Locale, …>`, tsc fails when a new language is added
// — the only guard against silently falling back to the wrong locale.
export const INTL_LOCALE: Record<Locale, string> = {
  tr: 'tr-TR', en: 'en-US', ja: 'ja-JP', fr: 'fr-FR',
};

/** Aktif dilin BCP-47 etiketi. `toLocaleString`/`Intl.*` çağrılarında kullanın. */
export function intlLocale(): string { return INTL_LOCALE[_locale]; }

export function setLocale(l: Locale): void {
  _locale = l;
  if (typeof localStorage !== 'undefined') localStorage.setItem('gtfs-locale', l);
  if (typeof document !== 'undefined') document.documentElement.lang = l;
  _syncDicts();
}

// ── Locale-aware dicts (mutable — synced via Object.assign on locale change) ──
// Fix.ts and rules.ts use these objects directly; they are updated with Object.assign when
// the locale changes, so no rerender is needed.

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

type TemplatePart = { text: string } | { hole: string };

function templateParts(template: string): TemplatePart[] {
  const parts: TemplatePart[] = [];
  const re = /\{(\w+)\}/g;
  let textStart = 0;
  for (let m = re.exec(template); m; m = re.exec(template)) {
    if (m.index > textStart) parts.push({ text: template.slice(textStart, m.index) });
    parts.push({ hole: m[1] });
    textStart = m.index + m[0].length;
  }
  if (textStart < template.length) parts.push({ text: template.slice(textStart) });
  return parts;
}

/**
 * Recovers the hole values of `template` from `text` — same rules as the Rust
 * `match_template`: every split is tried, and an ambiguous message (two different splits)
 * returns `null` so it keeps the engine's text.
 */
export function matchTemplate(template: string, text: string): Record<string, string> | null {
  const parts = templateParts(template);
  const found: Record<string, string>[] = [];
  const caps: Record<string, string> = {};
  const go = (i: number, pos: number): void => {
    if (found.length > 1) return;
    const part = parts[i];
    if (!part) {
      if (pos === text.length) found.push({ ...caps });
      return;
    }
    if ('text' in part) {
      if (text.startsWith(part.text, pos)) go(i + 1, pos + part.text.length);
      return;
    }
    const next = parts[i + 1];
    if (!next) {
      caps[part.hole] = text.slice(pos);
      go(i + 1, text.length);
      delete caps[part.hole];
      return;
    }
    if (!('text' in next)) return; // two holes in a row cannot be split
    for (let at = text.indexOf(next.text, pos); at !== -1; at = text.indexOf(next.text, at + 1)) {
      caps[part.hole] = text.slice(pos, at);
      go(i + 1, at);
      delete caps[part.hole];
      if (found.length > 1) return;
    }
  };
  go(0, 0);
  return found.length === 1 ? found[0] : null;
}

/** Same as Rust `translate_fragment`: empty stays empty, a known fragment is translated. */
function translateFragment(value: string): string {
  if (!value) return '';
  for (const engine of Object.keys(trFragmentTemplates).sort()) {
    const caps = matchTemplate(engine, value);
    if (caps) {
      // A fragment may hold a fragment; each level is a strict substring, so this ends.
      return trFragmentTemplates[engine].replace(/\{(\w+)\}/g, (_, k: string) => {
        const inner = caps[k] ?? '';
        return k.startsWith('f_') && inner !== value ? translateFragment(inner) : inner;
      });
    }
  }
  return value;
}

/**
 * The first template of the rule (in Rust's sorted order) that matches `message`, filled.
 * Holes named `f_…` hold sentence fragments and are translated through `trFragmentTemplates`.
 */
function translateByTemplate(templates: Record<string, string> | undefined, message: string): string | undefined {
  if (!templates) return undefined;
  for (const engine of Object.keys(templates).sort()) {
    const caps = matchTemplate(engine, message);
    if (caps) {
      return templates[engine].replace(/\{(\w+)\}/g, (_, k: string) =>
        k.startsWith('f_') ? translateFragment(caps[k] ?? '') : (caps[k] ?? ''));
    }
  }
  return undefined;
}

const enEngineMessageSet = new Set(enEngineMessageRules);

/** Translate a notice without mutating the application's active locale. */
export function tMsgForLocale(locale: Locale, n: NoticeLike): string {
  // Fixed messages moved to the Turkish dictionary are matched by the engine's own text (issue #201).
  if (locale === 'tr') {
    return trMessageTexts[n.rule_id]?.[n.message]
      ?? translateByTemplate(trMessageTemplates[n.rule_id], n.message)
      ?? translateByTemplate(trMessageTemplates['*'], n.message)
      ?? n.message;
  }
  if (locale === 'en' && enEngineMessageSet.has(n.rule_id)) return n.message;
  // JA: fall back to the EN template when there is no Japanese message
  const profile = n.details?.jp_profile?.toLowerCase();
  const variant = profile ? `${n.rule_id}.${profile}` : n.rule_id;
  const specific = n.details?.message_variant ? `${variant}.${n.details.message_variant}` : variant;
  const tpl = LOCALES[locale].ruleMessages[specific]
    ?? LOCALES.en.ruleMessages[specific]
    ?? LOCALES[locale].ruleMessages[variant]
    ?? LOCALES.en.ruleMessages[variant]
    ?? LOCALES[locale].ruleMessages[n.rule_id]
    ?? LOCALES.en.ruleMessages[n.rule_id];
  if (!tpl) return n.message;
  const params = _noticeParams(n);
  return tpl.replace(/\{(\w+)\}/g, (_, k) => params[k] ?? '');
}

/**
 * Fatal mesajını bildirimlerle aynı modelle çevirir: `{code}.{variant}` şablonu,
 * yoksa İngilizcesi, o da yoksa pipeline'ın kendi (Türkçe) metni.
 */
export function tFatal(error: FatalError, locale: Locale = _locale): string {
  // Same as Rust `translate_fatal`: the engine's English fatal text is mapped by template first.
  if (locale === 'tr') {
    const byTemplate = translateByTemplate(trMessageTemplates.FATAL, error.message);
    if (byTemplate !== undefined) return byTemplate;
  }
  if (!error.params) return error.message;
  const variant = error.params.variant;
  const key = variant ? `${error.code}.${variant}` : error.code;
  const tpl = LOCALES[locale].fatalMessages[key]
    ?? (locale === 'tr' ? undefined : LOCALES.en.fatalMessages[key]);
  if (!tpl) return error.message;
  const params = error.params;
  return tpl.replace(/\{(\w+)\}/g, (_, k) => params[k] ?? '');
}

export function tRemediation(n: { rule_id: string; remediation?: string | null; details?: Record<string, string> | null }): string {
  return tRemediationForLocale(_locale, n);
}

/** Translate remediation text without mutating the application's active locale. */
export function tRemediationForLocale(locale: Locale, n: { rule_id: string; remediation?: string | null; details?: Record<string, string> | null }): string {
  if (locale === 'tr') {
    // Same order as the Rust translator: variant → profile → rule, no English fallback;
    // rules not moved to the Turkish dictionary yet keep the engine's own text (issue #201).
    const profile = n.details?.jp_profile?.toLowerCase();
    const variant = n.details?.message_variant;
    const own = LOCALES.tr.ruleRemediations;
    // A rule whose branches give different remediations is matched by the engine's own text.
    const byText = trRemediationTexts[n.rule_id]?.[n.remediation ?? '']
      ?? translateByTemplate(trRemediationTemplates[n.rule_id], n.remediation ?? '')
      ?? translateByTemplate(trRemediationTemplates['*'], n.remediation ?? '');
    if (byText !== undefined) return byText;
    return (variant ? own[profile ? `${n.rule_id}.${profile}.${variant}` : `${n.rule_id}.${variant}`] : undefined)
      ?? (profile ? own[`${n.rule_id}.${profile}`] : undefined)
      ?? own[n.rule_id]
      ?? n.remediation ?? '';
  }
  // Current locale → EN → TR (Rust message)
  const specific = n.details?.message_variant
    ? n.details.jp_profile
      ? `${n.rule_id}.${n.details.jp_profile.toLowerCase()}.${n.details.message_variant}`
      : `${n.rule_id}.${n.details.message_variant}`
    : n.rule_id;
  return LOCALES[locale].ruleRemediations[specific]
    ?? LOCALES.en.ruleRemediations[specific]
    ?? LOCALES[locale].ruleRemediations[n.rule_id]
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
