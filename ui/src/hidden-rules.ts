import { getState, setConfigDelta } from './state';

/**
 * Kullanıcının gizlediği kural kimlikleri, `configDelta`'nın `disabled_rule_ids`
 * alanında tutulur — yani ayar panelinin yazdığı yerin aynısı. Motor bu listeyi
 * K7'den ÖNCE uygular: gizlenen kural ne bulgularda, ne skorlarda, ne de düzeltme
 * kuyruğunda görünür.
 *
 * ⚠️ Spec sınıfı kurallar buraya GİREMEZ: yayın kararı onlara dayanır ve motor
 * (`gtfs_pipeline::check_rule_scope`) böyle bir isteği fatal hata ile reddeder.
 * Arayüz bu yüzden Spec kurallarında gizle düğmesini hiç göstermez.
 */
function readDelta(): Record<string, unknown> {
  const raw = getState().configDelta;
  if (!raw || raw === '{}') return {};
  try {
    const parsed: unknown = JSON.parse(raw);
    return typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch { return {}; }
}

function writeDelta(delta: Record<string, unknown>): void {
  const entries = Object.entries(delta).filter(([, v]) => !(Array.isArray(v) && v.length === 0));
  setConfigDelta(entries.length > 0 ? JSON.stringify(Object.fromEntries(entries)) : '');
}

/** Gizlenmiş kural kimlikleri (sıralı, tekrarsız). */
export function hiddenRules(): string[] {
  const value = readDelta()['disabled_rule_ids'];
  return Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [];
}

export function isHidden(ruleId: string): boolean {
  return hiddenRules().includes(ruleId);
}

/** Kuralı gizler. Zaten gizliyse hiçbir şey yapmaz; dönüş: liste değişti mi. */
export function hideRule(ruleId: string): boolean {
  const current = hiddenRules();
  if (current.includes(ruleId)) return false;
  const delta = readDelta();
  delta['disabled_rule_ids'] = [...current, ruleId].sort();
  writeDelta(delta);
  return true;
}

/** Kuralı yeniden görünür yapar. Dönüş: liste değişti mi. */
export function unhideRule(ruleId: string): boolean {
  const current = hiddenRules();
  if (!current.includes(ruleId)) return false;
  const delta = readDelta();
  delta['disabled_rule_ids'] = current.filter(id => id !== ruleId);
  writeDelta(delta);
  return true;
}
