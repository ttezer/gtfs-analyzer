# GTFS Validator & Analyzer

🇹🇷 [Türkçe](README.md) · 🇬🇧 [English](README.en.md) · 🇯🇵 **日本語** · 🇫🇷 [Français](README.fr.md)

[![アプリを開く](https://img.shields.io/badge/%E3%82%A2%E3%83%97%E3%83%AA%E3%82%92%E9%96%8B%E3%81%8F-gtfs--analyzer-2ea44f?style=flat&logo=googlechrome&logoColor=white)](https://ttezer.github.io/gtfs-analyzer/)
[![GTFS-JP](https://img.shields.io/badge/GTFS--JP-v3%2Fv4%20%E5%AF%BE%E5%BF%9C-c8102e?style=flat)](https://www.gtfs.jp/)
[![ルール数](https://img.shields.io/badge/rules-612-blue?style=flat)](RULES.ja.md)
![GTFS Spec カバレッジ](https://img.shields.io/badge/GTFS%20Spec-97.2%25-007ec6?style=flat)
[![コーパス検証](https://img.shields.io/badge/corpus-4%2C318%20feeds%20%C3%97%2012%20runs-brightgreen?style=flat)](audit-results/)
[![crates.io](https://img.shields.io/crates/v/gtfs-analyzer?style=flat&label=crates.io)](https://crates.io/crates/gtfs-analyzer)
[![npm](https://img.shields.io/npm/v/gtfs-sdk?style=flat&label=npm)](https://www.npmjs.com/package/gtfs-sdk)
[![License MIT](https://img.shields.io/badge/%E3%83%A9%E3%82%A4%E3%82%BB%E3%83%B3%E3%82%B9-MIT-yellow?style=flat)](LICENSE)

GTFS Validator & Analyzer は、ブラウザ上で動作するオープンソースの GTFS バリデーター兼フィード品質分析ツールです。アップロードされた .zip ファイルはいかなるサーバーにも送信されず、すべての処理は WebAssembly によってユーザーのデバイス上で実行されます。ブラウザ、CLI（`cargo install gtfs-analyzer`）、Rust ライブラリ、CI/CD ゲート、`gtfs-sdk` npm パッケージの 5 つの方法で利用できます。

測定可能な GTFS 仕様要件の **97.2%** をカバーし、フィールドインベントリの 300 個のアトムすべてを少なくとも 1 つの Spec ルールにアンカーしています。**612 個の検証ルール**のうち **417 個**が直近の 4,318 フィード完全カタログ実行で少なくとも 1 件の指摘を出しました。GTFS-JP の追加ルールは別途 585 フィードのプロファイル実行で測定しています。すべてのルールは [`RULES.ja.md`](RULES.ja.md) に一覧化されています。

MobilityData の公式 `gtfs-validator` に対して **12 回の完全なカタログ実行**で精度を検証しました。各実行ではカタログ内のテスト可能な全 GTFS Schedule フィード（直近の実行で **4,318 件**）を同じマシン・同じ日付で両方のバリデーターにかけ、MobilityData 側では実際の Java `gtfs-validator v8.0.1` を使用しています。生データは [`audit-results/`](audit-results/) にあります。

GTFS Validator & Analyzer は、ファイルが仕様に準拠しているかどうかをチェックするだけではありません。フィードがどれだけ信頼でき、一貫性があり、利用可能であるかも分析します。エラーを該当するファイルと行番号とともに表示し、各検出結果に対する修正手順を提示し、地理的な問題 — 例えば逸脱した経路、壊れた座標、到達不能な停留所など — をインタラクティブな地図上にマーキングします。

すべての検出結果には、ルールコード、分析クラス、重大度レベルが付与されます。仕様・相互運用・品質・分析 のクラスと 致命的 → 情報 の重大度レベルにより、数千件の検出結果をフィルタリングし、優先順位付けし、体系的に処理できます。また本ツールは、フィードが使用している GTFS 機能 — Shapes、Transfers、Fares、Headsigns、Flex など — を自動的に検出してレポートに含めます。

GTFS Validator & Analyzer は、仕様検証を運用品質分析へと拡張します。路線ごとの運行頻度の不整合、異常な速度区間、孤立した停留所、サービスパターンの欠落、ネットワークトポロジーの問題を、612 個の異なる検証・分析ルールで精査します。結果は、準拠性と品質を別々に評価するスコアで要約されます。優先順位付けされた修正キューは、どの問題を最初に対処すべきか、および各修正がスコアに与える可能性のある影響を示します。

**対象ユーザー**

- **交通事業者・自治体** — フィードを公開する前に検証し、品質上の問題を解消するため。
- **GTFS インテグレーター・コンサルタント** — 納品データの技術的・運用的な品質を文書化するため。
- **アプリケーション開発者** — 利用するフィードの信頼性と統合リスクを評価するため。
- **研究者・アナリスト** — 異なる交通ネットワークをデータ品質と構造の観点で比較するため。

---

## 他のツールとの比較

### 機能比較表

| 機能 | MobilityData | GTFS Analyzer |
|---|:---:|:---:|
| Web インターフェース | ✅ | ✅ |
| データがブラウザから出ない | ❌ | ✅ |
| 仕様準拠ルール | ✅ | ✅ |
| 品質ルール | ❌ | ✅ |
| 運用アナリティクス | ❌ | ✅ |
| 地図の可視化 | ❌ | 停留所・経路・便・路線・通路 |
| フィードスコア | ❌ | ✅ |
| 修正ガイダンス | 一部 | ✅ |
| GTFS Flex サポート | 一部 | ✅ |
| Fares v2 検証 | 部分的 | ✅ |
| GTFS-JP プロファイル検証 | ❌ | ✅ |
| 出力形式 | HTML, JSON | HTML, CSV, JSON, PDF |
| 配布形態 | Web · デスクトップインストーラー（msi/dmg/deb）· CLI JAR · Docker | Web · CLI バイナリ · `cargo install` · npm SDK |
| 文書化された CI/CD 統合 | README に記載なし（Docker/CLI で可能） | ✅ `--fail-on` + 終了コード |
| npm パッケージ | ❌ | ✅ `gtfs-sdk` |
| crates.io パッケージ | — *（Java プロジェクト）* | ✅ `gtfs-analyzer` |
| GTFS Spec カバレッジ（測定値） | — | **97.2%** · 300/300 フィールドアンカー |
| **総ルール数** | **178** | **612** |

### コーパス検証

数件のフィードでは正確性を示せません。すべてのリリースは **MobilityDatabase の GTFS Schedule カタログ全体**に対して実行されます — 直近の実行で **4,318 フィード**、640 並列シャード。比較対象は MobilityData の **`gtfs-validator` v8.0.1** で、公開済みレポートを読むのではなく**同じアーカイブ上で再実行**します。したがって差分は「どちらが何を検出したか」であり、「どちらのレポートがいつ生成されたか」ではありません。

直近の実行（`32587015142`、4,275 フィードで両者とも正常完了）:

| | GTFS Analyzer | MobilityData |
|---|---|---|
| 実行時間の中央値 | **0.05 秒** | 3.00 秒 |
| ピークメモリの中央値 | **14 MB** | 329 MB |
| 完了できなかったフィード | **1** | 10 |
| MobilityData が検出し当方が見逃した事実 | **0 件** | — |

生データは [`audit-results/`](audit-results/) にあります — 最初の 7 回はリポジトリ内、以降は `audit-<run-id>` プレリリースとして保存されます。

### フィード分析の例

以下の数値は最新のコーパス実行からのものです。同じアーカイブを同じ分析日（2026-08-20）に使用し、MobilityData 側では Java `gtfs-validator v8.0.1` を実行しました。

#### BART（ベイエリア高速鉄道、サンフランシスコ）

フィード：`mdb-53` · 14 路線、287 停留所、4,417 便 · 0.9 MB。

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| 総通知数 | 2,715 | 740 |
| 致命的 / エラー | 2 | 2 |
| 高 / 警告 | 2,654 | 1 |
| 中 | — | 11 |
| 低 | — | 24 |
| 情報 | 59 | 702 |
| 発動した個別ルール種別数 | 13 | **37** |
| 検証時間 | 3.43 秒 | **0.19 秒** |
| 公開スコア | — | **92.6 / 100** |
| 総合スコア | — | **90.9 / 100** |

#### TriMet（ポートランド、オレゴン）

フィード：`mdb-247` · 112 路線、6,480 停留所、70,557 便 · 28.4 MB。

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| 総通知数 | 51 | 3,099 |
| 致命的 / エラー | 0 | 0 |
| 高 / 警告 | 38 | 12 |
| 中 | — | 97 |
| 低 | — | 497 |
| 情報 | 13 | 2,493 |
| 発動した個別ルール種別数 | 8 | **49** |
| 検証時間 | 14.85 秒 | **5.46 秒** |
| 公開スコア | — | **100 / 100** |
| 総合スコア | — | **90.0 / 100** |

> 仕様的にクリーンなフィードです。両ツールとも Critical は 0 件で、Publish Score は 100 です。ルール数の差は、GTFS Analyzer の追加の運用品質分析を反映しています。

#### Tokyo Toei（東京都交通局）

フィード：`mdb-3175` · 151 路線、5,370 停留所、68,817 便 · 8.6 MB · **GTFS-JP プロファイル**。

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| 総通知数 | 1,849 | 1,741 |
| 致命的 / エラー | 0 | 0 |
| 高 / 警告 | 268 | 12 |
| 中 | — | 809 |
| 低 | — | 548 |
| 情報 | 1,581 | 372 |
| 発動した個別ルール種別数 | 8 | **49** |
| 検証時間 | 5.94 秒 | **1.75 秒** |
| 公開スコア | — | **100 / 100** |
| 総合スコア | — | **87.2 / 100** |

> この実際の日本語フィードでは GTFS-JP プロファイルが誤検出を生みません。仕様的にクリーン（Critical 0、Publish Score 100）で、プロファイルルールは日本固有の要件だけを検査します。

#### VBB（ベルリン・ブランデンブルク運輸連合）

フィード：`mdb-782` · 1,274 路線、41,961 停留所、258,524 便、14,485 シェイプ · **約 75 MB**。

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| 総通知数 | 12,201 | 25,369 |
| 致命的 / エラー | 0 | 0 |
| 高 / 警告 | 11,486 | 1,307 |
| 中 | — | 7,440 |
| 低 | — | 8,186 |
| 情報 | 715 | 8,436 |
| 発動した個別ルール種別数 | 18 | **91** |
| 検証時間 | 45.16 秒 | **21.07 秒** |
| 公開スコア | — | **100 / 100** |
| 総合スコア | — | **78.4 / 100** |

> 🇩🇪 **大規模フィード：** MobilityData のホスト型 Web バリデーターではこのサイズを処理できませんが、GTFS Analyzer はファイルをサーバーに送信せずブラウザで直接検証できます。MobilityData の通知の半分以上は有効なドイツ語の ü/ö/ä/ß 文字に由来し、GTFS Analyzer は有効な Unicode 文字を問題として扱いません。中核的なチェックは一致しています。

---

## GTFS-JP 対応

GTFS Analyzer は、日本の国内 GTFS プロファイルである **GTFS-JP**（国土交通省 / MLIT 標準）を自動的に認識し、標準 GTFS では任意とされている項目のうち GTFS-JP が必須とする要件を検証します。MLIT は補助を受ける事業者に GTFS-JP の公開を求めているため、数百の中小事業者がこのプロファイルへの準拠を必要としますが、一般的なバリデーターはプロファイル固有の要件を検査しません。

**自動検出。** フィードに現行 GTFS-JP ファイル（`agency_jp.txt`、`office_jp.txt`、`pattern_jp.txt`）または旧版互換の `routes_jp.txt` が含まれる場合、`feed_lang` が `ja` で始まる場合、または `translations.txt` にかな（`ja-Hrkt`）の読みが含まれる場合、そのフィードは GTFS-JP として判定され、レポートに **GTFS-JP** バッジが表示されます。`routes_jp.txt` は v3 のファイルではなく、旧版フィード互換のためにのみ認識されます。プロファイルルールはこれらのフィードでのみ有効になり、標準フィードでは作動しません。

**解析プロファイルの選択。** Web アプリでは ZIP を選ぶ前に **分析設定** パネルを開き、**GTFS-JP 検証プロファイル**で `Auto`、`V3`、または `V4` を選択してください。フィードを選択すると現在の選択が保存され、そのまま自動解析が始まります。デフォルトは `Auto` です。CLI では `--gtfs-jp-profile v3` または `--gtfs-jp-profile v4`、SDK では `config: { gtfs_jp_profile: 'v3' }` または `'v4'` を指定します。これは検証範囲の選択であり、フィードの公式 GTFS-JP バージョンを自動判定するものではありません。詳細は [GTFS-JP v3/v4 互換性マトリクス](docs/gtfs-jp-v3-v4-matrix.md) を参照してください。

**プロファイルルール（JPN グループ）。**

| ルール | 検査内容 |
|---|---|
| **JPN_001** | 停留所名のかな（よみがな — `translations.txt`、`ja-Hrkt`）読み。音声案内・検索のため GTFS-JP で必須 |
| **JPN_002** | `jp_office_id`（`trips.txt` **または** `routes.txt`）が `office_jp.txt` の `office_id` と一致すること（営業所参照整合性） |
| **JPN_003** | `agency_jp.txt` の `agency_id` が `agency.txt` に定義されていること（事業者参照整合性） |
| **JPN_004** | `translations.txt` の存在 — GTFS-JP では（特にかな読みのため）必須 |
| **JPN_005** | `office_jp.txt` の必須項目 `office_name` が入力されていること |
| **JPN_006** | `fare_attributes.txt` は必須、運賃プロファイルが異なる場合は `fare_rules.txt` が条件付き必須 |
| **JPN_007** | `feed_info.txt` の存在 — GTFS-JP では必須 |
| **JPN_008** | 路線名（`route_long_name`）のかな（`ja-Hrkt`）読み |
| **JPN_009** | `trip_headsign` のかな（`ja-Hrkt`）読み |
| **JPN_010** | 事業者名（`agency_name`）のかな（`ja-Hrkt`）読み |
| **JPN_011** | 事業者が1つだけでも `agency_id` を必須とする |
| **JPN_012** | `agency_jp.agency_id` が必須で、`agency.txt` の行を参照すること |
| **JPN_013** | 存在する場合、`agency_zip_number` は7桁のASCII数字であること |
| **JPN_014** | `office_jp.office_id` が存在し、一意であること |
| **JPN_015** | 旧版 `routes_jp.route_id` の互換性チェック（v3 ファイルではありません） |
| **JPN_016** | `pattern_jp.route_update_date` と旧版 `routes_jp.route_update_date` が有効な `YYYYMMDD` 日付であること |
| **JPN_017** | `pattern_jp.jp_pattern_id` が存在し、一意であること |
| **JPN_018** | `pattern_jp.txt` が存在する場合、`trips.jp_pattern_id` が同ファイルを参照すること |
| **JPN_019** | `ja-Hrkt` 行が有効なGTFSテーブル・フィールド・レコード・stop_timesサブレコードを使うこと |
| **JPN_020** | `office_url` と `office_phone` の基本的な形式を品質チェックすること |
| **JPN_021** | `ja-Hrkt` 翻訳が空でなく、一貫し、日本語表記を含むこと |
| **JPN_022** | GTFS-JP v4 の `agency_lang`、`feed_start_date`、`feed_end_date`、`feed_version` 必須項目の欠落 |

上記の **Tokyo Toei** の比較は、実際の GTFS-JP フィードでこのプロファイルがどう振る舞うかを示しています。フィードは仕様的にクリーン（重大 0 件）であり、正しく参照されたデータではプロファイルルールが誤検出を生みません。

---

## 使い方

GTFS Analyzer は Web アプリケーションです — インストール不要です。ブラウザでライブ版を開き、GTFS の zip ファイルをアップロードしてください。

実行エンジンはブラウザの機能に応じて自動選択されます。Memory64 対応環境では
4 GB を超える大規模フィード向けに **WASM64**、それ以外では **WASM32** を使用します。
使用中のエンジンは画面に表示されます。診断用に `?wasm32=1`、`?wasm64=1`、`?serial=1` を使用できます。

**→ [https://ttezer.github.io/gtfs-analyzer/](https://ttezer.github.io/gtfs-analyzer/)**

1. GTFS の zip ファイルをドラッグ＆ドロップするか、ファイル選択ボタンを使用してください。
2. 検証が自動的に開始され、進行状況が画面上にステップごとに表示されます。
3. 完了すると、公開スコアと総合スコア、および詳細レポートのタブが表示されます。
4. 以前の解析と比較するには、**比較**タブで旧 Golden JSON をアップロードします。修正済み・新規・減少・増加ルールに加え、スコア、フィード期間、正規化した通知密度の変化を表示します。
5. 共有可能な成果物を作成するには、**エクスポート → エグゼクティブPDFレポート**を開き、レポート言語を選択して、プレビューの**印刷 / PDF保存**を使用します。

### エグゼクティブPDFレポート

**エグゼクティブPDFレポート**は、詳細な検証結果を意思決定者やフィード作成者向けの、読みやすく色分けされたA4対応文書に変換します。レポートは **GTFS Analyzer** の結果だけから生成され、他のバリデーターの結果や外部比較は含みません。

レポートには次の内容が含まれます：

- 公開可否、公開スコア、総合スコア、および仕様・相互運用性・品質・分析の各スコア；
- 停留所、路線、便、シェープ、運行日数、日付範囲をまとめたフィード概要；
- R1の公開ブロッカーとR9の影響度・工数順位を統合し、ルール単位で重複を除いた **P0 / P1 / P2** アクション；
- 各優先指摘の根拠、影響、推奨修正、実際の影響件数、見込まれるスコア改善；
- フィード固有の構造的な洞察、段階的な改善計画、重要度・クラス分布、技術付録。

画面表示では性能確保のため指摘例の件数が制限される場合でも、レポートは利用可能な場合に `capped_totals` の**実集計件数**を使用します。文書はUI言語とは独立して、トルコ語・英語・日本語で生成できます。生成と印刷はすべてブラウザ内で行われ、GTFSデータはサーバーに送信されず、外部APIも必要ありません。

> レポートのスコアはアップロードされたGTFSフィードを評価するものであり、GTFS Analyzer自体の性能や精度を評価するものではありません。

> セルフホスティングや開発環境の構築については、[開発者セットアップ](#開発者セットアップ)をご参照ください。

---

## 5 つの利用方法

同じ検証コア（`gtfs_pipeline::validate_bytes`）を 5 つの方法で実行できます。すべて同じ 612 ルールと同じ結果モデルを使用します。

| 方法 | 用途 | データの送信先 |
|---|---|---|
| **ブラウザ** ([アプリ](https://ttezer.github.io/gtfs-analyzer/)) | 1 つのフィードを地図とレポートで確認 | **どこにも送信しない** — デバイス上の WebAssembly |
| **CLI**（`cargo install gtfs-analyzer` またはビルド済みバイナリ） | 一括検証、スクリプト、Python 連携 | どこにも送信しない — ローカルバイナリ |
| **Rust ライブラリ**（[`gtfs-pipeline`](https://crates.io/crates/gtfs-pipeline)） | 自分の Rust サービスへの組み込み | どこにも送信しない — 自分のプロセス |
| **CI/CD**（終了コード + `--fail-on`） | フィード公開前のパイプラインゲート | どこにも送信しない — 自分の runner |
| **[`gtfs-sdk`](https://www.npmjs.com/package/gtfs-sdk) npm パッケージ** | Web または Node アプリへの組み込み | どこにも送信しない — ローカル WASM |

どの方法でもフィードがサーバーにアップロードされることはありません。組織のポリシーや契約上、外部に出せないデータにも利用できます。

### CI/CD 統合

`--fail-on` オプションで、指定した重大度またはクラスだけをパイプライン失敗条件にできます。Analytics の指摘でリリース全体が失敗することを防げます。

```yaml
# GitHub Actions — 公式 GTFS Spec 違反のみで失敗させる
- name: GTFS フィードを検証
  run: |
    curl -sL https://github.com/ttezer/gtfs-analyzer/releases/latest/download/gtfs-analyzer-x86_64-linux.tar.gz | tar xz
    ./gtfs-analyzer validate feed.zip --fail-on-class spec --min-severity critical
```

終了コード：`0` クリーン · `1` 条件に一致する指摘あり · `2` フィード / config / ファイルエラー。

### Rust ライブラリ

自分の Rust サービスに検証を組み込む場合は `gtfs-pipeline` を直接使用します。CLI もファイルシステムもネットワークも不要です:

```toml
[dependencies]
gtfs-pipeline = "0.12.0"
gtfs-config   = "0.12.0"
gtfs-core     = "0.12.0"
```

```rust
use gtfs_config::ValidatorConfig;
use gtfs_core::ValidateResult;
use gtfs_pipeline::validate_bytes;

let zip = std::fs::read("feed.zip")?;
let config = ValidatorConfig::default();

match validate_bytes(&zip, &config, 20_260_820) {
    ValidateResult::Ok(result) => {
        println!("指摘: {}", result.notices.len());
        println!("公開スコア: {}", result.reports.r5.pub_score);
    }
    ValidateResult::Fatal(err) => eprintln!("fatal: {}", err.message),
}
```

`validate_bytes` はバイト列を受け取り、すべてのレポート（`r1`–`r9`）、スコア、指摘を含む結果を返します。しきい値を変更するには `ValidatorConfig` のフィールドを調整するか、`merge_delta` で JSON デルタを適用します。

⚠️ ライブラリ crate はアナライザーの**内部実装**です。バイナリをレジストリからビルドできるように公開されているだけで、**API の安定性は保証されません**。安定したインターフェースが必要な場合は、CLI の JSON 出力または `gtfs-sdk` を使用してください。

### `gtfs-sdk` npm パッケージ

`gtfs-sdk` は v0.12.0 の検証エンジンを型付き JavaScript/TypeScript API として提供します。フィードはローカル WASM で検証され、アプリケーションの外に出ません。

```js
import { validateGtfs } from "gtfs-sdk";

const result = await validateGtfs(new Uint8Array(zipBytes), {
  today: "2026-08-20",
});
console.log(result.notices.length, result.reports.r5.score);
```

公開 API には `validateGtfs`、`getVersion`、および進捗・キャッシュイベントが必要なアプリ向けの `createValidatorSession` が含まれます。低レベルの `gtfs-wasm` binding は SDK 契約の一部ではなく、WASM64 と threaded engine の選択は最初の SDK パッケージでは内部実装です。

パッケージのソースは `sdk/` にあります。詳細な使い方、結果モデル、config リファレンスは [`sdk/README.md`](sdk/README.md) を参照してください。WASM binding はビルド時に `crates/wasm` から生成されます。

---

## CLI（ターミナル）

Web UI に加えて、同じ検証コア（`gtfs_pipeline::validate_bytes`）をターミナルから実行できます — Python/自動化連携向け。

### インストール

Rust がインストールされている場合、最短の方法：

```bash
cargo install gtfs-analyzer
gtfs-analyzer validate feed.zip
```

Rust をインストールせずに使う場合：[Releases](https://github.com/ttezer/gtfs-analyzer/releases) からお使いのプラットフォーム向けアーカイブ（`x86_64-linux`、`aarch64-macos`、`x86_64-windows`）をダウンロードし、展開して `gtfs-analyzer` バイナリを `PATH` に配置します。

```bash
# Linux / macOS — 最新リリース
curl -sL https://github.com/ttezer/gtfs-analyzer/releases/latest/download/gtfs-analyzer-x86_64-linux.tar.gz | tar xz
./gtfs-analyzer --version
```

ソースからビルドする場合：

```bash
cargo build --release -p gtfs-analyzer
target/release/gtfs-analyzer validate feed.zip --json

# または直接
cargo run -p gtfs-analyzer -- validate feed.zip --json
```

### `validate` — フィード検証

| フラグ | 説明 |
|---|---|
| `--json` | 結果全体を JSON として出力 |
| `--summary` | 短い要約：ステータス、通知数、スコア（デフォルト。`--json` とは併用不可） |
| `--rule SHP_010` | 指定ルールの通知のみ |
| `--severity critical` | この重大度に完全一致する通知のみ（critical/high/medium/low/info） |
| `--min-severity high` | この重大度以上（critical が最も重い） |
| `--class spec` | 指定したルールクラスのみ — `spec,interop,quality,analytics`、カンマ区切りで複数可 |
| `--fail-on critical` | この重大度以上が存在する場合**のみ** exit 1 |
| `--fail-on-class spec` | 指定クラスの通知が存在する場合のみ exit 1 |
| `--pretty` | JSON をインデント出力（`--json` が必要） |
| `--include-name-index` | `name_index`（停留所/路線/shape の参照表）を JSON に含める |
| `-o report.json` | stdout ではなくファイルに出力 |
| `--lang en` | 指摘テキストの言語：`en`（デフォルト）/ `tr` / `ja` / `fr` |
| `--config config.json` | JSON config デルタを適用（`ValidatorConfig::default()` の上に） |
| `--today 20260710` | 解析の「今日」を固定（カレンダールール用） |

**フィルタは表示を絞るだけです。** `notices` と R2–R9 のリストはフィルタされますが、**R1 の公開可否判定と R5 のスコアは常にフィード全体**を表します。フィルタ適用時は JSON に `filtered` フィールド、要約に `filter:` 行が追加されます。

`name_index` は**デフォルトでは出力されません**：大規模フィードでは停留所/shape の座標表がペイロードの大半を占めるためです。必要な場合は `--include-name-index` を指定してください。

パスの代わりに `-` を渡すと ZIP を **stdin** から読み込みます：`curl -sL <url> | gtfs-analyzer validate - --json`。（ZIP の中央ディレクトリはファイル末尾にあるため、アーカイブはストリーミングではなくメモリに読み込まれます。）

> **Web UI とは件数が異なります。** ブラウザは性能のためルールごとの指摘件数に上限を設けます（実際の合計は `capped_totals` で報告されます）。CLI はこの上限を**適用しません** — 同じフィードでもより多くの notice と、スケーリングされていない R9 影響値が返ります。これは想定どおりの差異であり、両者の件数を直接比較しないでください。

**終了コード：** `0` 通知なし · `1` 通知ありまたは `PARTIAL` レポート · `2` 致命的エラーまたは config/ファイルエラー。`PARTIAL` レポートでは利用できない入力を安全にスキップし、独立した検査を続行します。JSON には `status: "partial"`、`validation_status: "PARTIAL"`、`partial` の範囲が含まれます。`partial.skipped_checks` は前提条件が利用できず実行されなかった K4/K5/K6 の検査ファミリーと個別ルールを示し、`partial.skipped_stages` は大まかなステージメタデータのために保持されます。`--fail-on*` を指定した場合、`1` は一致する通知があるときのみ返り、その他の指摘は報告されますが実行を失敗させません。JSON モードでは stdout は JSON のみ、エラーは stderr。

```bash
# CI ゲート：公式 GTFS Spec 違反のみで失敗させる
gtfs-analyzer validate feed.zip --fail-on-class spec

# Spec の指摘のみを出力（スコアはフィード全体を表す）
gtfs-analyzer validate feed.zip --class spec --json --pretty -o spec.json
```

```python
import json, subprocess

proc = subprocess.run(
    ["target/release/gtfs-analyzer", "validate", "feed.zip", "--json"],
    text=True, capture_output=True,
)
# exit 1 は「通知あり」であり失敗ではない — check=True は使わない
data = json.loads(proc.stdout)
if data["status"] == "fatal":
    raise SystemExit(f'{data["code"]}: {data["message"]}')
for n in data["notices"]:
    print(n["rule_id"], n["severity"], n["rule_class"])
```

### `rules` — ルールレジストリ

検証を実行せずにルールレジストリ全体を一覧します — 連携するプロジェクトのルール辞書用です。

```bash
gtfs-analyzer rules --class spec --severity critical
gtfs-analyzer rules --rule STM_004 --json --pretty
```

フィールド：`id`、`severity`、`class`、`authority_source`、`base_effort`、`blocks`、`title`。
`--class` / `--severity` / `--min-severity` / `--rule` フィルタの意味は `validate` と同じです。
`--lang` はここでも有効です（ルールのタイトル）。

### 出力言語

検証コアは指摘テキストをトルコ語で生成します。`--lang en` / `--lang ja` / `--lang fr` は、**Web UI と同じ翻訳辞書**を使ってそれらを置き換えます。ルール ID、重大度、クラス（`CRITICAL`、`SPEC`）はどの言語でも機械可読な値のままで、翻訳されるのは `title`、`message`、`remediation` のみです。

ルールの翻訳がない場合の順序は、指定言語 → 英語 → トルコ語（コアが生成したテキスト）です。出力が空になることはありません。

辞書は `npm run locales:export` により `ui/src/locales/{en,ja}.ts` から `crates/cli/locales/*.json` へ生成され、CLI バイナリに埋め込まれます。locale を更新して export を実行しなかった場合は `locale-parity.test.ts` が CI で失敗します — 単一の情報源は locale ファイルです。

---

## 分析のしきい値

検証のしきい値は、アップロード画面の**分析のしきい値**セクションからカスタマイズできます。変更した値は次回の ZIP アップロード時に有効になります。リセットボタンでデフォルト値に戻せます。

### ルールのクラスと権威ソース

各ルールは4つのクラスのいずれかに分類されます。クラスは検出結果の**権威ソース**（正当性の根拠）を反映し、ユーザーはその検出が本当の GTFS Spec 違反なのか、相互運用性・品質・分析シグナルなのかを一目で判断できます:

- **Spec** — 公式の **GTFS Schedule Reference** が明示的に必須・禁止・無効と定めるケースのみ（required / conditionally required / conditionally forbidden フィールド、enum 値、外部キー、一意性、フォーマット制約）。他のソースは `Spec` を生成しません。
- **Interop** — MobilityData、Google Transit、または地域プロファイル（例: GTFS-JP）などの消費者/バリデータ挙動との互換性シグナル。
- **Quality** — GTFS ベストプラクティス、データ品質、可読性、一貫性、制作品質のチェック。
- **Analytics** — 統計的・運用的・パフォーマンス・分析目的のシグナル。

各ルールには機械可読な**権威ソース**（`authority_source`）フィールドもあります（`GTFS_SPEC`、`MOBILITYDATA_PARITY`、`REGIONAL_PROFILE`、`PROJECT_QUALITY` など）。不変条件: **`Spec` クラスは `authority_source = GTFS_SPEC` の場合にのみ正当**であり、MobilityData/Guru/Google とのパリティ、ベストプラクティス、プロジェクト固有のヒューリスティックだけでは Spec の証拠になりません。

### オプションプロファイルとソースURL

config deltaで`stop_name_best_practices=true`を設定すると、言語依存の`STP_040`と`STP_041`が有効になります。誤検出の可能性があるためデフォルトでは無効です。URLベースの統合では`source_url`メタデータを指定でき、`ARC_028`が恒久的な公開URLに`.zip`ファイル名を含むことを確認します。ファイルアップロードのみの場合、このチェックは実行されません。coreエンジンはフィード内URLへ通信せず、HTTP到達性チェックには明示的にopt-inする別online adapterが必要です。

### shape距離フィールドの整合

`stop_times.txt`で`shape_dist_traveled`を使用する便の参照shapeについて、`shapes.txt`の一部の点に同じ項目がない場合、`SHP_030`（品質・中）を出力します。両フィールドはGTFSで個別に任意項目なので、これはSpecの公開ブロッカーではありません。shape上で停留所を確実に配置できない可能性を示す互換性シグナルで、影響を受ける便数と代表的なtrip IDを検出結果の詳細に含めます。

便から実際に参照される1点だけのshapeは、`shape_id`と`shape_point_count=1`を詳細に含む低・品質の`SHP_006`として報告します。2点の直線セグメントは有効です。未使用の1点shapeは`SHP_018`だけで報告します。これはMobilityDataの`single_shape_point`に対する意図的なnear-parityで、Analyzerは使用中のshapeだけを`SHP_006`で報告します。

### 遠距離停留所の速度パリティ

現在のMobilityData rulesページでは`fast_travel_between_far_stops`の表示が一貫していません。主WARNING表では有効ですが、notice-detail metadataは`Deprecated since undefined`を示し、deprecated表にはありません。#115 auditでは陽性20 feedを調査し、10 km超の累積距離、非連続の停留所ペア、時刻cascadeを混在させるnoisyなシグナルであることを根拠に、`STM_012`/`STM_014`へのaliasを採用しませんでした。deprecationの仮定には依存せず、新しいルールは追加せず、意図的なAnalytics coverage gapとして扱います。

### 停留所URLの固有性

`STP_034`と`STP_035`は、保守的な構文上の同一性を使って`stop_url`を事業者URL・路線URLと比較し、低優先度の品質検出結果として報告します。スキーム/ホストの大文字小文字、ルート`/`、明示されたHTTP 80/HTTPS 443のデフォルトポートは同一とみなします。一方、クエリ、fragment、パス末尾の`/`、percent-encodingの違いは保持します。同じ正規化URLを使う停留所は1件の集約検出結果にまとめ、影響停留所数と代表IDを`details`に含めます。

### 速度のしきい値

| パラメーター | デフォルト | 範囲 | 説明 |
|---|---:|---|---|
| バス最高速度 | 120 km/h | 60–200 | バス便に許容される最高速度 |
| トラム最高速度 | 100 km/h | 40–160 | トラム便に許容される最高速度 |
| 地下鉄最高速度 | 150 km/h | 80–250 | 地下鉄便に許容される最高速度 |
| 鉄道最高速度 | 300 km/h | 100–400 | 鉄道便に許容される最高速度 |
| フェリー最高速度 | 80 km/h | 20–150 | フェリー便に許容される最高速度 |
| ケーブルカー最高速度 | 30 km/h | 10–60 | ケーブルカー・フニクラーに許容される最高速度 |

### 地理的・乗換しきい値

| パラメーター | デフォルト | 範囲 | 説明 |
|---|---:|---|---|
| 最小乗換時間 | 180 s | 30–1800 | 乗換に必要な最小接続時間 |
| 最大乗換距離 | 500 m | 50–2000 | 乗換が有効とみなされる最大距離 |
| 最大経路ジャンプ | 10 km | 1–50 | 連続するシェイプポイント間の最大距離 |
| 近接停留所しきい値 | 5 m | 1–20 | これより近い停留所は重複としてフラグが立てられます |
| 停留所からシェイプへの距離 | 100 m | 20–500 | 停留所がシェイプから離れてよい最大距離 |
| 親駅からの距離 | 100 m | 10–1000 | 停留所が親駅から離れてよい最大距離 |

### サービス・運用しきい値

| パラメーター | デフォルト | 範囲 | 説明 |
|---|---:|---|---|
| 有効期限警告 | 30 日 | 1–60 | この日数以内にフィードが期限切れになる場合に警告を生成 |
| feed_info有効期限警告 | 7 日 | 1–60 | `FIN_019`のデフォルト警告期間。`feed_info_expiry_warning_days=30`ならMobilityDataの30日パリティを適用でき、`CAL_008`とは別設定 |
| サービス空白しきい値 | 7 日 | 3–30 | これより長いサービスの空白にフラグが立てられます |
| 最大便所要時間 | 24 h | 8–72 | 1 便の最大所要時間 |
| 最小便所要時間 | 60 s | 10–300 | 1 便の最小所要時間 |
| 最大運行間隔 | 240 分 | 60–720 | これより長い運行間隔は警告を生成 |
| 過密しきい値 | 2 分 | 1–10 | これより短い運行間隔は過密としてフラグが立てられます |

---

## スコア

### 公開スコア（0〜100）

公式 GTFS Schedule Reference に照らしてフィードが公開可能かを測定します。スコアは **100 からスタート** し、各公開ブロッカー問題についてルールの重みと修正コストに比例したペナルティが差し引かれます。

**スコアの計算方法：**
- `仕様` クラスの `致命的` 重大度の問題のみ（公式 GTFS 仕様ゲート）が公開スコアに影響します。`相互運用` の互換性シグナルは別途報告されます（相互運用スコア / R8）。
- 同じルールが複数回発動した場合、ペナルティは **2×** で上限が設けられます。1 つの問題でスコアがゼロになることはありません。
- **0〜40：** フィードはおそらく使用不可です。ブロッカーエラーが存在します。
- **40〜70：** 部分的な問題があります。一部のアプリケーションがフィードを拒否する可能性があります。
- **70〜90：** 使用可能ですが、注意が必要です。
- **90〜100：** 公開準備完了です。

### 総合スコア（0〜100）

4 つの分析クラスの加重平均です（Spec×40% + Interop×30% + Quality×20% + Analytics×10%）。仕様準拠だけでなく、運用上のデータ品質も反映します。フィードは公開スコアが高くても、総合スコアが低い場合があります。

**スコアの計算方法：**
- 4 つのクラスすべての問題がそれぞれの重みに応じてこのスコアに影響します。
- オプションフィールドの欠落、サービスパターンの不整合、アクセシビリティの欠如は Quality・Analytics コンポーネントを通じて反映されます。
- **0〜60：** 重大な品質問題があります。乗客体験が影響を受ける可能性があります。
- **60〜80：** 品質は普通ですが、改善が推奨されます。
- **80〜100：** 良好なデータ品質です。

> **注記：** 公開スコアと総合スコアは異なる目的・異なる計算式で算出されます。公開スコアが高く総合スコアが低いフィードは技術的には機能しますが、アクセシビリティ情報の欠落や路線名の誤りなどの問題は乗客に影響を与えます。

---

## レポートタブ

### 1. レポート
概要サマリー：両スコア、フィードの指標（路線数、便数、日付範囲など）、および通知の分布チャートが表示されます。

### 2. 詳細＆修正
問題は優先度スコア順に並んだ修正キューとして表示されます。各行には次の情報が含まれます：

| 列 | 説明 |
|---|---|
| **スコア** | 優先度スコア — `重大度 × (1 + 依存) × log₂(1 + 件数) / 工数` で算出。高いほど先に修正 |
| **+公開** | このルールを修正した場合の公開スコアの向上量 |
| **+スコア** | このルールを修正した場合の総合スコアの向上量 |
| **依存** | このルールを修正すると自動的にクローズされる他のアクティブなルール数 |
| **工数** | 修正工数：1 = 単一フィールドの変更、2 = 限定的な複数ファイル、3 = 構造的 / データモデルの改訂 |

すべての +公開 値の合計は `100 − 現在の公開スコア` に等しく、すべての +スコア 値の合計は `100 − 現在の総合スコア` に等しくなります。地理的な問題には地図アイコンが表示され、クリックするとインタラクティブな地図上に問題の場所と関連するシェイプ・停留所データが表示されます。**ルールコード**をクリックすると、関連する GTFS 仕様のセクションが新しいタブで開きます — その指摘が最も影響するファイルのリファレンスページ（GTFS-JP ルールでは gtfs.jp）。

### 3. カテゴリ別
グループとクラスごとにすべてのルール違反が一覧表示されます。各行にはルールコード、タイトル、影響を受けたレコード数、重大度、および修正ガイダンスが表示されます。フィルタリングとソートがサポートされています。

### 4. エクスポート
レポートを HTML、CSV、または JSON としてダウンロードできます。PDF オプションはブラウザの印刷ダイアログを開きます —「PDF として保存」でエクスポートしてください。

---

## インタラクティブな GTFS ファイルマップ

GTFS Analyzer には、GTFS のデータ構造を、分析対象フィードの実際の検証結果と組み合わせて表示するインタラクティブなファイルマップが含まれています。

このビューは静的なスキーマ図ではありません。フィードに存在するファイル、欠落しているファイル、検出された問題、および検証済みのファイル間の関係を、分析結果に基づいて表示します。

### 機能

- 7 つの基幹 GTFS ファイルを **カレンダー** と **基幹サービス** のグループに表示
- 基幹以外の標準ファイルは、アナライザーが問題を検出した場合のみ表示
- フィードに含まれる仕様外ファイルを別グループとして一覧表示
- `route_id`、`trip_id`、`stop_id`、`service_id`、`shape_id` などの検証済み GTFS 関係を可視化
- ファイルを最も高い問題の重大度に応じて色分け
- 欠落・正常・問題ありのファイルを区別
- 行数、ファイルサイズ、検出件数、重大度の分布を表示
- 検出結果をルールごとに、常に **致命的 → 高 → 中 → 低 → 情報** の順で一覧表示
- 選択したファイルのすべての検出結果を、フィルタ済みの「詳細＆修正」ビューで開く
- ファイルの有無と重大度のフィルタを提供
- ズーム、画面に合わせる、ダークテーマ、モバイルレイアウトに対応

ファイルを選択すると、検証済みかつ関連する GTFS の接続のみが展開されます。仕様外ファイルは表示されたままですが、検証されていない関係は描画されません。

分析と可視化はすべてブラウザ内で実行されます。GTFS ファイルがサーバーにアップロードされることはありません。

![GTFS ファイルマップ](docs/images/gtfs-file-map.png)

---

## 実行間の比較

GTFS Analyzer は、同じフィードの 2 つの解析（前／後）を比較し、修正によって何が改善され、何が悪化したかを示します。**比較**タブを開き、以前の解析からダウンロードした **Golden JSON** をアップロードしてください。差分は現在の実行と比較して計算されます。

### 機能

- 公開・総合・サブスコア（仕様、相互運用、品質、分析）の前後変化を表示
- 各ルールを **修正済み・減少・増加・新規・同一** に分類し、フィルターと検索を提供
- 重大度（重大 → 情報）とクラス（仕様／相互運用／品質／分析）の分布変化を表示
- フィード構造（trip・stop・`stop_times`・`calendar_dates` の行数）とフィード／サービス期間を比較
- 通知密度を **1,000 trip あたり**および **100,000 stop_time あたり**に正規化し、規模の異なるフィードを比較可能に
- 2 つの実行がフィード名・期間・設定で異なる場合に警告し、誤解を招く差分の読み違いを防止
- 比較を CSV としてエクスポート
- 旧 Golden スキーマ（v1〜v3）も読み込み可能

比較は完全にブラウザ内で実行されます。Golden JSON はローカルで解析され、サーバーには一切アップロードされません。

---

## ルールクラス

| クラス | 測定内容 | 影響するスコア |
|---|---|---|
| **仕様** | GTFS 仕様からの逸脱 — 必須フィールドの欠落、無効な値、参照整合性エラー | 公開スコア |
| **相互運用** | 仕様準拠だが、一般的なコンシューマー（Google Maps、Apple Maps など）に拒否または誤解釈される問題 | 公開スコア |
| **品質** | オプションだが期待されるフィールドの欠落、不整合、ベストプラクティスからの逸脱 | 総合スコア |
| **分析** | サービスパターン分析 — 過密、疎なサービス、期限切れサービス | 総合スコア |

---

## 重大度レベル

| レベル | 意味 |
|---|---|
| **致命的** | フィードを使用不可にする、またはデータ損失を引き起こす |
| **高** | 重大な機能的問題；強く修正を推奨 |
| **中** | 注意が必要な不整合 |
| **低** | ベストプラクティスからの軽微な逸脱 |
| **情報** | 情報提供のみ；対応が不要な場合もある |

重大度は、[GTFS Schedule Reference](https://gtfs.org/documentation/schedule/reference/#file-requirements) のファイル/フィールド要件レベル（Required・Conditionally Required・Recommended・Optional）と、違反のsemantic impactを組み合わせて決定します。

### Spec 重大度の基準

Spec の重大度は、要件レベルとsemantic impactの組み合わせであり、同じ検出結果を MobilityData が `ERROR`、`WARNING`、`INFO` のどれで
分類するかではなく、無効なフィードが及ぼす影響で決まります。

- **致命的:** 必須ファイル/フィールド、主キー・外部キー整合性、または中核的な型/範囲の違反。フィードを信頼して利用できず、`Spec + 致命的` は公開ゲートになる。
- **高:** フィードを解析できても、運行、運賃、アクセシビリティ、Flex/pathway の意味を大きく変える直接的な規範違反。
- **中:** 主データモデルを読み取れる範囲で、影響が限定された、または条件付きの規範違反。
- **低:** 影響範囲が狭いメタデータ/オプション項目の規範上の逸脱。公開をブロックしない。
- **情報:** 規範上の Spec 違反には使用せず、測定または文脈シグナル専用。

したがって `Spec` ルールに `情報` の重大度は存在できません。2026-08-09 の audit では
307 個すべての Spec ルールを確認し、raw サービス日ルール `STM_048` と `STM_049` を
情報から高へ変更しました。全 ID の一覧は [Spec 重大度 audit](docs/audits/spec-severity-rubric-2026-08-09.md) を参照してください。

GTFS-JP フィードの場合、**JPN** グループのルールは公式の [GTFS-JP 仕様](https://www.gtfs.jp/)（gtfs.jp）に基づいています。

---

## 通知の上限

大規模なフィードでは、同じルールが何千回も発動することがあります。無制限の通知リストはブラウザのメモリを圧迫し、可読性を低下させます。2 段階の上限が適用されます：

| 上限 | 値 | 適用範囲 |
|---|---|---|
| ルールごと（デフォルト） | 500 | すべてのルール |
| ルールごと（高上限） | 2,000 | `TRP_020`、`OPR_007`、`STP_016`、`STP_017` |
| 合計（全ルール） | 100,000 | フィード全体 — 超過した場合は検証を停止 |

高上限リストのルールは、実際のフィードで大量の件数が自然に発生します（例：便ごとに 1 件の運行間隔レコード）。ルールが上限に達した場合、実際の違反件数は修正キューの**合計**列に表示されます。全検出結果タブでそのルールフィルターを選択すると、黄色の警告バナーが表示されます。

---

## ルールグループ

各ルールは `GROUP_NNN` 形式でコード化されています。グループは GTFS のファイルおよびコンポーネントの境界に従っています。

| グループ | GTFS コンポーネント | 説明 |
|---|---|---|
| **ARC** | アーカイブ・ファイルレベル | ZIP 展開、ファイル形式、必須ファイルの存在、文字エンコーディング |
| **AGN** | `agency.txt` | 交通事業者情報と複数事業者の一貫性 |
| **CAL** | `calendar.txt` | サービスカレンダーと週次曜日パターン |
| **CLD** | `calendar_dates.txt` | サービス例外日と日付の有効性 |
| **STP** | `stops.txt` | 停留所の位置、階層、アクセシビリティ情報 |
| **RTS** | `routes.txt` | 路線定義、路線タイプ、色、命名 |
| **TRP** | `trips.txt` | 便定義、ブロックとシェイプの関連付け |
| **STM** | `stop_times.txt` | 停留所の時刻、速度、シーケンス、時刻の一貫性 |
| **SHP** | `shapes.txt` | 経路シェイプ、ポイント順序、停留所との整合性 |
| **FRQ** | `frequencies.txt` | 頻度ベースの便と運行間隔値 |
| **TRF** | `transfers.txt` | 乗換定義、タイプ、所要時間の有効性 |
| **FAR** | `fare_attributes.txt` | 運賃定義、通貨、支払方法 |
| **FRL** | `fare_rules.txt` | 路線・ゾーンベースの運賃ルール |
| **FIN** | `feed_info.txt` | フィード発行者情報、言語、有効期間 |
| **PTH** | `pathways.txt` | 駅構内の通路ネットワークとアクセシビリティ接続 |
| **LVL** | `levels.txt` | 駅のフロアとエレベーター・階段の関係 |
| **TRN** | `translations.txt` | フィールドの翻訳と言語の一貫性 |
| **ATR** | `attributions.txt` | データソースと帰属情報 |
| **XFL** | クロスファイル | ファイル横断の参照整合性と一貫性 |
| **GEO** | 地理的分析 | 座標の一貫性、外れ値の検出、クラスタリング |
| **OPR** | 運用分析 | 便間の待機時間、路線密度、停留所の繰り返し |
| **VAT** | ネットワークトポロジー | 孤立した停留所、接続されていない路線、ネットワークアクセシビリティ |
| **DQ** | フィード全体の品質 | 一般的なデータ品質指標としきい値チェック |
| **RCT** | `rider_categories.txt` | 利用者カテゴリ、年齢範囲、デフォルトカテゴリ（Fares v2） |
| **FMD** | `fare_media.txt` | 支払媒体：物理カード、モバイルアプリ、EMV など（Fares v2） |
| **FPD** | `fare_products.txt` | 運賃商品、金額、通貨、媒体・カテゴリの関連付け（Fares v2） |
| **FLG** | `fare_leg_rules.txt` | 区間ごとの運賃ルールと優先度（Fares v2） |
| **FLJ** | `fare_leg_join_rules.txt` | 乗り継ぎで2つの区間を1つの実効運賃区間として扱う結合ルール（Fares v2） |
| **FTR** | `fare_transfer_rules.txt` | 乗換運賃ルールと時間制限（Fares v2） |
| **ARS** | `areas.txt` | 地理的エリア定義（Fares v2） |
| **SAR** | `stop_areas.txt` | 停留所とエリアのマッピング（Fares v2） |
| **NET** | `networks.txt` | ネットワーク定義（Fares v2） |
| **TFR** | `timeframes.txt` | 時間帯グループとサービスカレンダーの関連付け（Fares v2） |
| **BKR** | `booking_rules.txt` | デマンド型交通の予約ルール、事前通知ウィンドウ、予約タイプ（GTFS Flex） |
| **PDW** | Flex ウィンドウルール | `stop_times.txt` におけるデマンド型交通の乗降時間ウィンドウの一貫性（GTFS Flex） |
| **LOC** | `locations.geojson` | 柔軟サービスゾーンのジオメトリと形式の検証（GTFS Flex） |
| **GGL** | Google Transit 固有 | Google Maps および Google Transit が追加で要求または制限するルール |
| **JPN** | GTFS-JP プロファイル | 日本の GTFS-JP プロファイルルール — カナ読み、`office_jp.txt`・`agency_jp.txt` の参照整合性（GTFS-JP フィードのみ） |

---

## 開発者セットアップ

### 必要環境

- **Rust** — GNU ツールチェーン（`stable-x86_64-pc-windows-gnu`）、MinGW gcc
- **wasm-pack** — WASM ビルドツール
- **Node.js** — メンテナンス中の LTS リリース（正確な範囲は `ui/package.json` の `engines` を参照）

> **Windows の注意：** MSVC の代わりに GNU ツールチェーンが必要です。WASM ビルド時に `wasm-opt` がダウンロードされますが、このステップは MSVC リンカーと非互換です。MinGW の `gcc` が PATH に含まれている必要があります。

```powershell
# Rust GNU ツールチェーン（初回のみ）
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup override set stable-x86_64-pc-windows-gnu
```

### ビルド

```powershell
# 1. 依存関係のインストール
cd ui
npm install

# 2. WASM のコンパイル
npm run wasm

# 3. UI のコンパイル
npm run build
# 出力先: ui/dist/
```

### 開発サーバー

```powershell
cd ui
npm install
npm run dev
```

### テスト

```powershell
# Rust ユニットテストと統合テスト
cargo test

# workspace の全 crate・test・example target に対する警告ブロック lint
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Playwright スモークテスト
cd ui
npx playwright test
```

## プロジェクト構成

```
gtfs-validator/
├── crates/
│   ├── config/     # 設定型
│   ├── core/       # 共有データ構造と結果モデル
│   ├── pipeline/   # 検証パイプライン（k1〜k7 ステージ）
│   ├── rules/      # ルール定義とレジストリ（612 ルール、38 グループ）
│   └── wasm/       # wasm-bindgen WASM 出力
├── spec-audit/     # 仕様から生成したフィールド表（アンカー検査）
└── ui/             # Vite + TypeScript フロントエンド
    ├── pkg/          # wasm-pack 出力（生成済み、コミット済み）
    ├── src/
    │   └── pages/    # アプリケーションタブ（domain/fix/rules/export）
    └── tests/        # Playwright テスト
```

## ライセンス

MIT — 詳細は [LICENSE](LICENSE) をご参照ください。
