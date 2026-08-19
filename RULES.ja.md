# GTFS Validator & Analyzer — ルール一覧

🇹🇷 [Türkçe](RULES.md) · 🇬🇧 [English](RULES.en.md) · 🇯🇵 **日本語**

598ルール、38グループ。各ルールは一意のID、重要度、クラスで定義されます。
重要度: **致命的**（公開ブロッカー）· **高** · **中** · **低** · **情報**
クラス: **仕様**（GTFS妥当性）· **相互運用**（GTFSインターオペラビリティ）· **品質**（GTFS品質）· **分析**（GTFSアナリティクス）

---

## ARC — アーカイブ / ファイルレベル

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| ARC_001 | ZIPアーカイブを開けません | 致命的 | 仕様 |
| ARC_002 | ファイルをUTF-8で読み込めません | 致命的 | 品質 |
| ARC_003 | オプションファイルにUTF-8エンコードエラー | 中 | 品質 |
| ARC_004 | 必須ファイルが不足 | 致命的 | 仕様 |
| ARC_006 | オプションのGTFSファイルが存在 | 情報 | 品質 |
| ARC_007 | GTFS仕様外のファイル | 情報 | 品質 |
| ARC_008 | カレンダーファイルが不足（calendar.txt・calendar_dates.txtの両方） | 致命的 | 仕様 |
| ARC_031 | translations.txtがあるのにfeed_info.txtがない | 致命的 | 仕様 |
| ARC_009 | データ行なし（空ファイル） | 致命的 | 品質 |
| ARC_010 | ファイルにUTF-8 BOMあり | 中 | 品質 |
| ARC_011 | ファイルサイズ（情報） | 情報 | 分析 |
| ARC_012 | 行の列数がヘッダーと不一致 | 致命的 | 仕様 |
| ARC_013 | CSV解析エラー | 致命的 | 仕様 |
| ARC_014 | ヘッダーフィールドに前後の空白 | 中 | 品質 |
| ARC_015 | ヘッダー列が重複 | 致命的 | 相互運用 |
| ARC_025 | 必須列がヘッダーに存在しない | 致命的 | 仕様 |
| ARC_017 | GTFS仕様外の列名 | 情報 | 品質 |
| ARC_018 | 空データ行 | 中 | 品質 |
| ARC_019 | ヘッダーに空の列名 | 高 | 品質 |
| ARC_020 | 推奨ファイルが不足（shapes.txtまたはfeed_info.txt） | 低 | 品質 |
| ARC_021 | 非印刷可能または問題のある文字 | 低 | 品質 |
| ARC_022 | ファイルの行数が上限（100万行）を超過 | 低 | 品質 |
| ARC_023 | GTFSアーカイブ内のネストされたZIPファイル | 中 | 品質 |
| ARC_024 | サブディレクトリ内のGTFS .txtファイル（読み込み不可） | 中 | 仕様 |
| ARC_026 | 不正な改行文字 | 中 | 仕様 |
| ARC_027 | ZIPエントリにユーザー読み取り権限がない | 情報 | 品質 |
| ARC_028 | GTFS公開URLが.zipファイル名で終わっていない | 低 | 品質 |
| ARC_029 | Zip爆弾保護：アーカイブがzip爆弾の上限を超えました | 致命的 | 品質 |
| ARC_030 | フィールド値にタブまたは改行 | 高 | 仕様 |
| ARC_032 | フィールド値にHTMLマークアップまたは文字参照 | 高 | 仕様 |
| ARC_033 | フィールド値にエスケープされていない引用符（RFC 4180） | 高 | 仕様 |

## BKR — 予約ルール（Booking Rules）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| BKR_001 | 予約ルール: prior_notice_start_dayが禁止コンテキスト | 高 | 仕様 |
| BKR_002 | prior_notice_start_dayにprior_notice_last_dayが必要 | 高 | 仕様 |
| BKR_003 | prior_notice_start_timeにprior_notice_start_dayが必要 | 高 | 仕様 |
| BKR_004 | リアルタイム予約でprior_noticeフィールドが禁止 | 高 | 仕様 |
| BKR_005 | prior_notice_duration_maxはbooking_type=1のみ有効 | 中 | 仕様 |
| BKR_006 | prior_notice_duration_minが無効 | 高 | 仕様 |
| BKR_007 | booking_type=1でprior_notice_duration_minが必須 | 致命的 | 仕様 |
| BKR_008 | booking_type=2でprior_notice_last_dayが必須 | 致命的 | 仕様 |
| BKR_009 | booking_type=2でprior_notice_last_timeが必須 | 致命的 | 仕様 |
| BKR_010 | prior_notice_start_day設定時にprior_notice_start_timeが必須 | 高 | 仕様 |
| BKR_011 | prior_notice_start_day > prior_notice_last_day（無効な予約ウィンドウ） | 高 | 相互運用 |
| BKR_012 | booking_type=2でprior_notice_duration_min禁止 | 中 | 仕様 |
| BKR_013 | prior_notice_last_timeにprior_notice_last_dayが必要 | 高 | 仕様 |
| BKR_014 | prior_notice_service_idはbooking_type=2のみ有効 | 高 | 仕様 |
| BKR_015 | prior_notice_service_idが見つかりません（calendar/calendar_dates） | 致命的 | 仕様 |
| BKR_016 | booking_typeが未設定または無効 | 致命的 | 仕様 |
| BKR_017 | pickup_booking_rule_idが見つかりません（booking_rules） | 致命的 | 仕様 |
| BKR_018 | drop_off_booking_rule_idが見つかりません（booking_rules） | 致命的 | 仕様 |
| BKR_019 | booking_rule_idが未設定または重複 | 致命的 | 仕様 |
| BKR_024 | booking_type=1 かつ duration_max がある場合 prior_notice_start_day は禁止 | 中 | 仕様 |
| BKR_020 | booking_urlが無効 | 中 | 仕様 |
| BKR_021 | info_urlが無効 | 低 | 仕様 |
| BKR_022 | phone_numberが無効 | 低 | 品質 |
| BKR_023 | prior_notice数値項目が整数でない | 中 | 仕様 |
| BKR_025 | prior_notice時刻項目が有効な時刻でない | 中 | 仕様 |

## AGN — 事業者（Agency）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| AGN_002 | agency_nameが不足 | 致命的 | 仕様 |
| AGN_003 | agency_urlが不足または無効 | 致命的 | 仕様 |
| AGN_004 | agency_timezoneが不足または無効 | 致命的 | 仕様 |
| AGN_005 | 事業者間でタイムゾーンが不一致 | 中 | 仕様 |
| AGN_006 | agency_langが無効 | 低 | 仕様 |
| AGN_007 | agency_phoneが無効 | 低 | 品質 |
| AGN_008 | agency_fare_urlが無効 | 低 | 仕様 |
| AGN_009 | agency_emailが無効 | 低 | 仕様 |
| AGN_010 | agency_idが重複 | 致命的 | 仕様 |
| AGN_011 | 複数事業者でagency_idなし | 致命的 | 仕様 |
| AGN_012 | cemv_supportが無効（事業者） | 低 | 仕様 |
| AGN_013 | フィード言語と事業者言語が不一致 | 低 | 相互運用 |
| AGN_014 | 複数事業者だがagency.txtにagency_idなし | 致命的 | 仕様 |
| AGN_015 | agency_urlが安全でないhttp | 情報 | 品質 |
| AGN_016 | agency_phoneが疑わしい/プレースホルダー | 情報 | 品質 |
| AGN_017 | 事業者間で agency_lang が不一致 | 低 | 相互運用 |

