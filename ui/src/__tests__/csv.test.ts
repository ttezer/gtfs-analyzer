import { describe, expect, it } from 'vitest';
import { csvCell } from '../csv';

describe('csvCell', () => {
  it.each(['=1+1', '+1', '-1', '@SUM(A1)', '\t=1', '\r=1'])('keeps %j as text (formula injection)', v => {
    expect(csvCell(v).replace(/^"/, '').startsWith("'")).toBe(true);
  });
  it('quotes separators, quotes and line breaks', () => {
    expect(csvCell('a,b')).toBe('"a,b"');
    expect(csvCell('a"b')).toBe('"a""b"');
    expect(csvCell('a\nb')).toBe('"a\nb"');
    expect(csvCell('a\rb')).toBe('"a\rb"');
  });
  it('leaves plain values and numbers alone', () => {
    expect(csvCell('STP_016')).toBe('STP_016');
    expect(csvCell(42)).toBe('42');
    expect(csvCell(null)).toBe('');
  });
});
