import { describe, it, expect } from 'vitest';

// ── parseConfigDelta (upload.ts'ten inline edilmiş saf fonksiyon) ─────────────

function parseConfigDelta(delta: string): Record<string, number> {
  if (!delta || delta === '{}') return {};
  try {
    const parsed: unknown = JSON.parse(delta);
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return {};
    const result: Record<string, number> = {};
    for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
      if (typeof value === 'number' && isFinite(value)) result[key] = value;
    }
    return result;
  } catch { return {}; }
}

// ── csvCell (export.ts'ten inline edilmiş saf fonksiyon) ──────────────────────

function csvCell(v: string | number | null | undefined): string {
  const s = v == null ? '' : String(v);
  const safe = /^[=+\-@]/.test(s) ? `'${s}` : s;
  return safe.includes(',') || safe.includes('"') || safe.includes('\n')
    ? `"${safe.replace(/"/g, '""')}"`
    : safe;
}

// ── Testler ───────────────────────────────────────────────────────────────────

describe('parseConfigDelta', () => {
  it('boş string → boş obje', () => {
    expect(parseConfigDelta('')).toEqual({});
  });

  it('{} string → boş obje', () => {
    expect(parseConfigDelta('{}')).toEqual({});
  });

  it('geçerli number değerleri döner', () => {
    expect(parseConfigDelta('{"max_speed_bus_kmh":120,"min_transfer_time_sec":180}')).toEqual({
      max_speed_bus_kmh: 120,
      min_transfer_time_sec: 180,
    });
  });

  it('string value atlanır', () => {
    expect(parseConfigDelta('{"max_speed_bus_kmh":"FAST"}')).toEqual({});
  });

  it('null value atlanır', () => {
    expect(parseConfigDelta('{"max_speed_bus_kmh":null}')).toEqual({});
  });

  it('Infinity atlanır', () => {
    expect(parseConfigDelta('{"x":1e999}')).toEqual({});
  });

  it('array gelse boş obje döner', () => {
    expect(parseConfigDelta('[1,2,3]')).toEqual({});
  });

  it('geçersiz JSON → boş obje', () => {
    expect(parseConfigDelta('{bozuk json')).toEqual({});
  });

  it('karışık: geçerli ve geçersiz → sadece geçerliler', () => {
    expect(parseConfigDelta('{"a":10,"b":"x","c":20}')).toEqual({ a: 10, c: 20 });
  });
});

describe('csvCell', () => {
  it('normal string dokunulmaz', () => {
    expect(csvCell('normal')).toBe('normal');
  });

  it('null → boş string', () => {
    expect(csvCell(null)).toBe('');
  });

  it('undefined → boş string', () => {
    expect(csvCell(undefined)).toBe('');
  });

  it('virgül içeren değer tırnak içine alınır', () => {
    expect(csvCell('a,b')).toBe('"a,b"');
  });

  it('tırnak içeren değer escape edilir', () => {
    expect(csvCell('say "hello"')).toBe('"say ""hello"""');
  });

  it('= ile başlayan formula injection engellenir', () => {
    expect(csvCell('=SUM(A1:A10)')).toBe("'=SUM(A1:A10)");
  });

  it('+ ile başlayan engellenir', () => {
    expect(csvCell('+cmd')).toBe("'+cmd");
  });

  it('- ile başlayan engellenir', () => {
    expect(csvCell('-1+2')).toBe("'-1+2");
  });

  it('@ ile başlayan engellenir', () => {
    expect(csvCell('@SUM')).toBe("'@SUM");
  });

  it('number değer stringe çevrilir', () => {
    expect(csvCell(42)).toBe('42');
  });

  it('newline içeren değer tırnak içine alınır', () => {
    expect(csvCell('line1\nline2')).toBe('"line1\nline2"');
  });
});

// escHtml testleri → escape.test.ts (kanonik, tırnak kaçışlamayı da kapsar)