## STP — 停留所（Stops）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| STP_001 | stop_idが重複 | 致命的 | 仕様 |
| STP_002 | stop_idが空 | 致命的 | 仕様 |
| STP_003 | stop_nameが不足またはstop_latが範囲外 | 致命的 | 仕様 |
| STP_004 | stop_latが数値でない | 致命的 | 仕様 |
| STP_005 | stop_lonが無効または範囲外 | 致命的 | 仕様 |
| STP_006 | stop_latが不足 | 致命的 | 仕様 |
| STP_007 | stop_lonが不足 | 致命的 | 仕様 |
| STP_008 | location_typeが無効 | 高 | 仕様 |
| STP_009 | parent_stationがstops.txtに存在しない | 致命的 | 仕様 |
| STP_010 | parent_stationのlocation_typeが1でない | 高 | 仕様 |
| STP_011 | このlocation_typeにはparent_stationが必須 | 致命的 | 仕様 |
| STP_012 | stop_timesで使用される停留所のlocation_typeが不適切 | 致命的 | 仕様 |
| STP_013 | wheelchair_boardingが無効 | 低 | 仕様 |
| STP_014 | stop_timezoneが無効 | 中 | 仕様 |
| STP_015 | level_idがlevels.txtに存在しない | 致命的 | 仕様 |
| STP_016 | 同座標の停留所が別に存在 | 中 | 品質 |
| STP_017 | 停留所間が近すぎる | 低 | 品質 |
| STP_018 | 停留所が存在しない | 致命的 | 仕様 |
| STP_019 | stop_nameが長すぎる | 低 | 品質 |
| STP_020 | どの便にも使用されていない停留所 | 中 | 分析 |
| STP_021 | 乗車エリアの親がプラットフォームでない | 高 | 品質 |
| STP_022 | stop_codeが不足 | 中 | 品質 |
| STP_023 | tts_stop_nameが無効 | 低 | 品質 |
| STP_024 | stop_accessがK2互換範囲外 | 情報 | 品質 |
| STP_025 | stop_nameに先頭または末尾の空白 | 中 | 品質 |
| STP_026 | stop_accessが有効な列挙値でない | 低 | 仕様 |
| STP_027 | 経路接続駅でstop_accessが未設定 | 中 | 品質 |
| STP_028 | stop_codeが長すぎる | 情報 | 品質 |
| STP_029 | 停留所が親駅から遠い | 中 | 品質 |
| STP_030 | 子停留所のない駅 | 中 | 品質 |
| STP_031 | stop_nameとstop_descが同一 | 情報 | 品質 |
| STP_032 | 経路接続停留所にparent_stationなし | 中 | 品質 |
| STP_033 | zone_idが不足（運賃計算に必要） | 情報 | 品質 |
| STP_034 | stop_urlが事業者URLと同一 | 低 | 品質 |
| STP_035 | stop_urlが路線URLと同一 | 低 | 品質 |
| STP_036 | 駅（location_type=1）にparent_stationが設定されている | 低 | 仕様 |
| STP_037 | 一部の停留所で車椅子対応情報が未設定 | 中 | 品質 |
| STP_038 | いずれの停留所も車椅子対応情報を報告していない | 情報 | 品質 |
| STP_039 | stop_codeが重複している | 低 | 品質 |
| STP_040 | 停留所名に冗長なstop/station語が含まれる | 低 | 品質 |
| STP_044 | platform_codeに冗長なplatform/track語が含まれる | 低 | 品質 |
| STP_041 | 子停留所名に親駅名が含まれていない | 低 | 品質 |
| STP_042 | stop_urlが無効 | 低 | 仕様 |
| STP_043 | stop_accessが禁止された文脈で使用 | 中 | 仕様 |

## RTS — 路線（Routes）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| RTS_001 | route_idが重複 | 致命的 | 仕様 |
| RTS_002 | agency_idがagency.txtに存在しない | 致命的 | 仕様 |
| RTS_003 | route_short_nameとroute_long_nameの両方が不足 | 致命的 | 仕様 |
| RTS_004 | route_typeが不足または無効 | 致命的 | 仕様 |
| RTS_005 | route_urlが無効 | 中 | 仕様 |
| RTS_006 | route_colorが有効な16進カラーでない | 中 | 仕様 |
| RTS_007 | route_text_colorが有効な16進カラーでない | 中 | 仕様 |
| RTS_008 | route_colorとroute_text_colorのコントラストが不十分 | 中 | 品質 |
| RTS_010 | route_short_nameが長すぎる | 低 | 品質 |
| RTS_011 | route_long_nameが長すぎる | 低 | 品質 |
| RTS_012 | 便のない路線 | 中 | 品質 |
| RTS_013 | continuous_pickupが無効 | 低 | 仕様 |
| RTS_016 | アクティブなサービス日がない路線 | 低 | 品質 |
| RTS_017 | 形状が定義されていない路線 | 情報 | 品質 |
| RTS_018 | continuous_drop_offが無効 | 低 | 仕様 |
| RTS_019 | 路線名が重複 | 中 | 品質 |
| RTS_020 | 路線URLと事業者URLが同一 | 情報 | 品質 |
| RTS_021 | route_short_nameがGoogleトランジットの上限（6文字）を超過 | 低 | 品質 |
| RTS_022 | route_long_nameにroute_short_nameが含まれている | 低 | 品質 |
| RTS_023 | route_descが路線名の繰り返し | 情報 | 品質 |
| RTS_024 | cemv_supportが無効（路線） | 低 | 仕様 |
| RTS_025 | routes.txtのagency_idが空（推奨） | 情報 | 品質 |
| RTS_026 | 短い路線名の重複 | 情報 | 品質 |
| RTS_027 | 長い路線名の重複 | 情報 | 品質 |
| RTS_028 | Flex路線でcontinuous_pickup/drop_off禁止 | 高 | 仕様 |
| RTS_029 | route_sort_orderが無効 | 低 | 仕様 |
| RTS_030 | route_typeがコア列挙型外の拡張値 | 低 | 相互運用 |
| RTS_031 | route_idがありません | 致命的 | 仕様 |

