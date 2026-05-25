import type { Severity, RuleClass, FatalCode } from './types';

export const SEVERITY_TR: Record<Severity, string> = {
  CRITICAL : 'KRİTİK',
  HIGH     : 'YÜKSEK',
  MEDIUM   : 'ORTA',
  LOW      : 'DÜŞÜK',
  INFO     : 'BİLGİ',
};

export const RULE_CLASS_TR: Record<RuleClass, string> = {
  SPEC      : 'GTFS Geçerliliği',
  INTEROP   : 'GTFS Uyumluluğu',
  QUALITY   : 'GTFS Kalitesi',
  ANALYTICS : 'GTFS Analitiği',
};

export const FATAL_CODE_TR: Record<FatalCode, string> = {
  ZipUnreadable  : 'ZIP dosyası açılamadı',
  Utf8Critical   : 'UTF-8 kodlama hatası',
  NoRequiredFiles: 'Zorunlu dosyalar eksik',
  CsvMalformed   : 'CSV biçim hatası',
  ResourceLimit  : 'Notice limiti aşıldı',
  InvalidInput   : 'Geçersiz girdi',
};

export const SEVERITY_COLOR: Record<Severity, string> = {
  CRITICAL : 'var(--color-critical)',
  HIGH     : 'var(--color-high)',
  MEDIUM   : 'var(--color-medium)',
  LOW      : 'var(--color-low)',
  INFO     : 'var(--color-info)',
};
