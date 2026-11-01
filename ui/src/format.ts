// Formatting numeric values for display. For dates/times → dates.ts

/// Converts a byte count to a readable unit. Uses `Intl.NumberFormat` at the MB threshold:
/// the decimal separator follows the user's language (tr "12,3 MB" · en "12.3 MB").
export function formatBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${Math.round(b / 1024)} KB`;
  const mb = b / (1024 * 1024);
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(mb)} MB`;
}