## TRP — 便（Trips）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| TRP_001 | trip_idが重複 | 致命的 | 仕様 |
| TRP_002 | route_idがroutes.txtに存在しない | 致命的 | 仕様 |
| TRP_003 | service_idがカレンダーに存在しない | 致命的 | 仕様 |
| TRP_004 | shape_idがshapes.txtに存在しない | 高 | 仕様 |
| TRP_005 | direction_idが無効 | 中 | 仕様 |
| TRP_006 | wheelchair_accessibleが無効 | 低 | 仕様 |
| TRP_007 | bikes_allowedが無効 | 低 | 仕様 |
| TRP_032 | cars_allowedが無効 | 低 | 仕様 |
| TRP_011 | trip_headsignが未設定 | 高 | 品質 |
| TRP_012 | 双方向路線でdirection_idが不足 | 低 | 品質 |
| TRP_013 | 路線に便が1本のみ | 低 | 品質 |
| TRP_014 | trip_short_nameが長すぎる | 情報 | 品質 |
| TRP_015 | block内で唯一の便 | 低 | 品質 |
| TRP_017 | 頻度ベースの便にstop_timesなし | 中 | 品質 |
| TRP_019 | 連続サービス中にshape_idが必須 | 高 | 仕様 |
| TRP_020 | trip_headsignが中間停留所名と一致 | 情報 | 分析 |
| TRP_021 | bikes_allowed（自転車可否）が未設定 | 情報 | 品質 |
| TRP_022 | block内での便時刻の重複 | 高 | 相互運用 |
| TRP_023 | 今後7日間にアクティブな便なし | 低 | 品質 |
| TRP_024 | block内での路線タイプの不一致 | 低 | 品質 |
| TRP_025 | 車椅子対応情報が未設定の便の割合が高い | 情報 | 品質 |
| TRP_026 | アクティブ日付セットが空の便 | 中 | 分析 |
| TRP_028 | 一部の便で車椅子対応情報が未設定 | 中 | 品質 |
| TRP_029 | すべての便で車椅子対応情報が未報告 | 情報 | 品質 |
| TRP_031 | route_idが不足 | 致命的 | 仕様 |
| TRP_033 | block_idを共有する便が異なるroute_typeを持つ | 中 | 品質 |
| TRP_034 | safe_duration項目が数値でない | 中 | 仕様 |
| TRP_035 | trips.txtのservice_idが空 | 致命的 | 仕様 |

## STM — 停車時刻（Stop Times）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| STM_001 | trip_idがtrips.txtに存在しない | 致命的 | 仕様 |
| STM_002 | stop_idがstops.txtに存在しない | 致命的 | 仕様 |
| STM_003 | arrival_timeの形式エラー | 致命的 | 仕様 |
| STM_004 | departure_timeの形式エラー | 致命的 | 仕様 |
| STM_005 | stop_sequenceが不足または無効 | 致命的 | 仕様 |
| STM_006 | stop_idが不足 | 致命的 | 仕様 |
| STM_007 | 出発時刻が到着時刻より前（departure_time < arrival_time） | 高 | 相互運用 |
| STM_008 | 停留所間で時刻が逆転 | 致命的 | 相互運用 |
| STM_009 | pickup_typeが無効 | 高 | 仕様 |
| STM_010 | drop_off_typeが無効 | 高 | 仕様 |
| STM_012 | 非現実的な速度 | 高 | 相互運用 |
| STM_013 | 時刻カバレッジが混在 | 高 | 品質 |
| STM_014 | セグメントで速度超過 | 高 | 分析 |
| STM_015 | 最初の停留所にarrival_timeがない | 致命的 | 仕様 |
| STM_016 | 最後の停留所にarrival_timeがない | 致命的 | 仕様 |
| STM_017 | stop_timesにshape_dist_traveledなし | 中 | 品質 |
| STM_018 | stop_timesのcontinuous_pickupが無効 | 中 | 仕様 |
| STM_019 | stop_timesのcontinuous_drop_offが無効 | 中 | 仕様 |
| STM_020 | 距離 > 200mでの走行時間ゼロ | 高 | 品質 |
| STM_021 | 異なる停留所が同座標を共有 | 高 | 品質 |
| STM_022 | timepointが無効 | 中 | 仕様 |
| STM_024 | shape_dist_traveled単位の不一致 | 情報 | 品質 |
| STM_025 | 短い区間時刻 | 情報 | 分析 |
| STM_026 | 停留所間距離が超過 | 高 | 品質 |
| STM_028 | 便の総所要時間が上限超過 | 高 | 分析 |
| STM_029 | 便の総所要時間が下限未満 | 中 | 分析 |
| STM_030 | shape_dist_traveledが負または数値でない | 低 | 仕様 |
| STM_032 | 便内でstop_sequenceが重複 | 低 | 仕様 |
| STM_033 | 便に停留所が1つしかなく使用不可 | 高 | 相互運用 |
| STM_034 | arrival_timeとdeparture_timeのいずれか一方のみ定義 | 中 | 相互運用 |
| STM_035 | 同じ停留所に連続して停車（折返し・ループ） | 情報 | 分析 |
| STM_036 | stop_timesがtrip_id + stop_sequenceでソートされていない | 情報 | 品質 |
| STM_037 | Flexの乗降ウィンドウ内に時刻が設定されている | 高 | 仕様 |
| STM_038 | start_pickup_drop_off_window >= end_pickup_drop_off_window | 高 | 相互運用 |
| STM_039 | Flexコンテキストで乗降ウィンドウが不足 | 致命的 | 仕様 |
| STM_040 | Flex stop_timesで予約ルールIDが不足（仕様上は任意） | 中 | 品質 |
| STM_060 | トリップ内でゾーン・時間帯・乗降車が同時に重複 | 高 | 仕様 |
| STM_059 | pickup_type/drop_off_type=2 の場合は booking_rule_id を推奨 | 低 | 品質 |
| STM_041 | stop_idとlocation_id/group_idの同時使用不可 | 高 | 仕様 |
| STM_042 | stop_headsignにGoogleトランジット非対応の文字 | 低 | 相互運用 |
| STM_043 | 便の停留所数が極端に多い（>200） | 情報 | 分析 |
| STM_044 | stop_times行数が200万超（WASMパフォーマンス警告） | 情報 | 分析 |
| STM_045 | 便の出発時刻がサービス日の範囲を超過 | 中 | 品質 |
| STM_046 | trip_idが不足 | 致命的 | 仕様 |
| STM_047 | timepoint=1だが到着/出発時刻なし | 致命的 | 仕様 |
| STM_048 | GTFSサービス日の深夜以降の時刻が00:xx | 高 | 仕様 |
| STM_049 | 同一行の深夜以降の出発が00:xx（GTFS Spec） | 高 | 仕様 |
| STM_050 | timepoint列はあるが値が空 | 低 | 品質 |
| STM_051 | Flexウィンドウでpickup_type 0/3禁止 | 高 | 仕様 |
| STM_052 | Flexウィンドウでdrop_off_type 0禁止 | 高 | 仕様 |
| STM_053 | 多数の連続停留所が同じ時刻 | 中 | 品質 |
| STM_054 | Flexウィンドウでcontinuous_pickup禁止 | 高 | 仕様 |
| STM_055 | Flexウィンドウでcontinuous_drop_off禁止 | 高 | 仕様 |
| STM_056 | shape_dist_traveledが増加していない | 致命的 | 仕様 |
| STM_058 | Flexの受降車ウィンドウ時刻が無効 | 致命的 | 仕様 |

