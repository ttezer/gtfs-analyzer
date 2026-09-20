/**
 * Gizlenen kural listesinin SAF mantığı: `configDelta` JSON metnini alır, yenisini
 * döndürür. Tarayıcıya (sessionStorage, DOM, worker) hiç dokunmaz — bu yüzden
 * birim testi edilebilir; durum bağlantısı `hidden-rules.ts`'te kalır.
 *
 * Delta, ayar panelinin yazdığı nesnenin aynısıdır: `disabled_rule_ids` dışındaki
 * anahtarlar (eşikler, GTFS-JP profili) KORUNUR. Liste boşalırsa anahtar düşer;
 * delta tamamen boşalırsa `''` döner, çünkü motor boş metni "ayar yok" sayar.
 */
const KEY = 'disabled_rule_ids';

function parseDelta(delta: string): Record<string, unknown> {
  if (!delta || delta === '{}') return {};
  try {
    const parsed: unknown = JSON.parse(delta);
    return typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch { return {}; }
}

function serializeDelta(delta: Record<string, unknown>): string {
  const entries = Object.entries(delta).filter(([, v]) => !(Array.isArray(v) && v.length === 0));
  return entries.length > 0 ? JSON.stringify(Object.fromEntries(entries)) : '';
}

/** Deltadaki gizli kural kimlikleri; bozuk/eksik delta boş liste verir. */
export function parseHiddenRules(delta: string): string[] {
  const value = parseDelta(delta)[KEY];
  return Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [];
}

/** Kuralı listeye ekler. Zaten varsa `null` döner (değişiklik yok). */
export function withHiddenRule(delta: string, ruleId: string): string | null {
  const current = parseHiddenRules(delta);
  if (current.includes(ruleId)) return null;
  const next = parseDelta(delta);
  next[KEY] = [...current, ruleId].sort();
  return serializeDelta(next);
}

/** Kuralı listeden çıkarır. Listede yoksa `null` döner (değişiklik yok). */
export function withoutHiddenRule(delta: string, ruleId: string): string | null {
  const current = parseHiddenRules(delta);
  if (!current.includes(ruleId)) return null;
  const next = parseDelta(delta);
  next[KEY] = current.filter(id => id !== ruleId);
  return serializeDelta(next);
}
