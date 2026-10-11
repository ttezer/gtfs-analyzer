import { describe, expect, it } from 'vitest';
import { renderFeedSummary, safeHttpUrl, safeMailto } from '../pages/feed-summary';
import type { ValidationResult } from '../types';

const resultWith = (feed_summary: ValidationResult['feed_summary']): ValidationResult =>
  ({ feed_summary } as unknown as ValidationResult);

describe('feed summary card', () => {
  it('links only http(s) URLs and plain email addresses', () => {
    expect(safeHttpUrl('https://gtfs.example/a')).toBe('https://gtfs.example/a');
    expect(safeHttpUrl('javascript:alert(1)')).toBeNull();
    expect(safeHttpUrl('data:text/html,x')).toBeNull();
    expect(safeHttpUrl('www.example.com')).toBeNull();
    expect(safeMailto('ops@pub.example')).toBe('mailto:ops@pub.example');
    expect(safeMailto('a@b.c?subject=x"><script>')).toBeNull();
    expect(safeMailto('not an email')).toBeNull();
  });

  it('escapes feed text and keeps unsafe URLs as plain text', () => {
    const html = renderFeedSummary(resultWith({
      agency_count: 1,
      agencies: [{
        agency_id: 'BF',
        agency_name: '<img src=x onerror=alert(1)>',
        agency_url: 'javascript:alert(1)',
        agency_email: 'bookings@ferry.example',
      }],
      feed_info: { feed_publisher_url: 'https://gtfs.example/', feed_version: '1' },
    }));
    expect(html).not.toContain('<img');
    expect(html).toContain('&lt;img src=x onerror=alert(1)&gt;');
    expect(html).not.toContain('href="javascript:');
    expect(html).toContain('href="mailto:bookings@ferry.example"');
    expect(html).toContain('href="https://gtfs.example/" target="_blank" rel="noopener noreferrer"');
  });

  it('uses a table with only the populated columns for several agencies', () => {
    const html = renderFeedSummary(resultWith({
      agency_count: 2,
      agencies: [
        { agency_id: 'A', agency_name: 'Alpha', agency_phone: '1' },
        { agency_id: 'B', agency_name: 'Beta' },
      ],
    }));
    expect(html).toContain('<table class="data-table">');
    expect(html).toContain('<td class="fs-val">1</td>');
    // Acente + telefon; boş e-posta/saat dilimi sütunları yok.
    expect((html.match(/<th>/g) ?? []).length).toBe(2);
  });

  it('says how many agencies are not carried', () => {
    const html = renderFeedSummary(resultWith({
      agency_count: 103,
      agencies: [{ agency_name: 'A' }],
    }));
    expect(html).toContain('102');
  });

  it('renders nothing without agencies or feed_info', () => {
    expect(renderFeedSummary(resultWith(undefined))).toBe('');
    expect(renderFeedSummary(resultWith({ agency_count: 0 }))).toBe('');
  });
});