## PDW — 乗降ウィンドウ（Pickup/Drop-off Window）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| PDW_006 | 同一便・ゾーンで乗降ウィンドウが重複 | 中 | 分析 |

## LOC — locations.geojson

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| LOC_001 | locations.geojsonのジオメトリタイプが無効 | 高 | 仕様 |
| LOC_002 | フィーチャーのgeometryがnullまたは欠如 | 致命的 | 仕様 |
| LOC_003 | フィーチャーに'id'プロパティが不足 | 致命的 | 仕様 |
| LOC_004 | Polygonリングが閉じていない | 中 | 仕様 |
| LOC_005 | FeatureCollectionが空 | 低 | 品質 |
| LOC_006 | Polygonの面積が500km²超 | 中 | 品質 |
| LOC_007 | FeatureCollection内でフィーチャー'id'が重複 | 中 | 仕様 |
| LOC_008 | Featureの'type'が欠落または\"Feature\"でない | 中 | 仕様 |
| LOC_009 | Featureの'properties'オブジェクトが欠落 | 中 | 仕様 |
| LOC_011 | 不正なポリゴン: リングが自己交差、または穴が外側にあります | 高 | 仕様 |
| LOC_010 | Geometryの'coordinates'が欠落または配列でない | 致命的 | 仕様 |

## CAL — カレンダー（Calendar）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| CAL_001 | service_idが重複 | 致命的 | 仕様 |
| CAL_002 | 曜日フィールドの値が無効 | 致命的 | 仕様 |
| CAL_003 | start_dateが欠落または無効な形式 | 致命的 | 仕様 |
| CAL_004 | end_dateが欠落または無効な形式 | 致命的 | 仕様 |
| CAL_005 | start_dateがend_dateより後 | 致命的 | 相互運用 |
| CAL_006 | 週次スケジュールで全曜日が無効 | 情報 | 品質 |
| CAL_007 | サービス期間に空白あり | 中 | 分析 |
| CAL_008 | サービス期間が間もなく終了 | 高 | 分析 |
| CAL_009 | フィードの全カレンダー期間が終了 | 致命的 | 品質 |
| CAL_010 | アクティブ日数が少なすぎる | 中 | 分析 |
| CAL_011 | 使用されていないサービス | 低 | 品質 |
| CAL_012 | 近い将来のサービス空白 | 情報 | 分析 |
| CAL_013 | サービス期間が終了（最終日が過去） | 情報 | 分析 |
| CAL_014 | カレンダー日付がfeed_info有効期間外 | 低 | 品質 |
| CAL_015 | フィードの最初のサービス日が将来（本日アクティブな便なし） | 低 | 品質 |
| CAL_016 | サービスが2年以上先まで続く | 情報 | 品質 |
| CAL_017 | カレンダーがまだ開始していない（すべてのアクティブ日が将来） | 低 | 品質 |
| CAL_018 | アクティブな曜日なし（全曜日0、calendar_dates上書きなし） | 低 | 品質 |
| CAL_019 | サービスカレンダー日付がfeed_info有効期間外 | 低 | 品質 |
| CAL_020 | フィード有効期間が5年超 | 低 | 品質 |
| CAL_021 | 本日は有効だが今後数日間運行なし | 情報 | 分析 |
| CAL_022 | service_idが不足 | 致命的 | 仕様 |
| CAL_023 | カレンダーのend_dateが遠い未来（疑わしい） | 中 | 品質 |
| CAL_024 | 今後7日間にアクティブでないカレンダー | 低 | 品質 |
| CAL_025 | カレンダーの曜日フィールドが空 | 致命的 | 仕様 |

## CLD — カレンダー例外（Calendar Dates）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| CLD_001 | service_idが不足 | 致命的 | 仕様 |
| CLD_002 | 日付が欠落または形式が無効 | 致命的 | 仕様 |
| CLD_003 | exception_typeが不足または無効 | 致命的 | 仕様 |
| CLD_004 | calendar_datesのみのサービスにアクティブな日付なし | 高 | 品質 |
| CLD_005 | 日付が合理的な年の範囲外 | 致命的 | 品質 |
| CLD_006 | 例外日が多すぎる | 中 | 品質 |
| CLD_007 | カレンダー例外が多すぎる | 情報 | 分析 |

## SHP — 形状（Shapes）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| SHP_001 | shape_idが不足 | 致命的 | 仕様 |
| SHP_002 | shape_pt_latが不足または無効 | 致命的 | 仕様 |
| SHP_003 | shape_pt_lonが不足または無効 | 致命的 | 仕様 |
| SHP_004 | shape_pt_sequenceが不足または無効 | 致命的 | 仕様 |
| SHP_005 | shape_dist_traveledが減少 | 致命的 | 仕様 |
| SHP_006 | 形状点が1つしかない（最小2点必要） | 低 | 品質 |
| SHP_008 | shape_pt_sequence値が重複 | 致命的 | 仕様 |
| SHP_009 | 形状が自己交差している | 情報 | 分析 |
| SHP_010 | 連続する座標が同一 | 低 | 品質 |
| SHP_012 | 形状が便の停留所から遠すぎる | 高 | 分析 |
| SHP_014 | 最初または最後の停留所が形状端点から遠い | 情報 | 分析 |
| SHP_015 | 形状の点数が統計的に少なすぎる | 中 | 品質 |
| SHP_016 | 形状の方向が便方向と不一致 | 高 | 品質 |
| SHP_017 | 停留所順序が形状順序と矛盾 | 情報 | 分析 |
| SHP_018 | どの便にも参照されていない形状 | 低 | 品質 |
| SHP_019 | この形状を使用する便にstop_timesなし | 中 | 品質 |
| SHP_020 | 形状に重複点 | 情報 | 分析 |
| SHP_021 | shape_dist_traveledが負または数値でない | 低 | 品質 |
| SHP_022 | 形状上の停留所位置が曖昧 | 高 | 品質 |
| SHP_023 | 連続する点が同座標で同一のshape_dist_traveled | 中 | 品質 |
| SHP_024 | shape_dist_traveledと形状距離が不一致 | 中 | 品質 |
| SHP_025 | stop_timesの距離が形状の総長を超過 | 中 | 品質 |
| SHP_026 | 形状の点数が極端に多い（>5,000） | 情報 | 分析 |
| SHP_028 | 同一shape_dist_traveledで座標が異なる | 高 | 仕様 |
| SHP_029 | 同一shape_dist_traveled、座標差が微小 | 情報 | 品質 |
| SHP_030 | stop_timesでshape_dist_traveledを使用しているが、形状距離が不足 | 中 | 品質 |

