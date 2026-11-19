# Critères d’analyse, scores et plafonds de signalements

🇫🇷 **Français** · [Retour au README](../README.fr.md)

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

Un tracé à un seul point réellement référencé par une course est signalé comme `SHP_006` en Faible · Quality, avec `shape_id` et `shape_point_count=1` dans les détails. Un segment rectiligne à deux points est valide. Un tracé à un point inutilisé n’est signalé que par `SHP_018`.

### Spécificité de l’URL d’arrêt

`STP_034` et `STP_035` comparent `stop_url` aux URL d’agence et de ligne au moyen d’une identité syntaxique conservatrice, et produisent des constats Quality de faible priorité. La casse du schéma et de l’hôte, la racine `/` et les ports par défaut explicites (HTTP 80 / HTTPS 443) sont équivalents ; les chaînes de requête, les fragments, les barres obliques finales de chemin et l’encodage pourcent restent significatifs. Les arrêts partageant une même URL normalisée sont regroupés en un signalement agrégé unique, avec le nombre d’arrêts concernés et des identifiants représentatifs dans `details`.

### Seuils

Une partie des règles fonctionne avec des seuils réglables : vitesse maximale par type de ligne (bus 120 km/h, tramway 100, métro 150, train 300, ferry 80, téléphérique 30), durée et distance de correspondance, saut de tracé, arrêts trop proches (5 m), distance d’un arrêt à son tracé et à sa station parente, interruption de service, durée de course, intervalle et alerte d’expiration du jeu de données.

Les seuils se modifient dans le panneau **Critères d’analyse** de l’application web, avec `--config config.json` en CLI, et avec l’objet `config` dans le SDK et le paquet Python. Toutes les clés, valeurs par défaut et unités : [référence de configuration](../sdk/README.md#configuration-reference).

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

## Plafonds de signalements

Dans les gros jeux de données, une même règle peut se déclencher des milliers de fois. Des listes de signalements illimitées saturent la mémoire du navigateur et nuisent à la lisibilité. Un plafond à deux niveaux est appliqué :

| Plafond | Valeur | Portée |
|---|---|---|
| Par règle (défaut) | 500 | Toutes les règles |
| Par règle (élevé) | 2 000 | `TRP_020`, `OPR_007`, `STP_016`, `STP_017` |
| Total (toutes règles) | — | Pas de limite d’arrêt de 100 000 signalements à l’échelle du jeu de données |

Les règles du plafond élevé produisent naturellement de grands décomptes sur des jeux de données réels (par exemple un enregistrement d’intervalle par course). Lorsqu’une règle atteint son plafond, le nombre réel de violations apparaît dans la colonne **Total** de la file de correction ; dans l’onglet Tous les signalements, la sélection de ce filtre de règle affiche une bannière d’avertissement jaune.
