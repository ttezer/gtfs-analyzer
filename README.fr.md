# GTFS Validator & Analyzer

🇹🇷 [Türkçe](README.md) · 🇬🇧 [English](README.en.md) · 🇯🇵 [日本語](README.ja.md) · 🇫🇷 **Français**

[![Ouvrir l’application](https://img.shields.io/badge/Ouvrir%20l%27application-gtfs--analyzer-2ea44f?style=flat&logo=googlechrome&logoColor=white)](https://ttezer.github.io/gtfs-analyzer/)
[![GTFS-JP](https://img.shields.io/badge/GTFS--JP-v3%2Fv4%20support%C3%A9-c8102e?style=flat)](https://www.gtfs.jp/)
[![Nombre de règles](https://img.shields.io/badge/r%C3%A8gles-611-blue?style=flat)](RULES.fr.md)
![Couverture de la spécification GTFS](https://img.shields.io/badge/Sp%C3%A9cification%20GTFS-97.2%25-007ec6?style=flat)
[![Validation sur corpus](https://img.shields.io/badge/corpus-4%2C318%20jeux%20de%20donn%C3%A9es%20%C3%97%2012%20ex%C3%A9cutions-brightgreen?style=flat)](audit-results/)
[![crates.io](https://img.shields.io/crates/v/gtfs-analyzer?style=flat&label=crates.io)](https://crates.io/crates/gtfs-analyzer)
[![npm](https://img.shields.io/npm/v/gtfs-sdk?style=flat&label=npm)](https://www.npmjs.com/package/gtfs-sdk)
[![Licence MIT](https://img.shields.io/badge/licence-MIT-yellow?style=flat)](LICENSE)

GTFS Validator & Analyzer est un validateur GTFS et un analyseur de qualité de jeux de données open source. Le fichier `.zip` téléversé n’est jamais envoyé à un serveur ; toute la validation s’exécute sur l’appareil de l’utilisateur via WebAssembly. L’outil est disponible sous forme d’application navigateur, de CLI (`cargo install gtfs-analyzer`), de bibliothèque Rust, de barrière CI/CD et de paquet npm `gtfs-sdk`.

Le projet couvre **97,2 % des exigences mesurables de la spécification GTFS** et rattache les 300 atomes de l’inventaire des champs à au moins une règle Spec. Sur ses **611 règles**, **417** ont produit au moins un signalement lors de la dernière exécution complète sur le catalogue de 4 318 jeux de données ; les ajouts GTFS-JP ont été mesurés séparément sur une exécution de profil portant sur 585 jeux de données. Toutes les règles sont listées dans [`RULES.fr.md`](RULES.fr.md).

L’exactitude est éprouvée face au `gtfs-validator` officiel de MobilityData au fil de **douze exécutions complètes sur le catalogue**. Chaque exécution valide tous les jeux de données GTFS Schedule testables du catalogue — **4 318** lors de la dernière en date — avec les deux validateurs sur la même machine et à la même date, en utilisant réellement le `gtfs-validator v8.0.1` Java. Les sorties brutes sont disponibles dans [`audit-results/`](audit-results/).

GTFS Validator & Analyzer ne se contente pas de vérifier la conformité d’un fichier à la spécification ; il analyse aussi la fiabilité, la cohérence et l’exploitabilité du jeu de données. Il présente chaque erreur avec le fichier et le numéro de ligne concernés, fournit des étapes de correction pour chaque signalement, et localise sur une carte interactive les problèmes géographiques — tracés déviants, coordonnées erronées ou arrêts inatteignables.

Chaque signalement porte un code de règle, une classe d’analyse et un niveau de gravité. Grâce aux classes Spec · Interop · Quality · Analytics et aux niveaux de gravité Critique → Info, des milliers de signalements peuvent être filtrés, priorisés et traités méthodiquement. L’outil détecte également de façon automatique les fonctionnalités GTFS utilisées par le jeu de données — tracés, correspondances, tarifs, girouettes, Flex, etc. — et les intègre au rapport.

GTFS Validator & Analyzer prolonge la validation de la spécification par une analyse de qualité opérationnelle. Incohérences de fréquence par ligne, segments à vitesse anormale, arrêts isolés, ruptures dans les schémas de service et problèmes de topologie du réseau sont examinés à l’aide de 611 règles distinctes de validation et d’analyse. Les résultats sont synthétisés par des scores de publiabilité et de qualité globale du jeu de données. La file de correction priorisée indique quels problèmes traiter en premier et l’effet probable de chaque correction sur le score.

**À qui s’adresse-t-il ?**

- **Exploitants de transport et collectivités** — pour valider un jeu de données et résoudre les problèmes de qualité avant publication.
- **Intégrateurs et consultants GTFS** — pour documenter la qualité technique et opérationnelle des données livrées.
- **Développeurs d’applications** — pour évaluer la fiabilité et les risques d’intégration des jeux de données consommés.
- **Chercheurs et analystes** — pour comparer différents réseaux de transport sous l’angle de la qualité et de la structure des données.

---

## Comparaison avec d’autres outils

### Matrice de fonctionnalités

| Fonctionnalité | MobilityData | GTFS Analyzer |
|---|:---:|:---:|
| Interface web | ✅ | ✅ |
| Les données ne quittent jamais le navigateur | ❌ | ✅ |
| Règles de conformité à la spécification | ✅ | ✅ |
| Règles de qualité | ❌ | ✅ |
| Analytique opérationnelle | ❌ | ✅ |
| Visualisation cartographique | ❌ | Arrêts, lignes, courses, tracés, cheminements |
| Score du jeu de données | ❌ | ✅ |
| Aide à la correction | Partielle | ✅ |
| Prise en charge de GTFS Flex | Partielle | ✅ |
| Validation Tarifs v2 | Partielle | ✅ |
| Validation du profil GTFS-JP | ❌ | ✅ |
| Formats de sortie | HTML, JSON | HTML, CSV, JSON, PDF |
| Distribution | Web · installeurs bureau (msi/dmg/deb) · JAR CLI · Docker | Web · binaire CLI · `cargo install` · SDK npm |
| Intégration CI/CD documentée | Non documentée dans le README (possible via Docker/CLI) | ✅ `--fail-on` + codes de sortie |
| Paquet npm | ❌ | ✅ `gtfs-sdk` |
| Paquet crates.io | — *(projet Java)* | ✅ `gtfs-analyzer` |
| Couverture de la spécification GTFS (mesurée) | — | **97,2 %** · 300/300 ancrages de champs |
| **Nombre total de règles** | **178** | **611** |

### Validation sur corpus

L’exactitude ne se démontre pas avec une poignée de jeux de données. Chaque version est exécutée sur **l’intégralité du catalogue GTFS Schedule de MobilityDatabase** — **4 318 jeux de données** lors de la dernière exécution, répartis sur plus de 640 fragments parallèles. En face se trouve le **`gtfs-validator` v8.0.1** de MobilityData, exécuté lui aussi sur la même archive plutôt que lu depuis ses rapports publiés : la différence porte donc sur « qui a trouvé quoi », et non sur « quel rapport a été produit quand ».

D’après la dernière exécution (`32587015142`, les deux validateurs étant sans incident sur 4 275 jeux de données) :

| | GTFS Analyzer | MobilityData |
|---|---|---|
| Temps d’exécution médian | **0,05 s** | 3,00 s |
| Pic mémoire médian | **14 Mo** | 329 Mo |
| Jeux de données non traités | **1** | 10 |
| Faits vus par MobilityData et pas par nous | **0** | — |

Les sorties brutes se trouvent dans [`audit-results/`](audit-results/) — les sept premières exécutions sont versionnées, les suivantes sont archivées sous forme de préversion `audit-<run-id>`.

### Exemples d’analyse de jeux de données

Les chiffres ci-dessous proviennent de la dernière exécution sur corpus : même archive et même date d’analyse (2026-08-20), MobilityData utilisant le `gtfs-validator v8.0.1` Java.

#### BART (Bay Area Rapid Transit, San Francisco)

Jeu de données : `mdb-53` · 14 lignes, 287 arrêts, 4 417 courses · 0,9 Mo

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Total des signalements | 2 715 | 740 |
| Critique / Erreur | 2 | 2 |
| Élevée / Avertissement | 2 654 | 1 |
| Moyenne | — | 11 |
| Faible | — | 24 |
| Info | 59 | 702 |
| Types de règles déclenchés | 13 | **37** |
| Durée de validation | 3,43 s | **0,19 s** |
| Score de publication | — | **92,6 / 100** |
| Score global | — | **90,9 / 100** |

#### TriMet (Portland, Oregon)

Jeu de données : `mdb-247` · 112 lignes, 6 480 arrêts, 70 557 courses · 28,4 Mo

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Total des signalements | 51 | 3 099 |
| Critique / Erreur | 0 | 0 |
| Élevée / Avertissement | 38 | 12 |
| Moyenne | — | 97 |
| Faible | — | 497 |
| Info | 13 | 2 493 |
| Types de règles déclenchés | 8 | **49** |
| Durée de validation | 14,85 s | **5,46 s** |
| Score de publication | — | **100 / 100** |
| Score global | — | **90,0 / 100** |

> Ce jeu de données est conforme à la spécification : les deux outils signalent zéro constat critique et un score de publication de 100. L’écart du nombre de règles reflète l’analyse de qualité opérationnelle supplémentaire de GTFS Analyzer.

#### Tokyo Toei (Bureau des transports de la métropole de Tokyo)

Jeu de données : `mdb-3175` · 151 lignes, 5 370 arrêts, 68 817 courses · 8,6 Mo · **profil GTFS-JP**

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Total des signalements | 1 849 | 1 741 |
| Critique / Erreur | 0 | 0 |
| Élevée / Avertissement | 268 | 12 |
| Moyenne | — | 809 |
| Faible | — | 548 |
| Info | 1 581 | 372 |
| Types de règles déclenchés | 8 | **49** |
| Durée de validation | 5,94 s | **1,75 s** |
| Score de publication | — | **100 / 100** |
| Score global | — | **87,2 / 100** |

> Le profil GTFS-JP ne produit aucun faux positif sur ce jeu de données japonais réel : il est conforme à la spécification (0 critique, score de publication 100), et les règles de profil n’examinent que les exigences propres au Japon.

#### VBB (Communauté de transport Berlin-Brandebourg)

Jeu de données : `mdb-782` · 1 274 lignes, 41 961 arrêts, 258 524 courses, 14 485 tracés · **~75 Mo**

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Total des signalements | 12 201 | 25 369 |
| Critique / Erreur | 0 | 0 |
| Élevée / Avertissement | 11 486 | 1 307 |
| Moyenne | — | 7 440 |
| Faible | — | 8 186 |
| Info | 715 | 8 436 |
| Types de règles déclenchés | 18 | **91** |
| Durée de validation | 45,16 s | **21,07 s** |
| Score global | — | **78,4 / 100** |

> 🇩🇪 **Jeu de données volumineux :** le validateur web hébergé de MobilityData ne peut pas traiter un jeu de données de cette taille. GTFS Analyzer le valide directement dans le navigateur, sans envoyer le fichier à un serveur. Plus de la moitié du total de MobilityData (`non_ascii_or_non_printable_char`) provient de caractères allemands valides ü/ö/ä/ß ; GTFS Analyzer ne signale pas les lettres Unicode valides. Les contrôles fondamentaux restent alignés.

---

## Prise en charge de GTFS-JP

GTFS Analyzer reconnaît automatiquement **GTFS-JP**, le profil GTFS national du Japon (norme 国土交通省 / MLIT), et applique les exigences que GTFS-JP rend obligatoires là où le GTFS standard les laisse facultatives. Le MLIT imposant aux exploitants subventionnés de publier du GTFS-JP, des centaines de petits exploitants doivent se conformer à ce profil — alors que les validateurs courants ne vérifient pas ses obligations spécifiques.

**Détection automatique.** Un jeu de données est marqué GTFS-JP — et un badge **GTFS-JP** apparaît dans le rapport — lorsqu’il contient les fichiers GTFS-JP actuels (`agency_jp.txt`, `office_jp.txt`, `pattern_jp.txt`) ou le fichier `routes_jp.txt` conservé pour la compatibilité, lorsque `feed_lang` commence par `ja`, ou lorsque `translations.txt` porte des lectures kana (`ja-Hrkt`). `routes_jp.txt` n’est pas un fichier v3 ; il n’est reconnu que pour la compatibilité des jeux de données hérités. Le profil de règles par défaut est **auto** ; l’application web, la CLI et la configuration WASM permettent de sélectionner explicitement `v3` ou `v4`. En v4, les fichiers d’extension v3 sont des données de référence et les règles JPN qui leur sont propres ne s’exécutent pas. Les règles de profil ne s’activent que sur des signaux GTFS-JP et restent silencieuses sur les jeux de données standard.

**Choisir le profil d’une analyse.** Dans l’application web, ouvrez **Critères d’analyse** avant de choisir le ZIP et sélectionnez `Auto`, `V3` ou `V4` sous **Profil de validation GTFS-JP**. La sélection est prise en compte au moment où vous choisissez un jeu de données, avant le démarrage de la validation automatique ; `Auto` est la valeur par défaut. Pour la CLI, utilisez `--gtfs-jp-profile v3` ou `--gtfs-jp-profile v4`. Dans le SDK, passez `config: { gtfs_jp_profile: 'v3' }` ou `'v4'`. Ce choix définit la portée de validation ; il ne déduit pas la version GTFS-JP officielle du jeu de données. Voir la [matrice de compatibilité GTFS-JP v3/v4](docs/gtfs-jp-v3-v4-matrix.md) pour le détail des différences.

**Règles de profil (groupe JPN).**

| Règle | Contrôle |
|---|---|
| **JPN_001** | Lecture kana (よみがな — `translations.txt`, `ja-Hrkt`) des noms d’arrêt ; exigée par GTFS-JP pour les annonces sonores et la recherche |
| **JPN_002** | `jp_office_id` (dans `trips.txt` **ou** `routes.txt`) doit correspondre à un `office_id` défini dans `office_jp.txt` (intégrité référentielle du bureau d’exploitation) |
| **JPN_003** | L’`agency_id` de `agency_jp.txt` doit être défini dans `agency.txt` (intégrité référentielle de l’exploitant) |
| **JPN_004** | `translations.txt` doit être présent — obligatoire en GTFS-JP (notamment pour les lectures kana) |
| **JPN_005** | `office_name` (champ obligatoire) doit être renseigné dans `office_jp.txt` |
| **JPN_006** | `fare_attributes.txt` est obligatoire ; `fare_rules.txt` est conditionnel lorsque les profils tarifaires diffèrent |
| **JPN_007** | `feed_info.txt` doit être présent — obligatoire en GTFS-JP |
| **JPN_008** | Lecture kana (`ja-Hrkt`) du nom de la ligne (`route_long_name`) |
| **JPN_009** | Lecture kana (`ja-Hrkt`) de `trip_headsign` |
| **JPN_010** | Lecture kana (`ja-Hrkt`) du nom de l’exploitant (`agency_name`) |
| **JPN_011** | `agency_id` est obligatoire même lorsque le jeu de données ne comporte qu’une agence |
| **JPN_012** | `agency_jp.agency_id` est obligatoire et doit identifier un enregistrement de `agency.txt` |
| **JPN_013** | Lorsqu’il est présent, `agency_zip_number` doit contenir exactement 7 chiffres ASCII |
| **JPN_014** | `office_jp.office_id` doit être présent et unique |
| **JPN_015** | Contrôle de compatibilité de `routes_jp.route_id` (hérité) ; ce n’est pas un fichier v3 |
| **JPN_016** | `pattern_jp.route_update_date` et `routes_jp.route_update_date` (hérité) doivent être des dates `AAAAMMJJ` valides |
| **JPN_017** | `pattern_jp.jp_pattern_id` doit être présent et unique |
| **JPN_018** | Lorsque `pattern_jp.txt` existe, `trips.jp_pattern_id` doit le référencer |
| **JPN_019** | Les enregistrements `ja-Hrkt` doivent viser des tables, champs, enregistrements et sous-enregistrements GTFS valides |
| **JPN_020** | `office_url` et `office_phone` font l’objet de contrôles de format élémentaires |
| **JPN_021** | Les traductions `ja-Hrkt` doivent être non vides, cohérentes et écrites en japonais |
| **JPN_022** | GTFS-JP v4 exige `agency_lang`, `feed_start_date`, `feed_end_date` et `feed_version` |

La comparaison **Tokyo Toei** ci-dessus montre le comportement du profil sur un jeu de données GTFS-JP réel : le jeu est conforme à la spécification (0 critique) et les règles de profil ne produisent aucun faux positif sur des données correctement référencées.

---

## Utilisation

GTFS Validator & Analyzer est une application web — aucune installation nécessaire. Ouvrez la version en ligne dans votre navigateur et téléversez votre fichier ZIP GTFS.

Le moteur d’exécution est choisi automatiquement selon les capacités du navigateur : ceux qui prennent en charge Memory64 utilisent
**WASM64** pour les jeux de données dépassant 4 Go ; les autres utilisent **WASM32**. Le moteur actif est
indiqué sur l’écran de téléversement. Pour le diagnostic, utilisez `?wasm32=1`, `?wasm64=1` ou `?serial=1`.

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

> Pour l’auto-hébergement ou la mise en place d’un environnement de développement, voir [Installation pour le développement](#installation-pour-le-développement).

---

## Cinq façons de l’utiliser

Le même cœur de validation (`gtfs_pipeline::validate_bytes`) s’exécute de cinq manières — toutes utilisent les mêmes 611 règles et produisent le même modèle de résultat :

| Voie | Idéal pour | Où vont les données |
|---|---|---|
| **Navigateur** ([application](https://ttezer.github.io/gtfs-analyzer/)) | Examiner un jeu de données avec la carte et le rapport | **Nulle part** — WebAssembly sur l’appareil |
| **CLI** (`cargo install gtfs-analyzer`, ou un binaire précompilé) | Validation par lots, scripts et intégration Python | Nulle part — binaire local |
| **Bibliothèque Rust** ([`gtfs-pipeline`](https://crates.io/crates/gtfs-pipeline)) | Intégrer la validation dans votre propre service Rust | Nulle part — votre propre processus |
| **CI/CD** (codes de sortie + `--fail-on`) | Une barrière de publication avant de diffuser un jeu de données | Nulle part — votre propre runner |
| **Paquet npm [`gtfs-sdk`](https://www.npmjs.com/package/gtfs-sdk)** | Intégrer la validation dans votre application web ou Node | Nulle part — WASM local |

Dans aucun de ces modes le jeu de données n’est téléversé vers un serveur. L’outil convient donc à des données qui ne peuvent pas quitter votre organisation pour des raisons de politique interne ou contractuelles.

### Intégration CI/CD

Les options `--fail-on` ne font échouer l’exécution que pour la gravité ou la classe choisie, de sorte que les signalements Analytics ne bloquent pas une chaîne de publication :

```yaml
# GitHub Actions — n’échouer que sur les violations officielles de la spécification GTFS
- name: Valider le jeu de données GTFS
  run: |
    curl -sL https://github.com/ttezer/gtfs-analyzer/releases/latest/download/gtfs-analyzer-x86_64-linux.tar.gz | tar xz
    ./gtfs-analyzer validate feed.zip --fail-on-class spec --min-severity critical
```

Codes de sortie : `0` aucun signalement · `1` signalements correspondants présents · `2` erreur de jeu de données, de configuration ou de fichier.

### Bibliothèque Rust

Pour intégrer la validation dans votre propre service Rust, utilisez directement `gtfs-pipeline` — sans CLI, sans système de fichiers, sans réseau :

```toml
[dependencies]
gtfs-pipeline = "0.11.0"
gtfs-config   = "0.11.0"
gtfs-core     = "0.11.0"
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

### Paquet npm `gtfs-sdk`

`gtfs-sdk` expose le moteur de validation v0.11.0 sous forme d’API JavaScript/TypeScript typée. Le jeu de données est validé par WASM local et ne quitte jamais l’application :

```js
import { validateGtfs } from "gtfs-sdk";

const result = await validateGtfs(new Uint8Array(zipBytes), {
  today: "2026-08-20",
});
console.log(result.notices.length, result.reports.r5.score);
```

L’API publique comprend `validateGtfs`, `getVersion` et `createValidatorSession` pour les applications ayant besoin d’événements de progression et de cache. La liaison bas niveau `gtfs-wasm` ne fait pas partie du contrat du SDK ; la sélection des moteurs WASM64 et multithread reste interne au premier paquet SDK.

Les sources du paquet se trouvent sous `sdk/` ; l’utilisation détaillée, le modèle de résultat et la référence de configuration sont dans [`sdk/README.md`](sdk/README.md). La liaison WASM est générée à partir de `crates/wasm` lors de la construction.

---

## CLI (terminal)

Outre l’interface web, vous pouvez exécuter le même cœur de validation (`gtfs_pipeline::validate_bytes`) depuis un terminal — pour l’intégration Python ou l’automatisation.

### Installation

Avec Rust installé, le chemin le plus court :

```bash
cargo install gtfs-analyzer
gtfs-analyzer validate feed.zip
```

Sans installer Rust : téléchargez l’archive correspondant à votre plateforme depuis [Releases](https://github.com/ttezer/gtfs-analyzer/releases) (`x86_64-linux`, `aarch64-macos`, `x86_64-windows`), décompressez-la et placez le binaire `gtfs-analyzer` dans votre `PATH`.

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
| `--summary` | Résumé court : statut, nombre de signalements, scores (par défaut ; incompatible avec `--json`) |
| `--rule SHP_010` | Uniquement les signalements de la règle indiquée |
| `--severity critical` | Signalements de cette gravité exacte (critical/high/medium/low/info) |
| `--min-severity high` | Cette gravité et toutes les plus graves (critical étant la plus grave) |
| `--class spec` | Uniquement ces classes de règles — `spec,interop,quality,analytics`, séparées par des virgules |
| `--fail-on critical` | Sortie 1 **uniquement** si cette gravité ou pire est présente |
| `--fail-on-class spec` | Sortie 1 uniquement si un signalement de ces classes est présent |
| `--pretty` | Indente la sortie JSON (nécessite `--json`) |
| `--include-name-index` | Inclut `name_index` (tables de correspondance arrêts/lignes/tracés) dans le JSON |
| `-o report.json` | Écrit la sortie dans un fichier au lieu de stdout |
| `--lang en` | Langue des textes de signalement : `en` (par défaut) / `tr` / `ja` / `fr` |
| `--config config.json` | Applique un delta de configuration JSON (par-dessus `ValidatorConfig::default()`) |
| `--today 20260710` | Fixe la date « aujourd’hui » de l’analyse (pour les règles de calendrier) |

**Les filtres ne restreignent que l’affichage.** Les `notices` et les listes R2–R9 sont filtrées ; **le verdict de publication R1 et les scores R5 décrivent toujours l’intégralité du jeu de données**. Lorsqu’un filtre est actif, le JSON gagne un champ `filtered` et le résumé une ligne `filter:`.

`name_index` est **omis par défaut** : sur les gros jeux de données, les tables de coordonnées des arrêts et des tracés dominent la charge utile. Passez `--include-name-index` lorsque vous en avez besoin.

Passez `-` à la place d’un chemin pour lire le ZIP depuis **stdin** : `curl -sL <url> | gtfs-analyzer validate - --json`. (Le répertoire central du ZIP se situant en fin de fichier, l’archive est mise en mémoire tampon plutôt que traitée en flux.)

> **Les décomptes diffèrent de ceux de l’interface web.** Le navigateur plafonne le nombre de signalements conservés par règle pour des raisons de performance (les totaux réels figurent dans `capped_totals`). La CLI **n’applique aucun plafond** — le même jeu de données produit davantage de signalements et des chiffres d’impact R9 non réduits. C’est attendu ; ne comparez pas les deux sorties décompte par décompte.

**Codes de sortie :** `0` aucun signalement · `1` signalements présents ou rapport `PARTIAL` · `2` erreur fatale, de configuration ou de fichier. Un rapport `PARTIAL` ignore sans danger les entrées indisponibles et poursuit les contrôles indépendants ; le JSON expose `status: "partial"`, `validation_status: "PARTIAL"` et la portée `partial`. `partial.skipped_checks` liste les familles de contrôles K4/K5/K6 et les règles individuelles ignorées faute de prérequis ; `partial.skipped_stages` est conservé pour les métadonnées d’étape grossières. Avec `--fail-on*`, `1` n’est renvoyé que pour un signalement correspondant ; les autres constats sont tout de même rapportés sans faire échouer l’exécution. En mode JSON, stdout ne contient que du JSON ; les erreurs vont vers stderr.

```bash
# Barrière CI : n’échouer que sur les violations officielles de la spécification GTFS
gtfs-analyzer validate feed.zip --fail-on-class spec

# Ne rapporter que les constats Spec (les scores décrivent toujours tout le jeu de données)
gtfs-analyzer validate feed.zip --class spec --json --pretty -o spec.json
```

```python
import json, subprocess

proc = subprocess.run(
    ["target/release/gtfs-analyzer", "validate", "feed.zip", "--json"],
    text=True, capture_output=True,
)
# exit 1 signifie « des signalements existent », pas un échec — n’utilisez PAS check=True
data = json.loads(proc.stdout)
if data["status"] == "fatal":
    raise SystemExit(f'{data["code"]}: {data["message"]}')
for n in data["notices"]:
    print(n["rule_id"], n["severity"], n["rule_class"])
```

### `rules` — registre des règles

Liste l’ensemble du registre des règles sans lancer de validation — pensé comme dictionnaire de règles pour les projets intégrateurs.

```bash
gtfs-analyzer rules --class spec --severity critical
gtfs-analyzer rules --rule STM_004 --json --pretty
```

Champs : `id`, `severity`, `class`, `authority_source`, `base_effort`, `blocks`, `title`.
Les filtres `--class` / `--severity` / `--min-severity` / `--rule` ont le même sens que dans `validate`.
`--lang` s’applique ici aussi (titres des règles).

### Langue de sortie

Le cœur de validation produit ses textes de signalement en turc ; `--lang en` / `--lang ja` / `--lang fr` les remplacent à l’aide des **mêmes dictionnaires de traduction que l’interface web**. Les identifiants de règles, les gravités et les classes (`CRITICAL`, `SPEC`) restent lisibles par machine dans toutes les langues — seuls `title`, `message` et `remediation` sont traduits.

Lorsqu’une règle n’a pas de traduction, la chaîne est : langue demandée → anglais → turc (le texte propre au cœur), de sorte que la sortie n’est jamais vide.

Les dictionnaires sont dérivés de `ui/src/locales/{en,ja,fr}.ts` vers `crates/cli/locales/*.json` par `npm run locales:export`, puis intégrés au binaire de la CLI. Si un fichier de locale est modifié sans relancer l’export, `locale-parity.test.ts` échoue en CI — les fichiers de locale restent l’unique source de vérité.

---

## Seuils d’analyse

Les seuils de validation sont personnalisables depuis la section **Seuils d’analyse** de l’écran de téléversement. Les valeurs modifiées prennent effet au prochain téléversement de ZIP ; le bouton de réinitialisation rétablit les valeurs par défaut.

### Classes de règles et source d’autorité

Chaque règle relève de l’une des quatre classes. La classe reflète la **source d’autorité** du signalement (son fondement de légitimité), afin que l’on puisse voir d’un coup d’œil s’il s’agit d’une véritable violation de la spécification GTFS ou d’un signal d’interopérabilité, de qualité ou d’analytique :

- **Spec** — uniquement les cas que la **référence officielle GTFS Schedule** exige, interdit ou rend invalides de façon explicite (champs obligatoires, conditionnellement obligatoires ou conditionnellement interdits, valeurs d’énumération, clés étrangères, unicité, contraintes de format). Aucune autre source ne produit de `Spec`.
- **Interop** — signaux de compatibilité avec le comportement des consommateurs et validateurs tels que MobilityData, Google Transit ou un profil régional (par exemple GTFS-JP).
- **Quality** — bonnes pratiques GTFS, qualité des données, lisibilité, cohérence et contrôles de qualité de production.
- **Analytics** — signaux statistiques, opérationnels, de performance ou orientés analyse.

Chaque règle porte également un champ **source d’autorité** lisible par machine (`authority_source` : `GTFS_SPEC`, `MOBILITYDATA_PARITY`, `REGIONAL_PROFILE`, `PROJECT_QUALITY`, etc.). Invariant : **la classe `Spec` n’est légitime qu’avec `authority_source = GTFS_SPEC`** ; la parité avec MobilityData/Guru/Google, les bonnes pratiques ou les heuristiques propres au projet ne constituent pas à elles seules une preuve de Spec.

### Profils optionnels et URL source

Mettre `stop_name_best_practices=true` dans le delta de configuration active les contrôles `STP_040` et `STP_041`, dépendants de la langue ; ils sont désactivés par défaut en raison de leur risque de faux positifs. Les intégrations basées sur une URL peuvent fournir une métadonnée `source_url`, ce qui permet à `ARC_028` de vérifier que l’URL de publication permanente contient un nom de fichier `.zip`. La validation par simple téléversement ignore ce contrôle. Le moteur ne requête jamais les URL trouvées dans un jeu de données ; les contrôles de disponibilité HTTP exigent un adaptateur en ligne distinct et explicitement activé.

### Coordination des champs de distance de tracé

Si une course utilise `shape_dist_traveled` dans `stop_times.txt` alors que certains points du tracé référencé n’ont pas ce même champ dans `shapes.txt`, l’analyseur émet `SHP_030` (Quality · Moyenne). Les deux champs étant individuellement facultatifs en GTFS, il ne s’agit pas d’un bloquant de publication Spec ; c’est un signal de compatibilité au niveau du tracé indiquant que les consommateurs pourraient ne pas placer les arrêts de façon fiable sur le tracé. Le signalement inclut le nombre de courses concernées et des identifiants de courses représentatifs.

Un tracé à un seul point réellement référencé par une course est signalé comme `SHP_006` en Faible · Quality, avec `shape_id` et `shape_point_count=1` dans les détails. Un segment rectiligne à deux points est valide. Un tracé à un point inutilisé n’est signalé que par `SHP_018`. Il s’agit d’une correspondance de quasi-parité volontaire avec le `single_shape_point` de MobilityData : Analyzer ne signale `SHP_006` que pour un tracé utilisé.

### Parité sur la vitesse entre arrêts éloignés

La page de règles actuelle de MobilityData est incohérente en interne au sujet de `fast_travel_between_far_stops` : le tableau principal WARNING l’affiche comme active, tandis que les métadonnées de détail indiquent `Deprecated since undefined` et que le tableau des règles dépréciées l’omet. L’audit #115 a échantillonné 20 jeux de données positifs ; la décision se fonde sur la combinaison hétérogène et bruitée de distances cumulées supérieures à 10 km, de couples d’arrêts non consécutifs et de cascades d’horaires — et non sur une dépréciation supposée. L’aliasing vers `STM_012` ou `STM_014` a été rejeté ; aucune règle nouvelle n’a été ajoutée et l’écart demeure une lacune de couverture Analytics assumée.

### Spécificité de l’URL d’arrêt

`STP_034` et `STP_035` comparent `stop_url` aux URL d’agence et de ligne au moyen d’une identité syntaxique conservatrice, et produisent des constats Quality de faible priorité. La casse du schéma et de l’hôte, la racine `/` et les ports par défaut explicites (HTTP 80 / HTTPS 443) sont équivalents ; les chaînes de requête, les fragments, les barres obliques finales de chemin et l’encodage pourcent restent significatifs. Les arrêts partageant une même URL normalisée sont regroupés en un signalement agrégé unique, avec le nombre d’arrêts concernés et des identifiants représentatifs dans `details`.

### Seuils de vitesse

| Paramètre | Défaut | Plage | Description |
|---|---:|---|---|
| Vitesse max. bus | 120 km/h | 60–200 | Vitesse maximale autorisée pour les courses de bus |
| Vitesse max. tramway | 100 km/h | 40–160 | Vitesse maximale autorisée pour les courses de tramway |
| Vitesse max. métro | 150 km/h | 80–250 | Vitesse maximale autorisée pour les courses de métro |
| Vitesse max. train | 300 km/h | 100–400 | Vitesse maximale autorisée pour les courses ferroviaires |
| Vitesse max. ferry | 80 km/h | 20–150 | Vitesse maximale autorisée pour les courses de ferry |
| Vitesse max. téléphérique | 30 km/h | 10–60 | Vitesse maximale autorisée pour un téléphérique ou un funiculaire |

### Seuils géographiques et de correspondance

| Paramètre | Défaut | Plage | Description |
|---|---:|---|---|
| Temps de correspondance min. | 180 s | 30–1800 | Temps de correspondance minimal |
| Distance de correspondance max. | 500 m | 50–2000 | Distance maximale pour qu’une correspondance soit considérée comme valide |
| Saut de tracé max. | 10 km | 1–50 | Distance maximale entre deux points de tracé consécutifs |
| Seuil d’arrêts proches | 5 m | 1–20 | Les arrêts plus proches que cette distance sont signalés comme doublons |
| Distance arrêt–tracé | 100 m | 20–500 | Distance maximale autorisée entre un arrêt et son tracé |
| Distance à la station parente | 100 m | 10–1000 | Distance maximale autorisée entre un arrêt et sa station parente |

### Seuils de service et d’exploitation

| Paramètre | Défaut | Plage | Description |
|---|---:|---|---|
| Alerte d’expiration | 30 jours | 1–60 | Alerte émise si le jeu de données expire dans ce délai |
| Alerte d’expiration Feed Info | 7 jours | 1–60 | Horizon `FIN_019` par défaut pour `feed_info.feed_end_date` ; la parité MobilityData à 30 jours s’applique avec `feed_info_expiry_warning_days=30`, distinct de `CAL_008` |
| Seuil d’interruption de service | 7 jours | 3–30 | Les interruptions de service plus longues sont signalées |
| Durée de course max. | 24 h | 8–72 | Durée maximale d’une même course |
| Durée de course min. | 60 s | 10–300 | Durée minimale d’une même course |
| Intervalle max. | 240 min | 60–720 | Les intervalles plus longs déclenchent une alerte |
| Seuil d’effet accordéon | 2 min | 1–10 | Les intervalles plus courts sont signalés comme effet accordéon |

---

## Scores

### Score de publication (0–100)

Mesure la publiabilité du jeu de données au regard de la référence officielle GTFS Schedule. Le score **part de 100** ; chaque problème bloquant la publication retranche une pénalité proportionnelle au poids de la règle et au coût de correction.

**Mode de calcul :**
- Seuls les problèmes de classe `Spec` et de gravité `Critique` (la barrière officielle de la spécification GTFS) affectent le score de publication. Les signaux de compatibilité `Interop` sont rapportés séparément (score Interop / R8).
- Si une même règle se déclenche plusieurs fois, la pénalité est plafonnée à **2×** ; un problème isolé ne peut pas ramener le score à zéro.
- **0–40 :** le jeu de données est probablement inexploitable. Des erreurs bloquantes sont présentes.
- **40–70 :** problèmes partiels ; certaines applications peuvent rejeter le jeu de données.
- **70–90 :** exploitable, mais nécessite de l’attention.
- **90–100 :** prêt à publier.

### Score global (0–100)

Moyenne pondérée des quatre classes d’analyse : Spec×40 % + Interop×30 % + Quality×20 % + Analytics×10 %. Reflète à la fois la conformité à la spécification et la qualité opérationnelle des données. Un jeu de données peut être publiable tout en ayant un score global faible.

**Mode de calcul :**
- Les problèmes des quatre classes (Spec, Interop, Quality, Analytics) affectent ce score selon leurs poids respectifs.
- Champs facultatifs manquants, schémas de service incohérents et lacunes d’accessibilité s’y reflètent via les composantes Quality et Analytics.
- **0–60 :** problèmes de qualité importants ; l’expérience voyageur peut en pâtir.
- **60–80 :** qualité moyenne ; améliorations recommandées.
- **80–100 :** bonne qualité de données.

> **Remarque :** le score de publication et le score global répondent à des objectifs différents et reposent sur des formules différentes. Un jeu de données au score de publication élevé mais au score global faible fonctionne techniquement, mais des problèmes tels que des informations d’accessibilité manquantes ou des noms de ligne incorrects affectent les voyageurs.

---

## Onglets du rapport

### 1. Rapport
Vue de synthèse : les deux scores, les métriques du jeu de données (nombre de lignes, nombre de courses, plage de dates, etc.) et un graphique de répartition des signalements.

### 2. Détails et correction
Les problèmes sont présentés sous forme de file de correction priorisée, triée par score de priorité. Chaque ligne contient :

| Colonne | Description |
|---|---|
| **Score** | Score de priorité — calculé comme `Gravité × (1 + Dépendantes) × log₂(1 + Nombre) / Effort` ; plus élevé = à corriger en premier |
| **+Pub** | Gain de score de publication si cette règle est corrigée |
| **+Score** | Gain de score global si cette règle est corrigée |
| **Dépendantes** | Nombre d’autres règles actives qui se ferment automatiquement si celle-ci est corrigée |
| **Effort** | Effort de correction : 1 = modification d’un seul champ, 2 = portée limitée multi-fichiers, 3 = révision structurelle ou du modèle de données |

La somme de toutes les valeurs +Pub égale `100 − score de publication actuel` ; la somme de toutes les valeurs +Score égale `100 − score global actuel`. Les problèmes géographiques affichent une icône de carte ; un clic présente l’emplacement du problème et les données de tracé ou d’arrêt associées sur une carte interactive. Un clic sur le **code de règle** ouvre la section pertinente de la spécification GTFS dans un nouvel onglet — la page de référence du fichier le plus concerné par le signalement (gtfs.jp pour les règles GTFS-JP).

### 3. Par catégorie
Toutes les violations de règles listées par groupe et par classe. Chaque ligne indique le code de règle, le titre, le nombre d’enregistrements concernés, la gravité et l’aide à la correction. Le filtrage et le tri sont possibles.

### 4. Exporter
Téléchargez le rapport en HTML, CSV ou JSON. L’option PDF ouvre la boîte de dialogue d’impression du navigateur — utilisez « Enregistrer en PDF » pour exporter.

---

## Carte interactive des fichiers GTFS

GTFS Validator & Analyzer intègre une carte des fichiers interactive qui associe la structure des données GTFS aux signalements réels de validation du jeu de données analysé.

Cette vue n’est pas un schéma statique. Elle montre les fichiers présents dans le jeu de données, ceux qui manquent, les signalements et les relations entre fichiers validées, sur la base du résultat de l’analyse.

### Fonctionnalités

- Affiche les sept fichiers GTFS principaux dans les groupes **Calendrier** et **Service principal**
- N’affiche les fichiers standard non principaux que lorsque l’analyseur y rapporte un signalement
- Liste dans un groupe distinct les fichiers hors spécification trouvés dans le jeu de données
- Visualise les relations GTFS validées telles que `route_id`, `trip_id`, `stop_id`, `service_id` et `shape_id`
- Colore les fichiers selon la gravité la plus élevée de leurs signalements
- Distingue les fichiers manquants, conformes et problématiques
- Affiche le nombre d’enregistrements, la taille du fichier, le nombre de signalements et la répartition par gravité
- Liste les signalements par règle, toujours dans l’ordre **Critique → Élevée → Moyenne → Faible → Info**
- Ouvre tous les signalements du fichier sélectionné dans une vue Détails et correction filtrée
- Propose des filtres de présence de fichier et de gravité
- Prend en charge le zoom, l’ajustement à l’écran, le thème sombre et une mise en page mobile

Lorsqu’un fichier est sélectionné, seules ses connexions GTFS validées et associées se déploient. Les fichiers hors spécification restent visibles, mais aucune relation non validée n’est tracée.

L’analyse et la visualisation s’exécutent entièrement dans le navigateur. Les fichiers GTFS ne sont jamais téléversés vers un serveur.

![Carte des fichiers GTFS](docs/images/gtfs-file-map.png)

---

## Comparaison entre deux analyses

GTFS Validator & Analyzer peut comparer deux analyses d’un même jeu de données (avant/après) pour montrer ce qu’une série de corrections a amélioré et ce qu’elle a dégradé. Ouvrez **Comparer** et téléversez le **Golden JSON** téléchargé lors d’une analyse antérieure ; l’écart est calculé par rapport à l’exécution en cours.

### Fonctionnalités

- Affiche l’évolution avant/après des scores de publication, global et des sous-scores (Spec, Interop, Quality, Analytics)
- Classe chaque règle en **Corrigée, En baisse, En hausse, Nouvelle ou Identique**, avec filtrage et recherche
- Montre l’évolution de la répartition par gravité (Critique → Info) et par classe (Spec/Interop/Quality/Analytics)
- Compare la structure du jeu de données (nombre de courses, d’arrêts, d’enregistrements `stop_times` et `calendar_dates`) ainsi que les plages de dates du jeu de données et du service
- Normalise la densité de signalements **pour 1 000 courses** et **pour 100 000 horaires d’arrêt**, afin de rendre comparables des jeux de données de tailles différentes
- Avertit lorsque les deux exécutions diffèrent par le nom du jeu de données, la plage de dates ou la configuration, pour éviter une lecture trompeuse de l’écart
- Exporte la comparaison en CSV
- Lit également les schémas Golden hérités (v1–v3)

La comparaison s’exécute entièrement dans le navigateur. Le Golden JSON est analysé localement ; rien n’est téléversé vers un serveur.

---

## Classes de règles

| Classe | Ce qu’elle mesure | Influe sur |
|---|---|---|
| **Spec** | Écarts par rapport à la spécification GTFS — champs obligatoires manquants, valeurs invalides, erreurs d’intégrité référentielle | Score de publication |
| **Interop** | Conforme à la spécification mais rejeté ou mal interprété par les consommateurs courants (Google Maps, Apple Plans, etc.) | Score de publication |
| **Quality** | Champs facultatifs mais attendus manquants, incohérences, écarts aux bonnes pratiques | Score global |
| **Analytics** | Analyse des schémas de service — effet accordéon, service clairsemé, service expiré | Score global |

---

## Niveaux de gravité

| Niveau | Signification |
|---|---|
| **Critique** | Rend le jeu de données inexploitable ou provoque une perte de données |
| **Élevée** | Problème fonctionnel important ; correction fortement recommandée |
| **Moyenne** | Incohérence méritant attention |
| **Faible** | Écart mineur aux bonnes pratiques |
| **Info** | Informatif ; une action n’est pas nécessairement requise |

La gravité combine le niveau d’exigence du fichier ou du champ (Obligatoire · Conditionnellement obligatoire · Recommandé · Facultatif) issu de la [référence GTFS Schedule](https://gtfs.org/documentation/schedule/reference/#file-requirements) avec l’impact sémantique du signalement.

### Grille de gravité Spec

La gravité Spec combine niveau d’exigence et impact sémantique ; elle ne dépend pas du fait que MobilityData
qualifie le même constat d’`ERROR`, `WARNING` ou `INFO` :

- **Critique :** fichier ou champ obligatoire, intégrité de clé primaire ou étrangère, ou violation de type ou de plage fondamentale ; le jeu de données ne peut pas être consommé de façon fiable et `Spec + Critique` constitue la barrière de publication.
- **Élevée :** violation normative directe qui modifie sensiblement la sémantique des horaires, des tarifs, de l’accessibilité, du Flex ou des cheminements, même si le jeu de données reste analysable.
- **Moyenne :** violation normative localisée ou conditionnelle, le modèle de données principal restant lisible.
- **Faible :** écart normatif restreint portant sur des métadonnées ou un champ facultatif ; ne bloque pas la publication.
- **Info :** non utilisé pour les violations normatives Spec ; réservé aux signaux de mesure ou de contexte.

Aucune règle `Spec` ne peut donc avoir la gravité `Info`. L’audit du 2026-08-09 a passé en revue les 307
règles Spec et a fait passer les règles de journée de service brute `STM_048` et `STM_049` de Info à Élevée.
Voir l’[audit complet de la gravité Spec](docs/audits/spec-severity-rubric-2026-08-09.md).

Pour les jeux de données GTFS-JP, les règles du groupe **JPN** se fondent sur la [spécification GTFS-JP](https://www.gtfs.jp/) officielle (gtfs.jp).

---

## Plafonds de signalements

Dans les gros jeux de données, une même règle peut se déclencher des milliers de fois. Des listes de signalements illimitées saturent la mémoire du navigateur et nuisent à la lisibilité. Un plafond à deux niveaux est appliqué :

| Plafond | Valeur | Portée |
|---|---|---|
| Par règle (défaut) | 500 | Toutes les règles |
| Par règle (élevé) | 2 000 | `TRP_020`, `OPR_007`, `STP_016`, `STP_017` |
| Total (toutes règles) | 100 000 | À l’échelle du jeu de données — la validation s’arrête si ce total est dépassé |

Les règles du plafond élevé produisent naturellement de grands décomptes sur des jeux de données réels (par exemple un enregistrement d’intervalle par course). Lorsqu’une règle atteint son plafond, le nombre réel de violations apparaît dans la colonne **Total** de la file de correction ; dans l’onglet Tous les signalements, la sélection de ce filtre de règle affiche une bannière d’avertissement jaune.

---

## Groupes de règles

Chaque règle est codée `GROUPE_NNN`. Les groupes suivent les limites des fichiers et des composants GTFS.

| Groupe | Composant GTFS | Description |
|---|---|---|
| **ARC** | Archive / niveau fichier | Extraction du ZIP, format de fichier, présence des fichiers obligatoires, encodage des caractères |
| **AGN** | `agency.txt` | Informations sur l’agence et cohérence multi-agences |
| **CAL** | `calendar.txt` | Calendriers de service et schémas hebdomadaires |
| **CLD** | `calendar_dates.txt` | Dates d’exception de service et validité des dates |
| **STP** | `stops.txt` | Emplacements des arrêts, hiérarchie et informations d’accessibilité |
| **RTS** | `routes.txt` | Définition des lignes, type de ligne, couleur et nommage |
| **TRP** | `trips.txt` | Définition des courses, associations d’enchaînement et de tracé |
| **STM** | `stop_times.txt` | Horaires d’arrêt, vitesse, séquence et cohérence des horaires |
| **SHP** | `shapes.txt` | Tracés des lignes, ordre des points et alignement des arrêts |
| **FRQ** | `frequencies.txt` | Courses basées sur la fréquence et valeurs d’intervalle |
| **TRF** | `transfers.txt` | Définition des correspondances, types et validité des durées |
| **FAR** | `fare_attributes.txt` | Définition des tarifs, devise et moyen de paiement |
| **FRL** | `fare_rules.txt` | Règles tarifaires par ligne et par zone |
| **FIN** | `feed_info.txt` | Informations sur l’éditeur, langue, dates de validité |
| **PTH** | `pathways.txt` | Réseau de cheminements en station et connexions d’accessibilité |
| **LVL** | `levels.txt` | Niveaux de station et relations ascenseur/escalier |
| **TRN** | `translations.txt` | Traductions de champs et cohérence linguistique |
| **ATR** | `attributions.txt` | Informations de source et d’attribution des données |
| **XFL** | Inter-fichiers | Intégrité référentielle et cohérence entre fichiers |
| **GEO** | Analyse géographique | Cohérence des coordonnées, détection de valeurs aberrantes, regroupement |
| **OPR** | Analyse opérationnelle | Temps d’attente entre courses, densité des lignes, répétition d’arrêts |
| **VAT** | Topologie du réseau | Arrêts isolés, lignes déconnectées, accessibilité du réseau |
| **DQ** | Qualité globale du jeu de données | Métriques générales de qualité des données et contrôles de seuils |
| **RCT** | `rider_categories.txt` | Catégories de voyageurs, tranches d’âge et catégorie par défaut (Tarifs v2) |
| **FMD** | `fare_media.txt` | Supports de paiement : carte physique, application mobile, EMV, etc. (Tarifs v2) |
| **FPD** | `fare_products.txt` | Produits tarifaires, montant, devise et associations support/catégorie (Tarifs v2) |
| **FLG** | `fare_leg_rules.txt` | Règles tarifaires par trajet et priorité (Tarifs v2) |
| **FLJ** | `fare_leg_join_rules.txt` | Règles joignant deux trajets séparés par une correspondance en un seul trajet tarifaire effectif (Tarifs v2) |
| **FTR** | `fare_transfer_rules.txt` | Règles tarifaires de correspondance et limites de durée (Tarifs v2) |
| **ARS** | `areas.txt` | Définition des zones géographiques (Tarifs v2) |
| **SAR** | `stop_areas.txt` | Correspondances arrêt–zone (Tarifs v2) |
| **NET** | `networks.txt` | Définition des réseaux (Tarifs v2) |
| **TFR** | `timeframes.txt` | Groupes de plages horaires et associations au calendrier de service (Tarifs v2) |
| **BKR** | `booking_rules.txt` | Règles de réservation à la demande, fenêtres de préavis et types de réservation (GTFS Flex) |
| **PDW** | Règles de fenêtre Flex | Cohérence des fenêtres horaires de prise en charge/dépose à la demande dans `stop_times.txt` (GTFS Flex) |
| **LOC** | `locations.geojson` | Validation de la géométrie et du format des zones de service flexible (GTFS Flex) |
| **GGL** | Spécifique à Google Transit | Règles supplémentaires exigées ou restreintes par Google Maps et Google Transit |
| **JPN** | Profil GTFS-JP | Règles du profil national japonais GTFS-JP — lectures kana, intégrité référentielle de `office_jp.txt`/`agency_jp.txt` (jeux de données GTFS-JP uniquement) |

---

## Installation pour le développement

### Prérequis

- **Rust** — chaîne d’outils GNU (`stable-x86_64-pc-windows-gnu`), MinGW gcc
- **wasm-pack** — outil de construction WASM
- **Node.js** — une version LTS maintenue (plage exacte dans `ui/package.json` > `engines`)

> **Remarque Windows :** la chaîne d’outils GNU est requise à la place de MSVC. Lors de la construction WASM, `wasm-opt` est téléchargé et cette étape est incompatible avec l’éditeur de liens MSVC. Le `gcc` MinGW doit se trouver dans le PATH.

```powershell
# Chaîne d’outils Rust GNU (une seule fois)
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup override set stable-x86_64-pc-windows-gnu
```

### Construction

```powershell
# 1. Installer les dépendances
cd ui
npm install

# 2. Compiler le WASM
npm run wasm

# 3. Compiler l’interface
npm run build
# Sortie : ui/dist/
```

### Serveur de développement

```powershell
cd ui
npm install
npm run dev
```

### Tests

```powershell
# Tests unitaires et d’intégration Rust
cargo test

# Lint bloquant pour chaque crate, test et exemple de l’espace de travail
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Tests de fumée Playwright
cd ui
npx playwright test
```

## Structure du projet

```
gtfs-validator/
├── crates/
│   ├── config/     # Types de configuration
│   ├── core/       # Structures de données partagées et modèle de résultat
│   ├── pipeline/   # Pipeline de validation (étapes k1–k7)
│   ├── rules/      # Définition des règles et registre (611 règles, 38 groupes)
│   └── wasm/       # Sortie WASM wasm-bindgen
├── spec-audit/     # Table des champs générée depuis la spécification (barrière d’ancrage)
└── ui/             # Interface Vite + TypeScript
    ├── pkg/          # Sortie wasm-pack (générée, versionnée)
    ├── src/
    │   └── pages/    # Onglets de l’application (domain/fix/rules/export)
    └── tests/        # Tests Playwright
```

## Licence

MIT — voir [LICENSE](LICENSE) pour les détails.
