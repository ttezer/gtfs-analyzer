// One CSV cell for files users open in a spreadsheet. Feed values and uploaded golden files are
// untrusted: a cell starting with = + - @ (or a tab / carriage return, which some spreadsheets
// strip before evaluating) is run as a formula — CSV/formula injection (OWASP). Prefix it with an
// apostrophe so it stays text. Quote cells that contain a separator, a quote or a line break.
export function csvCell(v: string | number | null | undefined): string {
  const s = v == null ? '' : String(v);
  const safe = /^[=+\-@\t\r]/.test(s) ? `'${s}` : s;
  return /[",\r\n]/.test(safe) ? `"${safe.replace(/"/g, '""')}"` : safe;
}