## FRQ — 頻度（Frequencies）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| FRQ_001 | trip_idがtrips.txtに存在しない | 致命的 | 仕様 |
| FRQ_002 | start_timeが無効 | 致命的 | 仕様 |
| FRQ_003 | end_timeが無効 | 致命的 | 仕様 |
| FRQ_004 | headway_secsが不足または無効 | 致命的 | 仕様 |
| FRQ_005 | end_timeがstart_timeより前 | 致命的 | 品質 |
| FRQ_006 | headway_secsが長すぎる | 中 | 分析 |
| FRQ_007 | exact_timesが無効 | 中 | 仕様 |
| FRQ_008 | headway_secsがゼロ（無効な頻度） | 致命的 | 仕様 |
| FRQ_009 | 頻度間隔が短すぎる | 中 | 品質 |
| FRQ_010 | 運行頻度が非常に高い（詰まりリスク） | 情報 | 分析 |
| FRQ_011 | frequencies 期間の重複 | 高 | 仕様 |
| FRQ_012 | exact_times=1 で end_time が headway の境界と一致 | 低 | 仕様 |

## TRF — 乗換（Transfers）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| TRF_001 | from_stop_idが不足 | 致命的 | 仕様 |
| TRF_002 | to_stop_idが不足 | 致命的 | 仕様 |
| TRF_003 | 指定のIDがstops.txtに存在しない | 致命的 | 仕様 |
| TRF_004 | transfer_typeが無効 | 高 | 仕様 |
| TRF_005 | min_transfer_timeが不足 | 致命的 | 仕様 |
| TRF_006 | from_trip_idがtrips.txtに存在しない | 致命的 | 仕様 |
| TRF_007 | to_trip_idがtrips.txtに存在しない | 致命的 | 仕様 |
| TRF_008 | from_route_idがroutes.txtに存在しない | 致命的 | 仕様 |
| TRF_009 | to_route_idがroutes.txtに存在しない | 致命的 | 仕様 |
| TRF_010 | min_transfer_timeが長すぎる | 中 | 分析 |
| TRF_011 | 停留所は定義されているが歩行距離が非常に長い | 情報 | 品質 |
| TRF_012 | 乗換レコードが重複 | 致命的 | 仕様 |
| TRF_013 | 乗換タイプがコンテキストと不一致 | 致命的 | 品質 |
| TRF_014 | 席内乗換に対応する便なし | 高 | 仕様 |
| TRF_015 | 席内乗換が無効 | 高 | 品質 |
| TRF_016 | 乗換条件が競合 | 致命的 | 仕様 |
| TRF_017 | 乗換が誤った路線を参照 | 高 | 仕様 |
| TRF_018 | from_trip_idとto_trip_idが同一の便 | 中 | 品質 |
| TRF_019 | 席内乗換で異なるroute_type | 中 | 相互運用 |
| TRF_020 | 乗継に必要な歩行速度が速すぎる | 中 | 品質 |
| TRF_021 | 乗換の端点が停留所でも駅でもない | 致命的 | 仕様 |
| TRF_022 | 1-to-n継続で運行カレンダーが矛盾 | 高 | 仕様 |
| TRF_023 | n-to-1継続で運行カレンダーが矛盾 | 高 | 仕様 |

## GGL — Googleトランジット互換性

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| GGL_001 | transfer_type=4/5はGoogleトランジット非対応 | 低 | 相互運用 |
| GGL_002 | ic_priceが無効な値 | 低 | 相互運用 |

## FAR — 運賃属性（Fare Attributes）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| FAR_001 | fare_idが重複 | 致命的 | 仕様 |
| FAR_002 | 運賃が不足または無効 | 致命的 | 仕様 |
| FAR_003 | currency_typeが不足 | 致命的 | 仕様 |
| FAR_004 | payment_methodが無効 | 致命的 | 仕様 |
| FAR_005 | transfersが無効 | 致命的 | 仕様 |
| FAR_006 | transfer_durationが無効 | 中 | 仕様 |
| FAR_008 | agency_idが存在しない | 致命的 | 仕様 |
| FAR_009 | この運賃IDに路線ルールなし | 低 | 品質 |
| FAR_010 | 運賃ルールが重複 | 中 | 品質 |
| FAR_011 | payment_methodが不足 | 致命的 | 仕様 |
| FAR_013 | priceが通貨のISO 4217小数桁数と一致しません | 低 | 仕様 |
| FAR_012 | fare_idが不足 | 致命的 | 仕様 |

## FRL — 運賃ルール（Fare Rules）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| FRL_001 | fare_idがfares.txtに存在しない | 致命的 | 仕様 |
| FRL_002 | route_idがroutes.txtに存在しない | 致命的 | 仕様 |
| FRL_003 | origin_idが無効 | 致命的 | 仕様 |
| FRL_004 | destination_idが無効 | 致命的 | 仕様 |
| FRL_005 | contains_idが無効 | 致命的 | 仕様 |
| FRL_006 | 運賃ルールが定義されていない | 情報 | 品質 |
| FRL_007 | 運賃ルールの論理的不整合 | 中 | 品質 |
| FRL_008 | すべての路線をカバーする運賃が未定義 | 情報 | 品質 |

## RCT — 乗客カテゴリー（Rider Categories, Fares v2）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| RCT_001 | rider_category_idが重複 | 致命的 | 仕様 |
| RCT_002 | rider_category_nameが不足 | 致命的 | 仕様 |
| RCT_003 | is_default_fare_categoryが無効 | 致命的 | 仕様 |
| RCT_004 | min_ageまたはmax_ageが無効（GTFS拡張フィールド — 公式仕様には存在しません） | 中 | 品質 |
| RCT_005 | max_ageがmin_ageより小さい | 中 | 品質 |
| RCT_006 | fare_productのデフォルトrider_category数が1件ではない | 中 | 仕様 |
| RCT_007 | eligibility_urlが無効 | 低 | 仕様 |
| RCT_008 | rider_category_idが空 | 致命的 | 仕様 |

## FMD — 運賃メディア（Fare Media, Fares v2）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| FMD_001 | fare_media_idが重複 | 致命的 | 仕様 |
| FMD_002 | fare_media_typeが不足または無効 | 致命的 | 仕様 |
| FMD_003 | TransitCard/MobileAppにfare_media_nameを推奨 | 低 | 品質 |
| FMD_004 | fare_media_idが空 | 致命的 | 仕様 |

## FPD — 運賃商品（Fare Products, Fares v2）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| FPD_001 | fare_productsの複合主キーが重複 | 致命的 | 仕様 |
| FPD_002 | 金額が不足または負の値 | 致命的 | 仕様 |
| FPD_003 | currencyが有効なISO 4217コードでない | 致命的 | 仕様 |
| FPD_004 | fare_media_idが存在しない | 致命的 | 仕様 |
| FPD_005 | rider_category_idが存在しない | 致命的 | 仕様 |
| FPD_007 | amountが通貨のISO 4217小数桁数と一致しません | 低 | 仕様 |

