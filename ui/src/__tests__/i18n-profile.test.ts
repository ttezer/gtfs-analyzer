import { describe, expect, it } from 'vitest';
import { tMsgForLocale, tRemediationForLocale } from '../i18n';

describe('GTFS-JP profile-specific notice messages', () => {
  it('translates STM_047 by the independently missing time field', () => {
    const notice = {
      rule_id: 'STM_047', entity_id: 'T1', message: 'runtime', remediation: 'runtime',
      details: { message_variant: 'missing_departure' },
    };
    expect(tMsgForLocale('en', notice)).toBe(
      "Trip 'T1': timepoint=1 (exact time point) but departure_time is missing.",
    );
    expect(tMsgForLocale('ja', notice)).toContain('departure_timeがありません');
    expect(tMsgForLocale('fr', notice)).toContain('departure_time est absent');
    expect(tRemediationForLocale('en', notice)).toContain('Provide departure_time');
    expect(tRemediationForLocale('ja', notice)).toContain('departure_timeを入力');
    expect(tRemediationForLocale('fr', notice)).toContain('Renseignez departure_time');
  });

  it('keeps aggregate counts and manual review in every translated locale', () => {
    for (const locale of ['en', 'fr', 'ja'] as const) {
      const aggregate = tMsgForLocale(locale, {
        rule_id: 'JPN_029', field: 'stop_headsign', message: 'runtime',
        details: { jp_profile: 'V4', message_variant: 'aggregate',
          table_name: 'stop_times', source_value: '渋谷', affected_records: '18426',
          example_record_ids: 'trip_id=T1,stop_sequence=2' },
      });
      for (const part of ['18426', '渋谷', 'stop_times.stop_headsign', 'trip_id=T1,stop_sequence=2']) {
        expect(aggregate).toContain(part);
      }
      const review = { rule_id: 'JPN_006', message: 'runtime', remediation: 'runtime',
        details: { jp_profile: 'V4', message_variant: 'missing_review' } };
      expect(tMsgForLocale(locale, review)).not.toBe(tMsgForLocale(locale, { ...review, details: { jp_profile: 'V4' } }));
      expect(tRemediationForLocale(locale, review)).not.toBe(tRemediationForLocale(locale, { ...review, details: {} }));
    }
  });
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
