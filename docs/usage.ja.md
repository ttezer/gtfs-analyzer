# 9 つの利用方法

🇯🇵 **日本語** · [README に戻る](../README.ja.md)

GTFS Validator & Analyzer の 630 ルールのエンジンには 9 つの方法でアクセスできます。このページでは、それぞれのインストールと使い方を説明します。

## 1. ブラウザ

GTFS Analyzer は Web アプリケーションです — インストール不要です。ブラウザでライブ版を開き、GTFS の zip ファイルをアップロードしてください。

実行エンジンはブラウザの機能に応じて自動選択されます。Memory64 対応環境では **WASM64**、それ以外では **WASM32** を使用し、フィードサイズの固定上限はありません。使用中のエンジンは画面に表示されます。診断用に `?wasm32=1`、`?wasm64=1`、`?serial=1` を使用できます。

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

> セルフホスティングや開発環境の構築については、[開発者セットアップ](../README.ja.md#開発者セットアップ)をご参照ください。

## 2. CLI

Web UI に加えて、同じ検証コア（`gtfs_pipeline::validate_bytes`）をターミナルから実行できます — Python/自動化連携向け。

### インストール

Rust がインストールされている場合、最短の方法：

```bash
cargo install gtfs-analyzer
gtfs-analyzer validate feed.zip
```

Rust をインストールせずに使う場合：[Releases](https://github.com/ttezer/gtfs-analyzer/releases) からお使いのプラットフォーム向けアーカイブ（`x86_64-linux`、`aarch64-macos`、`x86_64-macos`、`x86_64-windows`）をダウンロードし、展開して `gtfs-analyzer` バイナリを `PATH` に配置します。

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
| `--compact-json` | 機械的な利用者向けの正規コンパクト JSON 投影を出力（R9 の根本原因例は `--samples N` で追加） |
| `--summary` | 短い要約：ステータス、通知数、スコア（デフォルト。`--json` とは併用不可） |
| `--rule SHP_010` | 指定ルールの通知のみ |
| `--severity critical` | この重大度に完全一致する通知のみ（critical/high/medium/low/info） |
| `--min-severity high` | この重大度以上（critical が最も重い） |
| `--class spec` | 指定したルールクラスのみ — `spec,interop,quality,analytics`、カンマ区切りで複数可 |
| `--disable-rule DQ_004` | 指定したルールを指摘・スコア・R9キューから除外します。カンマ区切りで複数可 |
| `--disable-rule-gtfs-jp DQ_004` | GTFS-JPと判定されたフィードでのみ、指定したルールを除外します |
| `--fail-on critical` | この重大度以上が存在する場合**のみ** exit 1 |
| `--fail-on-class spec` | 指定クラスの通知が存在する場合のみ exit 1 |
| `--pretty` | JSON をインデント出力（`--json` が必要） |
| `--include-name-index` | `name_index`（停留所/路線/shape の参照表）を JSON に含める |
| `--max-notices-per-rule 20` | JSON に残す通知をルールごとに最大 N 件にする。切り詰めたルールの実件数は `capped_totals` に入り、R1/R5/R9 は全通知から計算される（`--json` が必要） |
| `--samples N` | `--compact-json` と併用すると、R9 の各ルールに最初の N 件の根本原因指摘（`entity_type`、`entity_id`、`file`、`line`、`field`、`observed_value`、`expected_value`）を追加 |
| `-o report.json` | stdout ではなくファイルに出力 |
| `--lang en` | 指摘テキストの言語：`en`（デフォルト）/ `tr` / `ja` / `fr` / `es`（`es` はプランナーカードのみ） |
| `--config config.json` | JSON config デルタを適用（`ValidatorConfig::default()` の上に） |
| `--today 20260710` | 解析の「今日」を固定（カレンダールール用） |

**フィルタは表示を絞るだけです。** `notices` と R2–R9 のリストはフィルタされますが、**R1 の公開可否判定と R5 のスコアは常にフィード全体**を表します。フィルタ適用時は JSON に `filtered` フィールド、要約に `filter:` 行が追加されます。

`name_index` は**デフォルトでは出力されません**：大規模フィードでは停留所/shape の座標表がペイロードの大半を占めるためです。必要な場合は `--include-name-index` を指定してください。

パスの代わりに `-` を渡すと ZIP を **stdin** から読み込みます：`curl -sL <url> | gtfs-analyzer validate - --json`。（ZIP の中央ディレクトリはファイル末尾にあるため、アーカイブはストリーミングではなくメモリに読み込まれます。）

> **Web UI とは件数が異なります。** ブラウザは性能のためルールごとの指摘件数に上限を設けます（実際の合計は `capped_totals` で報告されます）。CLI はこの上限を**適用しません** — 同じフィードでもより多くの notice と、スケーリングされていない R9 影響値が返ります。これは想定どおりの差異であり、両者の件数を直接比較しないでください。

### JSON エンベロープのバージョン

すべての JSON 出力（`--json`、`--compact-json`、致命的エラーのオブジェクト）は、トップ
レベルに `schema_version`、`engine_version`、`engine_commit` を含みます。
`schema_version`（現在 `1`）は既存フィールドの削除、名前の変更、意味の変更があった
場合にのみ上がります。新しいフィールドはバージョンを変えずに追加されることがあります。
出力を読むプログラムはこの値を確認し、未知のフィールドを受け入れてください。

### Compact JSON と Web の上限

`--compact-json` は機械的な利用者向けの正規投影です。完全な `--json` レポート
から個別の `notices` リストと `name_index` テーブルを省略します。トップレベル
には `status`、`schema_version`、`engine`、`engine_version`、`engine_commit`、
`validation_date`、`partial`、`analysis`、`reports.r1`、`reports.r5`、
`reports.r9`、`metrics`、`notice_counts`、`triggered_rule_count` が含まれます。
`reports.r9.items` の各要素にはルール名、重大度、クラス、メッセージ、修正方法、
優先度、影響度、スコア差分に加え、`--lang` の言語による平易な `card`（`title`、`what`、
`action`）が含まれ、`--samples N` で各要素に最大 N 件の根本原因例を追加できます。

`--compact-json` は Web UI の payload ではなく、それ自体はルールごとの上限を
適用しません。Web UI の表示 notice 件数を完全 JSON で近似する場合は
`--json --max-notices-per-rule 500` を使ってください。R1/R5/R9 は全 notice から
計算され、切り詰めたルールの実数は `capped_totals` に記録されます。

**終了コード：** `0` 通知なし · `1` 通知ありまたは `PARTIAL` レポート · `2` 致命的エラーまたは config/ファイルエラー。`PARTIAL` レポートでは利用できない入力を安全にスキップし、独立した検査を続行します。JSON には `status: "partial"`、`validation_status: "PARTIAL"`、`partial` の範囲が含まれます。`partial.skipped_checks` は前提条件が利用できず実行されなかった K4/K5/K6 の検査ファミリーと個別ルールを示し、`partial.skipped_stages` は大まかなステージメタデータのために保持されます。`--fail-on*` を指定した場合、`1` は一致する通知があるときのみ返り、その他の指摘は報告されますが実行を失敗させません。JSON モードでは stdout は JSON のみ、エラーは stderr。

```bash
# CI ゲート：公式 GTFS Spec 違反のみで失敗させる
gtfs-analyzer validate feed.zip --fail-on-class spec

# Spec の指摘のみを出力（スコアはフィード全体を表す）
gtfs-analyzer validate feed.zip --class spec --json --pretty -o spec.json
```

```python
import json, pathlib, subprocess

for feed in sorted(pathlib.Path("feeds").glob("*.zip")):
    proc = subprocess.run(
        ["gtfs-analyzer", "validate", str(feed), "--json"],
        text=True, capture_output=True,
    )
    # exit 1 は「通知あり」であり失敗ではない — check=True は使わない
    data = json.loads(proc.stdout)
    if data["status"] == "fatal":
        print(feed.name, "FATAL", data["code"], data["message"])
        continue
    print(feed.name, data["reports"]["r5"]["pub_score"], len(data["notices"]))
```

### `rules` — ルールレジストリ

検証を実行せずにルールレジストリ全体を一覧します — 連携するプロジェクトのルール辞書用です。

```bash
gtfs-analyzer rules --class spec --severity critical
gtfs-analyzer rules --rule STM_004 --json --pretty
```

フィールド：`id`、`severity`、`class`、`authority_source`、`base_effort`、`blocks`、`title`、`message`、`remediation`、`card`。
`--class` / `--severity` / `--min-severity` / `--rule` フィルタの意味は `validate` と同じです。
`--lang` はここでも有効です（ルールのタイトル、メッセージ、修正方法）。

### 出力言語

検証コアは指摘テキストをトルコ語で生成します。`--lang en` / `--lang ja` / `--lang fr` は、**Web UI と同じ翻訳辞書**を使ってそれらを置き換えます。ルール ID、重大度、クラス（`CRITICAL`、`SPEC`）はどの言語でも機械可読な値のままで、翻訳されるのは `title`、`message`、`remediation` のみです。

ルールの翻訳がない場合の順序は、指定言語 → 英語 → トルコ語（コアが生成したテキスト）です。出力が空になることはありません。

ルール単位の出力（`--compact-json` の R9 要素と `rules --json`）には `card` も含まれます。Web UI のプランナービューと同じ平易なタイトル、何が起きたか（`what`）、何をすべきか（`action`）で、`tr`、`en`、`ja`、`fr`、`es` で提供されます。`es` はカードのみの言語で、その他のテキストは英語にフォールバックします。カードの文が指摘ごとの値を必要とする場合、`what` は省略されます。

辞書は `npm run locales:export` により `ui/src/locales/{en,ja}.ts` から `crates/core/locales/*.json` へ生成され、CLI バイナリと Python パッケージに埋め込まれます。locale を更新して export を実行しなかった場合は `locale-parity.test.ts` が CI で失敗します — 単一の情報源は locale ファイルです。

## 3. Rust ライブラリ

自分の Rust サービスに検証を組み込む場合は `gtfs-pipeline` を直接使用します。CLI もファイルシステムもネットワークも不要です:

```toml
[dependencies]
gtfs-pipeline = "0.22.0"
gtfs-config   = "0.22.0"
gtfs-core     = "0.22.0"
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

## 4. CI/CD

`--fail-on` オプションで、指定した重大度またはクラスだけをパイプライン失敗条件にできます。Analytics の指摘でリリース全体が失敗することを防げます。

```yaml
# GitHub Actions — 公式 GTFS Spec 違反のみで失敗させる
- name: GTFS フィードを検証
  run: |
    curl -sL https://github.com/ttezer/gtfs-analyzer/releases/latest/download/gtfs-analyzer-x86_64-linux.tar.gz | tar xz
    ./gtfs-analyzer validate feed.zip --fail-on-class spec --min-severity critical
```

終了コード：`0` クリーン · `1` 条件に一致する指摘あり · `2` フィード / config / ファイルエラー。

## 5. `gtfs-sdk` npm パッケージ

`gtfs-sdk` は v0.22.0 の検証エンジンを型付き JavaScript/TypeScript API として提供します。フィードはローカル WASM で検証され、アプリケーションの外に出ません。

```js
import { validateGtfs } from "gtfs-sdk";

const result = await validateGtfs(new Uint8Array(zipBytes), {
  today: "2026-08-20",
});
console.log(result.notices.length, result.reports.r5.score);
```

公開 API には `validateGtfs`、`getVersion`、および進捗・キャッシュイベントが必要なアプリ向けの `createValidatorSession` が含まれます。低レベルの `gtfs-wasm` binding は SDK 契約の一部ではなく、WASM64 と threaded engine の選択は最初の SDK パッケージでは内部実装です。

パッケージのソースは `sdk/` にあります。詳細な使い方、結果モデル、config リファレンスは [`sdk/README.md`](../sdk/README.md) を参照してください。WASM binding はビルド時に `crates/wasm` から生成されます。

## 6. Python パッケージ

[PyPI](https://pypi.org/project/gtfs-analyzer/) からインストールできます（CPython 3.9 以上）。

```bash
pip install gtfs-analyzer
```

PyO3 ネイティブモジュールを通じて同じ Rust パイプラインを実行するため、Cargo や
CLI のインストールは不要です。ZIP はパスまたはバイト列で渡せます。

```python
from gtfs_analyzer import validate_gtfs

result = validate_gtfs("feed.zip", today="2026-09-30")
print(result["validation_status"])
print(len(result["notices"]))
```

`result` は npm SDK や CLI の JSON 出力と同じ基本結果を持ちます。パラメータ、結果の構造、
エラー型は [`python/README.md`](../python/README.md)（英語）を参照してください。

## 7. R パッケージ

`gtfsanalyzer` パッケージは R から CLI バイナリを実行し、結果をデータフレームで返します。
Java、Python、Rust のツールチェーンは不要です。

```r
# install.packages("remotes")
remotes::install_github("ttezer/gtfs-analyzer", subdir = "r")

library(gtfsanalyzer)
ga_install()                  # チェックサム検証付きでバイナリをユーザーキャッシュへダウンロード
res <- ga_analyze("feed.zip")
res                           # 公開可否、スコア、重大度別の指摘
summary(res)                  # ルールごとの指摘
ga_compare("before.zip", "after.zip")   # 時刻表の編集後に何が変わったか
```

`ga_install()` は SHA256 チェックサムがパッケージに固定されたエンジンのバージョンのみを
インストールし、`openssl` パッケージがない場合は停止します。自分でビルドまたは
ダウンロードしたバイナリを使う場合は `GTFS_ANALYZER_BIN=/path/to/gtfs-analyzer` を設定
してください。終了コード `1`（指摘あり）はエラーではなく、読み込めないフィードでは
`gtfsanalyzer_fatal` 条件が発生します。詳細：[`r/README.md`](../r/README.md)。

## 8–9. ChatGPT と Claude

インストールと使い方は [README の ChatGPT と Claude の節](../README.ja.md)にあります。

## コミュニティ製バインディング

| 言語 | パッケージ | 保守 |
|---|---|---|
| Kotlin / JVM | [`io.github.transittrack:gtfs-analyzer`](https://central.sonatype.com/artifact/io.github.transittrack/gtfs-analyzer) | 本リポジトリ外の作者 |

コミュニティ製バインディングは本リポジトリではテストされず、本リポジトリのリリース工程からも
公開されません。エンジンの問題はここへ、バインディング固有の問題はパッケージ側のリポジトリへ
報告してください。

---