## FLG — 運賃区間ルール（Fare Leg Rules, Fares v2）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| FLG_001 | fare_product_idが存在しない | 致命的 | 仕様 |
| FLG_002 | network_idが存在しない | 致命的 | 仕様 |
| FLG_003 | from_area_idが存在しない | 致命的 | 仕様 |
| FLG_004 | to_area_idが存在しない | 致命的 | 仕様 |
| FLG_005 | from_timeframe_group_idが存在しない | 致命的 | 仕様 |
| FLG_006 | to_timeframe_group_idが存在しない | 致命的 | 仕様 |
| FLG_007 | rule_priorityが無効 | 中 | 仕様 |
| FLG_008 | fare_product_idがありません | 致命的 | 仕様 |

## FLJ

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| FLJ_001 | from_network_idが未入力または不明 | 致命的 | 仕様 |
| FLJ_002 | to_network_idが未入力または不明 | 致命的 | 仕様 |
| FLJ_003 | from_stop_idが未入力または不明 | 致命的 | 仕様 |
| FLJ_004 | to_stop_idが未入力または不明 | 致命的 | 仕様 |

## FTR — 運賃乗換ルール（Fare Transfer Rules, Fares v2）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| FTR_001 | fare_transfer_typeが不足または無効 | 致命的 | 仕様 |
| FTR_002 | from_leg_group_idが存在しない | 致命的 | 仕様 |
| FTR_003 | to_leg_group_idが存在しない | 致命的 | 仕様 |
| FTR_004 | fare_product_idが存在しない | 致命的 | 仕様 |
| FTR_005 | duration_limit_typeが無効 | 致命的 | 仕様 |
| FTR_006 | duration_limitが無効 | 中 | 仕様 |
| FTR_007 | duration_limitなしでduration_limit_typeが設定されている | 中 | 仕様 |
| FTR_008 | transfer_countが無効 | 中 | 仕様 |
| FTR_009 | leg groupが同じ場合transfer_count必須 | 中 | 仕様 |
| FTR_010 | leg groupが異なる場合transfer_count禁止 | 中 | 仕様 |
| FTR_011 | duration_limit定義済みだがduration_limit_type欠如 | 中 | 仕様 |

## ARS — エリア（Areas, Fares v2）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| ARS_001 | area_idが重複 | 致命的 | 仕様 |
| ARS_002 | areas.txtのarea_idが空 | 致命的 | 仕様 |

## SAR — 停留所エリア（Stop Areas, Fares v2）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| SAR_001 | area_idが存在しない | 致命的 | 仕様 |
| SAR_002 | stop_idが存在しない | 致命的 | 仕様 |
| SAR_003 | stop_areasのarea_idが空 | 致命的 | 仕様 |
| SAR_004 | stop_areasのstop_idが空 | 致命的 | 仕様 |

## NET — ネットワーク（Networks, Fares v2）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| NET_001 | network_idが重複 | 致命的 | 仕様 |
| NET_002 | route_networks.network_idが未入力または不明 | 致命的 | 仕様 |
| NET_003 | route_networks.route_idが未入力または不明 | 致命的 | 仕様 |
| NET_004 | networks.txtのnetwork_idが空 | 致命的 | 仕様 |

## TFR — 時間帯（Timeframes, Fares v2）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| TFR_001 | timeframe_group_idが不足 | 致命的 | 仕様 |
| TFR_002 | service_idが存在しない | 致命的 | 仕様 |
| TFR_003 | start_timeまたはend_timeの形式が無効 | 高 | 仕様 |
| TFR_004 | end_timeがstart_timeより前 | 中 | 品質 |
| TFR_005 | 同一グループ・service_id内で時間範囲が重複 | 中 | 仕様 |
| TFR_006 | start_timeまたはend_timeが24:00:00超 | 致命的 | 仕様 |
| TFR_007 | start_timeとend_timeの一方のみ指定 | 致命的 | 仕様 |
| TFR_008 | timeframesのservice_idが空 | 致命的 | 仕様 |

## PTH — 経路（Pathways）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| PTH_001 | pathway_idが重複 | 致命的 | 仕様 |
| PTH_002 | from_stop_idが存在しない | 致命的 | 仕様 |
| PTH_003 | to_stop_idが存在しない | 致命的 | 仕様 |
| PTH_004 | pathway_modeが不足または無効 | 致命的 | 仕様 |
| PTH_005 | is_bidirectionalが不足 | 致命的 | 仕様 |
| PTH_006 | lengthが無効 | 中 | 仕様 |
| PTH_007 | traversal_timeが無効 | 中 | 仕様 |
| PTH_008 | stair_countが不足 | 低 | 品質 |
| PTH_009 | max_slopeが不足 | 低 | 品質 |
| PTH_010 | min_widthが無効 | 低 | 仕様 |
| PTH_011 | 経路がループを形成 | 高 | 品質 |
| PTH_012 | 駅にアクセシブルな経路なし | 高 | 仕様 |
| PTH_013 | アクセシブル経路の分析 | 情報 | 分析 |
| PTH_014 | 経路が駅の境界を越えている | 致命的 | 品質 |
| PTH_015 | 経路が到達不能な停留所につながる | 中 | 分析 |
| PTH_016 | 出口が双方向として定義されている | 高 | 仕様 |
| PTH_017 | max_slopeが数値でない | 中 | 仕様 |
| PTH_018 | signposted_asが長すぎる | 低 | 品質 |
| PTH_019 | 汎用ノードが1経路にのみ接続（行き止まり） | 中 | 品質 |
| PTH_020 | pathway_idが不足 | 致命的 | 仕様 |
| PTH_021 | from_stop_idが不足 | 致命的 | 仕様 |
| PTH_022 | to_stop_idが不足 | 致命的 | 仕様 |
| PTH_023 | pathway_modeが不足 | 致命的 | 仕様 |
| PTH_024 | is_bidirectionalが不足 | 致命的 | 仕様 |
| PTH_025 | 推奨されるpathway lengthが不足 | 低 | 品質 |
| PTH_030 | 乗降場を持つプラットフォームに通路が割り当てられています | 低 | 仕様 |
| PTH_029 | 推奨されるpathway traversal_timeが不足 | 低 | 品質 |
| PTH_026 | 経路の端点が駅 | 致命的 | 仕様 |
| PTH_031 | 経路の端点が街路から直接アクセスされる停留所(stop_access=1) | 致命的 | 仕様 |
| PTH_027 | stair_countが無効（0または整数でない） | 中 | 仕様 |
| PTH_028 | max_slopeが歩道以外で使用 | 低 | 品質 |

## LVL — 階層（Levels）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| LVL_001 | level_idが重複 | 致命的 | 仕様 |
| LVL_002 | level_indexが無効 | 致命的 | 仕様 |
| LVL_003 | level_nameが不足 | 低 | 品質 |
| LVL_004 | 未使用の階層 | 低 | 品質 |
| LVL_005 | level_nameが長すぎる | 中 | 品質 |
| LVL_006 | エレベーター接続停留所にlevel_idなし | 中 | 品質 |
| LVL_007 | level_indexが不足 | 致命的 | 仕様 |
| LVL_008 | level_idが不足 | 致命的 | 仕様 |

