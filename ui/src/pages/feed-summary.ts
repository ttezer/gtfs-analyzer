// Ajans ve yayıncı künyesi kartı (`result.feed_summary`). Değerler feed'den gelir, yani
// güvenilmezdir: her metin kaçışlanır, bağlantı yalnız http(s) URL'si ve düz e-posta
// adresi için kurulur (javascript:, data: vb. düz metin kalır).
import type { AgencyContact, FeedInfoSummary, ValidationResult } from '../types';
import { t, intlLocale } from '../i18n';
import { escHtml } from '../escape';

/** Yalnız http/https mutlak URL'yi bağlantı yapılabilir sayar. */
export function safeHttpUrl(value: string): string | null {
  try {
    const url = new URL(value);
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.href : null;
  } catch {
    return null;
  }
}

const EMAIL_RE = /^[^\s@<>"'()\\,;:]+@[^\s@<>"'()\\,;:]+\.[^\s@<>"'()\\,;:]+$/;

/** Düz bir e-posta adresi için `mailto:` hedefi; başka her şey null. */
export function safeMailto(value: string): string | null {
  return EMAIL_RE.test(value) ? `mailto:${value}` : null;
}

function link(value: string, href: string | null): string {
  if (!href) return escHtml(value);
  const external = href.startsWith('mailto:') ? '' : ' target="_blank" rel="noopener noreferrer"';
  return `<a href="${escHtml(href)}"${external}>${escHtml(value)}</a>`;
}

function languageName(code: string): string {
  try {
    const name = new Intl.DisplayNames([intlLocale()], { type: 'language' }).of(code);
    return name && name !== code ? `${name} (${code})` : code;
  } catch {
    return code;
  }
}

function row(labelKey: string, html: string | null): string {
  if (!html) return '';
  return `<div class="cal-row"><span class="cal-label">${t(labelKey)}</span><span class="cal-val fs-val">${html}</span></div>`;
}

const opt = (v: string | undefined, f: (s: string) => string): string | null => (v ? f(v) : null);

function renderAgency(a: AgencyContact): string {
  const name = a.agency_name || a.agency_id || t('agency.unnamed');
  const id = a.agency_id && a.agency_id !== a.agency_name
    ? ` <span class="cal-sub">${escHtml(a.agency_id)}</span>`
    : '';
  return `
    <div class="cal-rows fs-group">
      ${row('domain.feed_summary.agency', `${escHtml(name)}${id}`)}
      ${row('domain.feed_summary.website', opt(a.agency_url, v => link(v, safeHttpUrl(v))))}
      ${row('domain.feed_summary.phone', opt(a.agency_phone, escHtml))}
      ${row('domain.feed_summary.email', opt(a.agency_email, v => link(v, safeMailto(v))))}
      ${row('domain.feed_summary.fare_url', opt(a.agency_fare_url, v => link(v, safeHttpUrl(v))))}
      ${row('domain.feed_summary.timezone', opt(a.agency_timezone, escHtml))}
      ${row('domain.feed_summary.language', opt(a.agency_lang, v => escHtml(languageName(v))))}
    </div>`;
}

type Column = { key: keyof AgencyContact; label: string; cell: (v: string) => string };

const AGENCY_COLUMNS: Column[] = [
  { key: 'agency_url', label: 'domain.feed_summary.website', cell: v => link(v, safeHttpUrl(v)) },
  { key: 'agency_phone', label: 'domain.feed_summary.phone', cell: escHtml },
  { key: 'agency_email', label: 'domain.feed_summary.email', cell: v => link(v, safeMailto(v)) },
  { key: 'agency_fare_url', label: 'domain.feed_summary.fare_url', cell: v => link(v, safeHttpUrl(v)) },
  { key: 'agency_timezone', label: 'domain.feed_summary.timezone', cell: escHtml },
  { key: 'agency_lang', label: 'domain.feed_summary.language', cell: v => escHtml(languageName(v)) },
];

/** Birden fazla acente: kompakt tablo; yalnız en az bir acentede dolu olan sütunlar. */
function renderAgencyTable(agencies: AgencyContact[]): string {
  const cols = AGENCY_COLUMNS.filter(c => agencies.some(a => a[c.key]));
  const head = cols.map(c => `<th>${t(c.label)}</th>`).join('');
  const body = agencies.map(a => {
    const name = a.agency_name || a.agency_id || t('agency.unnamed');
    const id = a.agency_id && a.agency_id !== a.agency_name ? ` <span class="cal-sub">${escHtml(a.agency_id)}</span>` : '';
    const cells = cols.map(c => {
      const v = a[c.key];
      return `<td class="fs-val">${v ? c.cell(v) : ''}</td>`;
    }).join('');
    return `<tr><td><strong>${escHtml(name)}</strong>${id}</td>${cells}</tr>`;
  }).join('');
  return `
    <div class="table-scroll fs-group">
      <table class="data-table">
        <thead><tr><th>${t('domain.feed_summary.agency')}</th>${head}</tr></thead>
        <tbody>${body}</tbody>
      </table>
    </div>`;
}

function renderFeedInfo(f: FeedInfoSummary): string {
  return `
    <div class="cal-rows fs-group">
      ${row('domain.feed_summary.publisher', opt(f.feed_publisher_name, escHtml))}
      ${row('domain.feed_summary.publisher_url', opt(f.feed_publisher_url, v => link(v, safeHttpUrl(v))))}
      ${row('domain.feed_summary.contact_email', opt(f.feed_contact_email, v => link(v, safeMailto(v))))}
      ${row('domain.feed_summary.contact_url', opt(f.feed_contact_url, v => link(v, safeHttpUrl(v))))}
      ${row('domain.feed_summary.feed_language', opt(f.feed_lang, v => escHtml(languageName(v))))}
      ${row('domain.feed_summary.feed_version', opt(f.feed_version, escHtml))}
    </div>`;
}

export function renderFeedSummary(result: ValidationResult): string {
  const s = result.feed_summary;
  if (!s) return '';
  const agencies = s.agencies ?? [];
  if (agencies.length === 0 && !s.feed_info) return '';
  const hidden = s.agency_count - agencies.length;
  const more = hidden > 0
    ? `<div class="cal-sub">${escHtml(t('domain.feed_summary.more_agencies', { n: hidden }))}</div>`
    : '';
  return `
    <div class="card">
      <h3 class="rpt-section-title">${t('domain.feed_summary.title')}</h3>
      ${agencies.length === 1 ? renderAgency(agencies[0]) : agencies.length > 1 ? renderAgencyTable(agencies) : ''}
      ${more}
      ${s.feed_info ? renderFeedInfo(s.feed_info) : ''}
    </div>`;
}
