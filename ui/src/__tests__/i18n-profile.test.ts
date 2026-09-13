import { describe, expect, it } from 'vitest';
import { tMsgForLocale } from '../i18n';

describe('GTFS-JP profile-specific notice messages', () => {
  it('uses the explicit profile variant and falls back to the neutral message', () => {
    const base = {
      rule_id: 'JPN_006',
      message: 'Türkçe çalışma zamanı mesajı',
    };

    const v4 = tMsgForLocale('en', {
      ...base,
      details: { jp_profile: 'V4' },
    });
    const legacy = tMsgForLocale('en', base);

    expect(v4).toContain('GTFS-JP v4');
    expect(v4).toContain('cannot be represented');
    expect(legacy).toBe('The GTFS-JP fare coverage is missing or incomplete.');
  });

  it('preserves required versus recommended wording in every UI locale', () => {
    const notice = {
      rule_id: 'JPN_008',
      entity_id: 'R1',
      field: 'route_long_name',
      message: 'Türkçe tarafsız çalışma zamanı mesajı',
    };
    const expected = {
      en: ['required', 'recommended'],
      fr: ['exigée', 'recommandée'],
      ja: ['必須', '推奨'],
    } as const;

    for (const [locale, [v3Word, v4Word]] of Object.entries(expected)) {
      expect(tMsgForLocale(locale as keyof typeof expected, {
        ...notice,
        details: { jp_profile: 'v3' },
      })).toContain(v3Word);
      expect(tMsgForLocale(locale as keyof typeof expected, {
        ...notice,
        details: { jp_profile: 'v4' },
      })).toContain(v4Word);
    }
    expect(tMsgForLocale('tr', notice)).toBe(notice.message);
  });
});
