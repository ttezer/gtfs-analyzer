import { getState, setConfigDelta } from './state';
import { parseHiddenRules, withHiddenRule, withoutHiddenRule } from './hidden-rules-core';
import { t } from './i18n';
import { escHtml } from './escape';
import { rerunValidation } from './validator-client';

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
export function hiddenRules(): string[] {
  return parseHiddenRules(getState().configDelta);
}

export function isHidden(ruleId: string): boolean {
  return hiddenRules().includes(ruleId);
}

/** Kuralı gizler. Zaten gizliyse hiçbir şey yapmaz; dönüş: liste değişti mi. */
export function hideRule(ruleId: string): boolean {
  const next = withHiddenRule(getState().configDelta, ruleId);
  if (next === null) return false;
  setConfigDelta(next);
  return true;
}

/** Kuralı yeniden görünür yapar. Dönüş: liste değişti mi. */
export function unhideRule(ruleId: string): boolean {
  const next = withoutHiddenRule(getState().configDelta, ruleId);
  if (next === null) return false;
  setConfigDelta(next);
  return true;
}

/**
 * Gizlenen kuralların şeridi. HER rapor sayfasının en üstünde görünür (Rapor ve
 * Ayrıntı/Düzeltme): kullanıcı hangi sayfada olursa olsun neyin ölçüm dışı kaldığını
 * görebilmeli ve tek tıkla geri getirebilmeli.
 */
export function renderHiddenRulesBar(): string {
  const hidden = hiddenRules();
  if (hidden.length === 0) return '';
  const chips = hidden.map(id =>
    `<button class="hidden-rule-chip" data-rule="${escHtml(id)}" title="${t('fix.unhide_rule')}">${escHtml(id)} ↩</button>`
  ).join('');
  return `
    <div class="card hidden-rules-card">
      <p class="hint">${t('fix.hidden_note', { count: hidden.length })}</p>
      <div class="hidden-rule-chips">${chips}</div>
    </div>`;
}

/**
 * Gizleme/gösterme tek yoldan işler: listeyi güncelle → K6+K7'yi cache'ten yeniden
 * koş → sonucu tazele. K1-K5 TEKRARLANMAZ, büyük feed'de de anlık gelir.
 * `setResult` sayfayı 'domain'e döndürdüğü için kullanıcının sayfası geri konur.
 */
export async function applyRuleVisibility(changed: boolean): Promise<void> {
  if (!changed) return;
  const { renderApp } = await import('./main');
  const { setResult, setPage } = await import('./state');
  const state = getState();
  const page = state.page;
  // Yeniden çizim sayfayı sıfırdan kurar: R9 akordeonu kapanır, önem filtresi
  // seçimi silinir ve sayfa başa döner. Kullanıcı okuduğu yerde kalmalı.
  // Sınıf/dosya filtreleri state'te tutulduğu için kendiliğinden korunur; önem
  // rozetleri yalnız DOM'da yaşar, bu yüzden burada yakalanır.
  const r9Open = document.querySelector<HTMLDetailsElement>('.r9-card')?.open ?? false;
  const scrollY = window.scrollY;
  const activeSev = (id: string): string[] => {
    const chips = Array.from(document.querySelectorAll(`#${id} .sev-chip.active`)) as HTMLButtonElement[];
    return chips.map(chip => chip.dataset['sev'] ?? '').filter(Boolean);
  };
  const sevSelection = { r2: activeSev('r2-sev-filter'), r9: activeSev('r9-sev-filter') };
  try {
    const fresh = await rerunValidation(state.configDelta);
    setResult(fresh, state.fileName, state.fileSize ?? 0, state.reportDurationMs);
    setPage(page);
  } catch {
    // Yeniden koşum başarısızsa ayar yine de kayıtlı; sonraki koşumda uygulanır.
  }
  renderApp();
  const r9 = document.querySelector<HTMLDetailsElement>('.r9-card');
  if (r9 && r9Open) r9.open = true;
  // Rozete tıklamak hem 'active' işaretini hem de filtre uygulamasını geri getirir;
  // filtre mantığını burada TEKRARLAMAK iki kopya demek olurdu.
  for (const [id, sevs] of [['r2-sev-filter', sevSelection.r2], ['r9-sev-filter', sevSelection.r9]] as const) {
    for (const sev of sevs) {
      document.querySelector<HTMLButtonElement>(`#${id} .sev-chip[data-sev="${sev}"]`)?.click();
    }
  }
  window.scrollTo({ top: scrollY });
}

/** Şerit rozetleri ve bulgu satırındaki gizle düğmeleri için dinleyiciler. */
export function attachHiddenRuleListeners(root: HTMLElement): void {
  root.querySelectorAll<HTMLButtonElement>('.hide-rule-btn').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation();  // R9 satırı tıklanınca açılır; gizleme onu tetiklemesin.
      const rule = btn.dataset['rule'];
      if (rule) void applyRuleVisibility(hideRule(rule));
    });
  });
  root.querySelectorAll<HTMLButtonElement>('.hidden-rule-chip').forEach(chip => {
    chip.addEventListener('click', () => {
      const rule = chip.dataset['rule'];
      if (rule) void applyRuleVisibility(unhideRule(rule));
    });
  });
}