## FIN — フィード情報（Feed Info）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| FIN_001 | feed_publisher_nameが不足 | 致命的 | 仕様 |
| FIN_002 | feed_publisher_urlが不足または無効 | 致命的 | 仕様 |
| FIN_003 | feed_langが不足 | 致命的 | 仕様 |
| FIN_004 | default_langが無効 | 中 | 仕様 |
| FIN_005 | feed_start_dateが無効 | 中 | 仕様 |
| FIN_006 | feed_end_dateが無効または過去の日付 | 高 | 仕様 |
| FIN_007 | feed_versionが不足 | 低 | 品質 |
| FIN_008 | feed_contact_emailが無効 | 低 | 仕様 |
| FIN_009 | feed_contact_urlが無効 | 低 | 仕様 |
| FIN_010 | フィードの有効期限が切れている | 高 | 分析 |
| FIN_012 | feed_start_dateがfeed_end_dateより後 | 低 | 仕様 |
| FIN_013 | fare_attributes.agency_idを推奨するが不足 | 情報 | 品質 |
| FIN_014 | feed_start_date・feed_end_dateの両方が不足 | 低 | 品質 |
| FIN_015 | feed_info.txtのレコードが複数 | 中 | 品質 |
| FIN_016 | feed_start_dateが将来（フィードがまだ有効でない） | 低 | 品質 |
| FIN_017 | フィードが非常に遠い将来に期限切れ | 情報 | 品質 |
| FIN_018 | feed_contact_emailとfeed_contact_urlの両方が不足 | 低 | 品質 |
| FIN_019 | フィードの有効期限まで7日以内 | 低 | 品質 |
| FIN_020 | フィード有効期間が7日未満 | 中 | 品質 |

## TRN — 翻訳（Translations）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| TRN_001 | table_nameが無効な値 | 致命的 | 仕様 |
| TRN_002 | field_nameがこのテーブルで無効 | 致命的 | 仕様 |
| TRN_003 | languageが無効 | 中 | 仕様 |
| TRN_004 | record_idが存在しない | 高 | 仕様 |
| TRN_005 | 翻訳が重複 | 致命的 | 仕様 |
| TRN_006 | 翻訳レコードが競合 | 致命的 | 仕様 |
| TRN_007 | 翻訳がfeed_langと同一言語 | 低 | 品質 |
| TRN_008 | translationの値が空 | 致命的 | 仕様 |
| TRN_009 | record_idとfield_valueを同時使用不可 | 高 | 仕様 |
| TRN_010 | record_sub_idが無効 | 高 | 仕様 |
| TRN_011 | field_nameが翻訳不可 | 高 | 仕様 |
| TRN_013 | feed_info翻訳でIDフィールドを使用不可 | 高 | 仕様 |
| TRN_014 | record_sub_idはstop_timesのみ有効 | 高 | 仕様 |
| TRN_017 | stop_times翻訳にrecord_sub_idがありません（対象行が特定できません） | 中 | 仕様 |
| TRN_015 | record_idとfield_valueが両方空 | 高 | 仕様 |
| TRN_016 | field_valueがどのレコードとも一致しません（翻訳が適用されません） | 中 | 仕様 |

## ATR — 帰属（Attributions）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| ATR_001 | attribution_idが不足 | 高 | 品質 |
| ATR_002 | organization_nameが不足 | 致命的 | 仕様 |
| ATR_003 | ロールが未定義（is_producer・is_operator・is_authority） | 高 | 仕様 |
| ATR_004 | is_producerが無効 | 致命的 | 仕様 |
| ATR_005 | is_operatorが無効 | 致命的 | 仕様 |
| ATR_006 | is_authorityが無効 | 致命的 | 仕様 |
| ATR_007 | attribution_urlが無効 | 致命的 | 仕様 |
| ATR_008 | attribution_emailが無効 | 低 | 仕様 |
| ATR_009 | 帰属参照フィールドが複数設定 | 高 | 仕様 |
| ATR_010 | agency_idが存在しない | 低 | 仕様 |
| ATR_011 | route_idが存在しない | 低 | 仕様 |
| ATR_012 | trip_idが存在しない | 低 | 仕様 |

## XFL — クロスファイル / 意味的整合性

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| XFL_002 | 便にstop_timesレコードなし | 高 | 相互運用 |
| XFL_006 | サービスにキャンセル例外のみ（アクティブ日なし） | 中 | 分析 |
| XFL_011 | カレンダー日付がfeed_info有効期間と不整合 | 中 | 相互運用 |
| XFL_012 | 運行可能な便のない路線 | 高 | 品質 |
| XFL_013 | shape_idが複数の方向で使用されている | 高 | 品質 |
| XFL_014 | 翻訳のソースレコードが存在しない | 中 | 品質 |
| XFL_015 | 帰属の参照が無効 | 致命的 | 仕様 |
| XFL_016 | 翻訳でfeed_infoを参照しているがfeed_info.txtなし | 高 | 仕様 |
| XFL_017 | route_cemv_supportとagency_cemv_supportが競合 | 低 | 品質 |
| XFL_019 | ネットワーク割り当てが2か所に定義されている | 中 | 仕様 |
| XFL_020 | （from_trip_id/to_trip_id, route_id）の組み合わせが無効 | 致命的 | 仕様 |
| XFL_021 | （from_trip_id/to_trip_id, stop_id）の組み合わせが無効 | 高 | 相互運用 |
| XFL_022 | location_group_idが存在しない | 致命的 | 仕様 |
| XFL_023 | stop_idが存在しない（location_group_stops） | 致命的 | 仕様 |
| XFL_024 | location_group_idが存在しない（stop_times） | 致命的 | 仕様 |
| XFL_025 | location_idが存在しない（locations.geojson） | 致命的 | 仕様 |
| XFL_031 | ID衝突：stop_id・locations.geojson id・location_group_idは共通の名前空間を持ちます | 致命的 | 仕様 |
| XFL_032 | location_groups.txtのlocation_group_idが空 | 致命的 | 仕様 |
| XFL_033 | location_group_stopsのlocation_group_idが空 | 致命的 | 仕様 |
| XFL_034 | location_group_stopsのstop_idが空 | 致命的 | 仕様 |
| XFL_026 | 路線cemv=1だが適用可能なcontactless productなし | 中 | 品質 |
| XFL_027 | 路線cemv=2だが適用可能なcontactless productあり | 中 | 品質 |
| XFL_028 | agency cemv=1だがcontactless mediaなし | 情報 | 品質 |
| XFL_029 | route cemv=1だがcontactless mediaなし | 情報 | 品質 |
| XFL_030 | contactless mediaありだがcemv=1なし | 情報 | 品質 |

