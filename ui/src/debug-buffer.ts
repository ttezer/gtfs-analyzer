// Lightweight in-memory diagnostic buffers (#14 debug bundle).
// Privacy: store only application logs and user-action METADATA — feed content (stop
// names/coordinates/notice messages) is NEVER written here. The caller allowlists action
// details (e.g. file NAME + size, but not content).

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
  }).join(' ').slice(0, 500); // single-line limit
  logs.push({ t: nowIso(), level, msg });
  if (logs.length > LOG_LIMIT) logs.shift();
}

/** Wraps console.error/warn/info and writes to the ring buffer (preserving the originals). */
export function initDebugBuffer(): void {
  const levels: Array<'error' | 'warn' | 'info'> = ['error', 'warn', 'info'];
  for (const lvl of levels) {
    const orig = console[lvl].bind(console);
    console[lvl] = (...args: unknown[]) => { pushLog(lvl, args); orig(...args); };
  }
}

/** Records user-action METADATA. detail must contain only allowlisted data. */
export function logAction(type: string, detail?: string): void {
  actions.push({ t: nowIso(), type, detail });
  if (actions.length > ACTION_LIMIT) actions.shift();
}

export function getLogs(): readonly LogEntry[] { return logs; }
export function getActions(): readonly ActionEntry[] { return actions; }
