# Neuf façons de l’utiliser

🇫🇷 **Français** · [Retour au README](../README.fr.md)

Le moteur de 630 règles de GTFS Validator & Analyzer est accessible de neuf façons. Cette page décrit l’installation et l’usage de chacune.

## 1. Navigateur

GTFS Validator & Analyzer est une application web — aucune installation nécessaire. Ouvrez la version en ligne dans votre navigateur et téléversez votre fichier ZIP GTFS.

Le moteur d’exécution est choisi automatiquement selon les capacités du navigateur : ceux qui prennent en charge Memory64 utilisent **WASM64**, les autres **WASM32** ; il n’y a pas de limite fixe de taille. Le moteur actif est indiqué sur l’écran de téléversement. Pour le diagnostic, utilisez `?wasm32=1`, `?wasm64=1` ou `?serial=1`.

**→ [https://ttezer.github.io/gtfs-analyzer/](https://ttezer.github.io/gtfs-analyzer/)**

1. Glissez-déposez votre fichier ZIP GTFS, ou utilisez le sélecteur de fichiers.
2. La validation démarre automatiquement ; la progression s’affiche étape par étape.
3. Une fois terminée, les scores de publication et global apparaissent avec les onglets du rapport détaillé.
4. Pour comparer avec une analyse antérieure, ouvrez **Comparer** et téléversez son fichier Golden JSON. Les règles corrigées, nouvelles, en baisse et en hausse sont présentées avec l’évolution des scores, des dates du jeu de données et de la densité de signalements normalisée.
5. Pour un livrable partageable, ouvrez **Exporter → Rapport de direction PDF**, choisissez la langue du rapport, puis utilisez **Imprimer / Enregistrer en PDF** dans l’aperçu.

### Rapport de direction PDF

Le **Rapport de direction PDF** transforme les résultats détaillés de validation en un document lisible, à code couleur, prêt pour l’impression A4, destiné aux décideurs et aux producteurs de données. Il est généré exclusivement à partir des résultats de **GTFS Analyzer** et n’intègre ni la sortie d’un autre validateur ni une comparaison externe.

Le rapport contient :

- l’état de publication, le score de publication, le score global et les composantes Spec · Interop · Quality · Analytics ;
- un profil du jeu de données couvrant arrêts, lignes, courses, tracés, jours de service et plages de dates ;
- des actions **P0 / P1 / P2** dédupliquées, combinant les bloquants de publication R1 et le classement impact/effort R9 ;
- pour chaque signalement prioritaire : preuve, impact, correction recommandée, nombre réel d’occurrences concernées et gain de score potentiel ;
- des observations structurelles propres au jeu de données, un plan de correction par phases, les répartitions par gravité et par classe, ainsi qu’une annexe technique.

Même lorsque l’interface ne conserve qu’un nombre limité d’exemples de signalements pour des raisons de performance, le rapport utilise les **décomptes agrégés réels** de `capped_totals` lorsqu’ils sont disponibles. Le document peut être généré en turc, anglais, japonais ou français, indépendamment de la langue de l’interface. La génération et l’impression se déroulent entièrement dans le navigateur : les données GTFS ne sont pas téléversées vers un serveur et aucune API externe n’est requise.

> Les scores du rapport évaluent le jeu de données GTFS téléversé ; ils ne mesurent ni la performance ni la précision de GTFS Analyzer lui-même.

> Pour l’auto-hébergement ou la mise en place d’un environnement de développement, voir [Installation pour le développement](../README.fr.md#installation-pour-le-développement).

## 2. CLI

Outre l’interface web, vous pouvez exécuter le même cœur de validation (`gtfs_pipeline::validate_bytes`) depuis un terminal — pour l’intégration Python ou l’automatisation.

### Installation

Avec Rust installé, le chemin le plus court :

```bash
cargo install gtfs-analyzer
gtfs-analyzer validate feed.zip
```

Sans installer Rust : téléchargez l’archive correspondant à votre plateforme depuis [Releases](https://github.com/ttezer/gtfs-analyzer/releases) (`x86_64-linux`, `aarch64-macos`, `x86_64-macos`, `x86_64-windows`), décompressez-la et placez le binaire `gtfs-analyzer` dans votre `PATH`.

```bash
# Linux / macOS — dernière version
curl -sL https://github.com/ttezer/gtfs-analyzer/releases/latest/download/gtfs-analyzer-x86_64-linux.tar.gz | tar xz
./gtfs-analyzer --version
```

Pour construire depuis les sources :

```bash
cargo build --release -p gtfs-analyzer
target/release/gtfs-analyzer validate feed.zip --json

# ou directement
cargo run -p gtfs-analyzer -- validate feed.zip --json
```

### `validate` — validation d’un jeu de données

| Option | Description |
|---|---|
| `--json` | Écrit le résultat complet en JSON |
| `--compact-json` | Produit la projection JSON compacte canonique pour les consommateurs machine ; ajoutez des exemples R9 avec `--samples N` |
| `--summary` | Résumé court : statut, nombre de signalements, scores (par défaut ; incompatible avec `--json`) |
| `--rule SHP_010` | Uniquement les signalements de la règle indiquée |
| `--severity critical` | Signalements de cette gravité exacte (critical/high/medium/low/info) |
| `--min-severity high` | Cette gravité et toutes les plus graves (critical étant la plus grave) |
| `--class spec` | Uniquement ces classes de règles — `spec,interop,quality,analytics`, séparées par des virgules |
| `--disable-rule DQ_004` | Retire les règles indiquées des signalements, des scores et de la file R9 ; séparées par des virgules |
| `--disable-rule-gtfs-jp DQ_004` | Retire ces règles uniquement pour les jeux détectés comme GTFS-JP |
| `--fail-on critical` | Sortie 1 **uniquement** si cette gravité ou pire est présente |
| `--fail-on-class spec` | Sortie 1 uniquement si un signalement de ces classes est présent |
| `--pretty` | Indente la sortie JSON (nécessite `--json`) |
| `--include-name-index` | Inclut `name_index` (tables de correspondance arrêts/lignes/tracés) dans le JSON |
| `--max-notices-per-rule 20` | Conserve au plus N signalements par règle dans le JSON ; le nombre réel des règles tronquées figure dans `capped_totals`, R1/R5/R9 restent calculés sur tous les signalements (nécessite `--json`) |
| `--samples N` | Avec `--compact-json`, ajoute les N premiers signalements de cause racine par règle R9 (`entity_type`, `entity_id`, `file`, `line`, `field`, `observed_value`, `expected_value`) |
| `-o report.json` | Écrit la sortie dans un fichier au lieu de stdout |
| `--lang en` | Langue des textes de signalement : `en` (par défaut) / `tr` / `ja` / `fr` / `es` (`es` : fiches planificateur uniquement) |
| `--config config.json` | Applique un delta de configuration JSON (par-dessus `ValidatorConfig::default()`) |
| `--today 20260710` | Fixe la date « aujourd’hui » de l’analyse (pour les règles de calendrier) |

**Les filtres ne restreignent que l’affichage.** Les `notices` et les listes R2–R9 sont filtrées ; **le verdict de publication R1 et les scores R5 décrivent toujours l’intégralité du jeu de données**. Lorsqu’un filtre est actif, le JSON gagne un champ `filtered` et le résumé une ligne `filter:`.

`name_index` est **omis par défaut** : sur les gros jeux de données, les tables de coordonnées des arrêts et des tracés dominent la charge utile. Passez `--include-name-index` lorsque vous en avez besoin.

Passez `-` à la place d’un chemin pour lire le ZIP depuis **stdin** : `curl -sL <url> | gtfs-analyzer validate - --json`. (Le répertoire central du ZIP se situant en fin de fichier, l’archive est mise en mémoire tampon plutôt que traitée en flux.)

> **Les décomptes diffèrent de ceux de l’interface web.** Le navigateur plafonne le nombre de signalements conservés par règle pour des raisons de performance (les totaux réels figurent dans `capped_totals`). La CLI **n’applique aucun plafond** — le même jeu de données produit davantage de signalements et des chiffres d’impact R9 non réduits. C’est attendu ; ne comparez pas les deux sorties décompte par décompte.

### Version de l’enveloppe JSON

Chaque sortie JSON (`--json`, `--compact-json` et l’objet d’erreur fatale) porte au niveau
supérieur `schema_version`, `engine_version` et `engine_commit`. `schema_version`
(actuellement `1`) ne change que lorsqu’un champ existant est supprimé, renommé ou change
de sens ; de nouveaux champs peuvent apparaître sans changement de version. Les programmes
qui lisent la sortie doivent la vérifier et accepter les champs inconnus.

### JSON compact et plafond web

`--compact-json` est la projection canonique destinée aux consommateurs machine : elle
omet la liste détaillée `notices` et les tables `name_index` du rapport complet
`--json`. Elle contient au niveau supérieur `status`, `schema_version`, `engine`,
`engine_version`, `engine_commit`, `validation_date`, `partial`, `analysis`,
`reports.r1`, `reports.r5`, `reports.r9`, `metrics`, `notice_counts` et
`triggered_rule_count`. Chaque élément de `reports.r9.items` contient le titre, la
gravité, la classe, le message, la correction, la priorité, l’impact et les variations
de score, ainsi qu’une fiche en langage simple `card` (`title`, `what`, `action`) dans la
langue de `--lang` ; `--samples N` ajoute jusqu’à N exemples de cause racine à chaque élément.

`--compact-json` n’est pas le payload de l’interface web et n’applique pas de plafond
par règle. Pour approximer dans le JSON complet de la CLI la liste visible dans l’interface,
utilisez `--json --max-notices-per-rule 500` ; R1/R5/R9 restent calculés sur toute la
liste et les règles tronquées sont indiquées dans `capped_totals`.

**Codes de sortie :** `0` aucun signalement · `1` signalements présents ou rapport `PARTIAL` · `2` erreur fatale, de configuration ou de fichier. Un rapport `PARTIAL` ignore sans danger les entrées indisponibles et poursuit les contrôles indépendants ; le JSON expose `status: "partial"`, `validation_status: "PARTIAL"` et la portée `partial`. `partial.skipped_checks` liste les familles de contrôles K4/K5/K6 et les règles individuelles ignorées faute de prérequis ; `partial.skipped_stages` est conservé pour les métadonnées d’étape grossières. Avec `--fail-on*`, `1` n’est renvoyé que pour un signalement correspondant ; les autres constats sont tout de même rapportés sans faire échouer l’exécution. En mode JSON, stdout ne contient que du JSON ; les erreurs vont vers stderr.

```bash
# Barrière CI : n’échouer que sur les violations officielles de la spécification GTFS
gtfs-analyzer validate feed.zip --fail-on-class spec

# Ne rapporter que les constats Spec (les scores décrivent toujours tout le jeu de données)
gtfs-analyzer validate feed.zip --class spec --json --pretty -o spec.json
```

```python
import json, pathlib, subprocess

for feed in sorted(pathlib.Path("feeds").glob("*.zip")):
    proc = subprocess.run(
        ["gtfs-analyzer", "validate", str(feed), "--json"],
        text=True, capture_output=True,
    )
    # exit 1 signifie « des signalements existent », pas un échec — n’utilisez PAS check=True
    data = json.loads(proc.stdout)
    if data["status"] == "fatal":
        print(feed.name, "FATAL", data["code"], data["message"])
        continue
    print(feed.name, data["reports"]["r5"]["pub_score"], len(data["notices"]))
```

### `rules` — registre des règles

Liste l’ensemble du registre des règles sans lancer de validation — pensé comme dictionnaire de règles pour les projets intégrateurs.

```bash
gtfs-analyzer rules --class spec --severity critical
gtfs-analyzer rules --rule STM_004 --json --pretty
```

Champs : `id`, `severity`, `class`, `authority_source`, `base_effort`, `blocks`, `title`, `message`, `remediation`, `card`.
Les filtres `--class` / `--severity` / `--min-severity` / `--rule` ont le même sens que dans `validate`.
`--lang` s’applique ici aussi (titres, messages et corrections des règles).

### Langue de sortie

Le cœur de validation produit ses textes de signalement en turc ; `--lang en` / `--lang ja` / `--lang fr` les remplacent à l’aide des **mêmes dictionnaires de traduction que l’interface web**. Les identifiants de règles, les gravités et les classes (`CRITICAL`, `SPEC`) restent lisibles par machine dans toutes les langues — seuls `title`, `message` et `remediation` sont traduits.

Lorsqu’une règle n’a pas de traduction, la chaîne est : langue demandée → anglais → turc (le texte propre au cœur), de sorte que la sortie n’est jamais vide.

Les sorties au niveau des règles (éléments R9 de `--compact-json` et `rules --json`) portent aussi `card` : le titre en langage simple, ce qui s’est passé (`what`) et que faire (`action`) de la vue planificateur de l’interface web, en `tr`, `en`, `ja`, `fr` et `es`. `es` ne contient que les fiches : ses autres textes se replient sur l’anglais. `what` est omis lorsque le texte de la fiche a besoin de valeurs propres à un signalement.

Les dictionnaires sont dérivés de `ui/src/locales/{en,ja,fr}.ts` vers `crates/core/locales/*.json` par `npm run locales:export`, puis intégrés au binaire de la CLI et au paquet Python. Si un fichier de locale est modifié sans relancer l’export, `locale-parity.test.ts` échoue en CI — les fichiers de locale restent l’unique source de vérité.

## 3. Bibliothèque Rust

Pour intégrer la validation dans votre propre service Rust, utilisez directement `gtfs-pipeline` — sans CLI, sans système de fichiers, sans réseau :

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
        println!("notices: {}", result.notices.len());
        println!("publication score: {}", result.reports.r5.pub_score);
    }
    ValidateResult::Fatal(err) => eprintln!("fatal: {}", err.message),
}
```

`validate_bytes` prend des octets et renvoie un résultat portant tous les rapports (`r1`–`r9`), les scores et les signalements. Ajustez les champs de `ValidatorConfig` pour modifier les seuils, ou appliquez un delta JSON avec `merge_delta`.

⚠️ Les crates de bibliothèque sont les **rouages internes** de l’analyseur. Elles sont publiées pour que le binaire puisse être construit depuis le registre et n’offrent **aucune garantie de stabilité d’API**. Si vous avez besoin d’une surface stable, la sortie JSON de la CLI ou `gtfs-sdk` est le choix le plus sûr.

## 4. CI/CD

Les options `--fail-on` ne font échouer l’exécution que pour la gravité ou la classe choisie, de sorte que les signalements Analytics ne bloquent pas une chaîne de publication :

```yaml
# GitHub Actions — n’échouer que sur les violations officielles de la spécification GTFS
- name: Valider le jeu de données GTFS
  run: |
    curl -sL https://github.com/ttezer/gtfs-analyzer/releases/latest/download/gtfs-analyzer-x86_64-linux.tar.gz | tar xz
    ./gtfs-analyzer validate feed.zip --fail-on-class spec --min-severity critical
```

Codes de sortie : `0` aucun signalement · `1` signalements correspondants présents · `2` erreur de jeu de données, de configuration ou de fichier.

## 5. Paquet npm `gtfs-sdk`

`gtfs-sdk` expose le moteur de validation v0.22.0 sous forme d’API JavaScript/TypeScript typée. Le jeu de données est validé par WASM local et ne quitte jamais l’application :

```js
import { validateGtfs } from "gtfs-sdk";

const result = await validateGtfs(new Uint8Array(zipBytes), {
  today: "2026-08-20",
});
console.log(result.notices.length, result.reports.r5.score);
```

L’API publique comprend `validateGtfs`, `getVersion` et `createValidatorSession` pour les applications ayant besoin d’événements de progression et de cache. La liaison bas niveau `gtfs-wasm` ne fait pas partie du contrat du SDK ; la sélection des moteurs WASM64 et multithread reste interne au premier paquet SDK.

Les sources du paquet se trouvent sous `sdk/` ; l’utilisation détaillée, le modèle de résultat et la référence de configuration sont dans [`sdk/README.md`](../sdk/README.md). La liaison WASM est générée à partir de `crates/wasm` lors de la construction.

## 6. Paquet Python

Installation depuis [PyPI](https://pypi.org/project/gtfs-analyzer/) (CPython 3.9+) :

```bash
pip install gtfs-analyzer
```

Le paquet exécute le même pipeline Rust via un module natif PyO3 ; aucune
installation de Cargo ou de la CLI n’est nécessaire. Le ZIP se passe sous forme
de chemin ou d’octets :

```python
from gtfs_analyzer import validate_gtfs

result = validate_gtfs("feed.zip", today="2026-09-30")
print(result["validation_status"])
print(len(result["notices"]))
```

`result` contient le même résultat de base que le SDK npm et la sortie JSON de
la CLI. Paramètres, structure du résultat et types d’erreur :
[`python/README.md`](../python/README.md) (en anglais).

## 7. Paquet R

Le paquet `gtfsanalyzer` exécute le binaire de la CLI depuis R et renvoie les résultats
sous forme de data frames. Aucune chaîne d’outils Java, Python ou Rust n’est nécessaire.

```r
# install.packages("remotes")
remotes::install_github("ttezer/gtfs-analyzer", subdir = "r")

library(gtfsanalyzer)
ga_install()                  # téléchargement du binaire vérifié par somme de contrôle
res <- ga_analyze("feed.zip")
res                           # verdict, score, signalements par gravité
summary(res)                  # signalements par règle
ga_compare("avant.zip", "apres.zip")   # ce qui a changé après une modification d’horaires
```

`ga_install()` n’installe que les versions du moteur dont la somme SHA256 est fixée dans
le paquet, et s’arrête si le paquet `openssl` est absent. Pour utiliser un binaire que
vous avez compilé ou téléchargé vous-même, définissez `GTFS_ANALYZER_BIN=/chemin/gtfs-analyzer`.
Le code de sortie `1` (signalements présents) n’est pas une erreur ; un jeu illisible
déclenche une condition `gtfsanalyzer_fatal`. Détails : [`r/README.md`](../r/README.md).

## 8–9. ChatGPT et Claude

Installation et usage : [section ChatGPT et Claude du README](../README.fr.md).

## Liaisons communautaires

| Langage | Paquet | Maintenance |
|---|---|---|
| Kotlin / JVM | [`io.github.transittrack:gtfs-analyzer`](https://central.sonatype.com/artifact/io.github.transittrack/gtfs-analyzer) | Par ses auteurs, hors de ce dépôt |

Les liaisons communautaires ne sont pas testées dans ce dépôt et ne sont pas publiées
par son processus de version. Signalez ici les problèmes du moteur, et ceux propres à
la liaison dans le dépôt du paquet.

---