## OPR — 運行整合性（Operational Consistency）

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| OPR_001 | 路線の運行頻度が低い | 中 | 分析 |
| OPR_003 | 便の詰まり（最小間隔が短すぎる） | 低 | 分析 |
| OPR_004 | 週末運行なし | 情報 | 分析 |
| OPR_005 | 異常な運行頻度 | 情報 | 分析 |
| OPR_006 | 便の停留所数が少なすぎる | 高 | 分析 |
| OPR_007 | 便内の停留所再訪パターン | 情報 | 分析 |
| OPR_008 | 複数セグメントで速度超過 | 高 | 分析 |
| OPR_009 | 深夜出発時刻が遅すぎる | 情報 | 分析 |
| OPR_010 | 路線でアクセシビリティまたは自転車ポリシーが競合 | 中 | 分析 |
| OPR_011 | サービスにアクティブな日なし | 高 | 分析 |
| OPR_012 | サービスの空白 | 中 | 分析 |
| OPR_013 | 路線が一方向のみ運行 | 情報 | 分析 |
| OPR_014 | フィード全体の平均乗換時間が長すぎる | 中 | 分析 |
| OPR_015 | 路線が1つの形状のみで運行 | 情報 | 分析 |
| OPR_016 | フィード全体でアクティブなサービスなし | 情報 | 分析 |
| OPR_017 | 便の距離が非常に短い | 中 | 分析 |
| OPR_019 | 路線で同日に複数のサービスが重複 | 情報 | 分析 |
| OPR_020 | 路線の例外日の重複 | 低 | 分析 |
| OPR_021 | カレンダー上書き競合: 上書きとベースが同時アクティブ | 高 | 分析 |
| OPR_022 | カレンダー上書き未適用: 上書き日にベースサービスが動いている | 高 | 分析 |
| OPR_023 | カレンダー上書き空白: ウィンドウ内にアクティブなサービスなし | 中 | 分析 |
| OPR_024 | 路線の便数が極端に多い | 情報 | 分析 |
| OPR_025 | フィード全体の平均便所要時間が60秒未満 | 高 | 分析 |

## GEO — 地理 / 空間

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| GEO_002 | 停留所がフィードの中央値から非常に遠い | 高 | 分析 |
| GEO_006 | 経路形状に大きなジャンプ | 高 | 分析 |
| GEO_007 | 経路形状に重大なジャンプ（しきい値の3倍） | 高 | 分析 |
| GEO_009 | 停留所が経路形状から遠すぎる | 高 | 品質 |
| GEO_012 | 停留所のクラスター（非常に近い停留所） | 中 | 分析 |
| GEO_013 | フィードの地理的カバレッジ概要 | 情報 | 分析 |
| GEO_014 | フィードの地理的カバレッジが非常に広い | 情報 | 分析 |
| GEO_015 | 停留所座標が日本の範囲外（feed_lang: ja） | 中 | 品質 |
| GEO_016 | 停留所がNull Island付近（\|lat\|<0.1かつ\|lon\|<0.1） | 高 | 品質 |
| GEO_017 | 形状点がNull Island付近 | 高 | 品質 |
| GEO_018 | フィードの全停留所が200m圏内（テストデータの可能性） | 高 | 分析 |
| GEO_019 | 停留所が整数座標（精度ゼロ） | 中 | 品質 |
| GEO_020 | シェープが退化（全点が同一座標） | 高 | 品質 |
| GEO_021 | 停留所の30%以上が座標を共有（系統的エラー） | 高 | 分析 |
| GEO_022 | 停留所の緯度が極に近い（\|lat\| > 89） | 高 | 品質 |

## DQ — データ品質 / ユーザー体験

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| DQ_003 | 路線の説明が不足 | 情報 | 品質 |
| DQ_004 | 路線URLが不足 | 情報 | 品質 |
| DQ_005 | 有効なサービス期間なし | 高 | 品質 |
| DQ_005b | stop_timesのある便なし | 高 | 品質 |
| DQ_005c | 座標なしの停留所の割合が高い | 高 | 品質 |
| DQ_006 | 形状なしの便の割合が高い | 高 | 品質 |
| DQ_009 | すべての便にstop_timesなし | 情報 | 品質 |
| DQ_010 | 事業者がどの路線にも使用されていない | 情報 | 品質 |
| DQ_011 | 停留所が1つしかない | 低 | 品質 |
| DQ_012 | 複数の事業者でagency_idが使用されていない | 低 | 品質 |
| DQ_013 | 便数が少なすぎる | 中 | 品質 |
| DQ_016 | フィールド値に余分な空白 | 中 | 品質 |
| DQ_017 | 疑わしい座標値 | 情報 | 品質 |
| DQ_018 | 推奨フィールドが全大文字 | 中 | 品質 |
| DQ_019 | 推奨フィールドが全小文字 | 中 | 品質 |
| DQ_020 | 推奨フィールドが不足または空 | 低 | 品質 |
| DQ_021 | 主キーが重複 | 高 | 仕様 |
| DQ_022 | 停留所の80%以上が同じstop_name値を共有 | 高 | 品質 |

## VAT — エンティティ分析検出

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| VAT_001 | 路線の形状類似度が高い（重複路線の可能性） | 中 | 分析 |
| VAT_002 | 乗換ポイントが未定義（多くの路線が通過するが乗換なし） | 情報 | 分析 |
| VAT_003 | 便の所要時間が統計的な外れ値 | 低 | 分析 |
| VAT_005 | 孤立した停留所クラスター（ネットワークから切断） | 中 | 分析 |
| VAT_006 | サービス密度の偏り（1路線がフィードの大部分を占有） | 情報 | 分析 |
| VAT_007 | 終点の乗換不足（別路線が終点に来るが乗換未定義） | 情報 | 分析 |
| VAT_008 | 同じシェープがフィード路線の30%以上で使用 | 情報 | 分析 |

## JPN

| ルール | タイトル | 重要度 | クラス |
|---|---|---|---|
| JPN_001 | GTFS-JP：停留所名のかな（ja-Hrkt）読みが欠落 | 中 | 品質 |
| JPN_002 | GTFS-JP：jp_office_id が office_jp.txt に未定義 | 高 | 相互運用 |
| JPN_003 | GTFS-JP：agency_jp の agency_id が agency.txt に未定義 | 高 | 相互運用 |
| JPN_004 | GTFS-JP：translations.txt が欠如 | 高 | 相互運用 |
| JPN_005 | GTFS-JP：office_jp の office_name が空 | 高 | 相互運用 |
| JPN_006 | GTFS-JP：運賃ファイルが欠如 | 中 | 品質 |
| JPN_007 | GTFS-JP：feed_info.txt が欠如 | 中 | 品質 |
| JPN_008 | GTFS-JP：route_long_name のかな読みが欠如 | 中 | 品質 |
| JPN_009 | GTFS-JP：trip_headsign のかな読みが欠如 | 中 | 品質 |
| JPN_010 | GTFS-JP：agency_name のかな読みが欠如 | 中 | 品質 |
| JPN_011 | GTFS-JP：事業者が1つでもagency_idが必須 | 高 | 相互運用 |
