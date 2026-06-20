// Hafif, bellek-içi teşhis tamponları (#14 debug bundle).
// Gizlilik: yalnızca uygulama log'ları ve kullanıcı aksiyon META'sı tutulur — feed
// içeriği (durak adı/koordinat/notice mesajı) ASLA buraya yazılmaz. Aksiyon detayları
// çağıran tarafça whitelist edilir (örn. dosya ADI + boyut, ama içerik değil).

const LOG_LIMIT = 100;
const ACTION_LIMIT = 50;

export interface LogEntry { t: string; level: string; msg: string }
export interface ActionEntry { t: string; type: string; detail?: string }

const logs: LogEntry[] = [];
const actions: ActionEntry[] = [];

function nowIso(): string {
  return new Date().toISOString().slice(0, 19).replace('T', ' ');
}

function pushLog(level: string, args: unknown[]): void {
  const msg = args.map(a => {
    if (typeof a === 'string') return a;
    try { return JSON.stringify(a); } catch { return String(a); }
  }).join(' ').slice(0, 500); // tek satır sınırı
  logs.push({ t: nowIso(), level, msg });
  if (logs.length > LOG_LIMIT) logs.shift();
}

/** console.error/warn/info'yu sarmalayıp halka-tampona yazar (orijinali korunur). */
export function initDebugBuffer(): void {
  const levels: Array<'error' | 'warn' | 'info'> = ['error', 'warn', 'info'];
  for (const lvl of levels) {
    const orig = console[lvl].bind(console);
    console[lvl] = (...args: unknown[]) => { pushLog(lvl, args); orig(...args); };
  }
}

/** Kullanıcı aksiyonu META'sı kaydeder. detail yalnızca whitelist veri içermeli. */
export function logAction(type: string, detail?: string): void {
  actions.push({ t: nowIso(), type, detail });
  if (actions.length > ACTION_LIMIT) actions.shift();
}

export function getLogs(): readonly LogEntry[] { return logs; }
export function getActions(): readonly ActionEntry[] { return actions; }
