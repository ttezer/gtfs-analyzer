# GTFS Validator & Analyzer — Liste des règles

🇹🇷 [Türkçe](RULES.md) · 🇬🇧 [English](RULES.en.md) · 🇯🇵 [日本語](RULES.ja.md) · 🇫🇷 **Français**

612 règles, 38 groupes. Chaque règle est identifiée par un ID unique, un niveau de gravité et une classe.
Niveaux de gravité : **CRITIQUE** (bloquant pour la publication) · **ÉLEVÉE** · **MOYENNE** · **FAIBLE** · **INFO**
Classes : **Spec** (validité GTFS) · **Interop** (interopérabilité GTFS) · **Quality** (qualité GTFS) · **Analytics** (analytique GTFS)

---

## ARC — Archive / niveau fichier

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| ARC_001 | Impossible d’ouvrir l’archive ZIP | CRITIQUE | Spec |
| ARC_002 | Fichier illisible en UTF-8 | CRITIQUE | Quality |
| ARC_003 | Erreur d’encodage UTF-8 dans un fichier optionnel | MOYENNE | Quality |
| ARC_004 | Fichier obligatoire manquant | CRITIQUE | Spec |
| ARC_006 | Fichier GTFS optionnel présent | INFO | Quality |
| ARC_007 | Fichier non GTFS non reconnu | INFO | Quality |
| ARC_008 | Fichier de calendrier manquant (calendar.txt et calendar_dates.txt) | CRITIQUE | Spec |
| ARC_031 | feed_info.txt manquant alors que translations.txt est présent | CRITIQUE | Spec |
| ARC_009 | Le fichier facultatif ne contient aucun enregistrement de données | INFO | Quality |
| ARC_010 | Le fichier contient une BOM UTF-8 | MOYENNE | Quality |
| ARC_011 | Taille du fichier (info) | INFO | Analytics |
| ARC_012 | Le nombre de colonnes ne correspond pas à l’en-tête | CRITIQUE | Spec |
| ARC_013 | Erreur d’analyse CSV | CRITIQUE | Spec |
| ARC_014 | Espaces en début ou en fin dans l’en-tête | MOYENNE | Quality |
| ARC_015 | Colonne d’en-tête en double | CRITIQUE | Interop |
| ARC_025 | Colonne obligatoire absente de l’en-tête | CRITIQUE | Spec |
| ARC_017 | Colonne inconnue (non définie dans la spécification GTFS) | INFO | Quality |
| ARC_018 | Enregistrement de données vide | MOYENNE | Quality |
| ARC_019 | Nom de colonne vide dans l’en-tête | ÉLEVÉE | Quality |
| ARC_020 | Fichier GTFS recommandé manquant (shapes.txt ou feed_info.txt) | FAIBLE | Quality |
| ARC_021 | Caractère non imprimable ou problématique dans un champ | FAIBLE | Quality |
| ARC_022 | Le nombre d’enregistrements dépasse la limite de 1 000 000 | FAIBLE | Quality |
| ARC_023 | Fichier ZIP imbriqué dans l’archive GTFS | MOYENNE | Quality |
| ARC_024 | Fichier GTFS .txt dans un sous-répertoire (il ne sera pas analysé) | MOYENNE | Spec |
| ARC_026 | Caractères de fin de ligne mal formés | MOYENNE | Spec |
| ARC_027 | L’entrée ZIP n’a pas de droit de lecture utilisateur | INFO | Quality |
| ARC_028 | L’URL de publication GTFS ne se termine pas par un fichier .zip | FAIBLE | Quality |
| ARC_029 | Protection anti-bombe zip déclenchée | CRITIQUE | Quality |
| ARC_030 | Tabulation ou saut de ligne dans la valeur d’un champ | ÉLEVÉE | Spec |
| ARC_032 | Balise HTML ou séquence d’échappement dans la valeur d’un champ | ÉLEVÉE | Spec |
| ARC_033 | Guillemet non échappé dans la valeur d’un champ (RFC 4180) | ÉLEVÉE | Spec |
| ARC_034 | Ligne d’en-tête répétée comme enregistrement de données | ÉLEVÉE | Spec |
| ARC_035 | Fichier obligatoire vide | CRITIQUE | Spec |

## BKR — Règles de réservation

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| BKR_001 | Champ de réservation en jour antérieur défini dans un contexte interdit | ÉLEVÉE | Spec |
| BKR_002 | prior_notice_start_day valide uniquement avec prior_notice_last_day | MOYENNE | Quality |
| BKR_003 | prior_notice_start_time valide uniquement avec prior_notice_start_day | ÉLEVÉE | Spec |
| BKR_004 | Champs prior_notice interdits pour la réservation en temps réel | ÉLEVÉE | Spec |
| BKR_005 | prior_notice_duration_max valide uniquement avec booking_type=1 | MOYENNE | Spec |
| BKR_006 | prior_notice_duration_min invalide (≤ 0 ou non numérique) | ÉLEVÉE | Spec |
| BKR_007 | prior_notice_duration_min obligatoire pour booking_type=1 | CRITIQUE | Spec |
| BKR_008 | prior_notice_last_day obligatoire pour booking_type=2 | CRITIQUE | Spec |
| BKR_009 | prior_notice_last_time obligatoire pour booking_type=2 | CRITIQUE | Spec |
| BKR_010 | prior_notice_start_time obligatoire lorsque prior_notice_start_day est défini | ÉLEVÉE | Spec |
| BKR_011 | prior_notice_last_day > prior_notice_start_day : fenêtre de réservation invalide | ÉLEVÉE | Interop |
| BKR_012 | prior_notice_duration_min interdit avec booking_type=2 | MOYENNE | Spec |
| BKR_013 | prior_notice_last_time valide uniquement avec prior_notice_last_day | ÉLEVÉE | Spec |
| BKR_014 | prior_notice_service_id valide uniquement avec booking_type=2 | ÉLEVÉE | Spec |
| BKR_015 | prior_notice_service_id introuvable (calendar/calendar_dates) | CRITIQUE | Spec |
| BKR_016 | booking_type manquant ou invalide | CRITIQUE | Spec |
| BKR_017 | pickup_booking_rule_id introuvable (booking_rules) | CRITIQUE | Spec |
| BKR_018 | drop_off_booking_rule_id introuvable (booking_rules) | CRITIQUE | Spec |
| BKR_019 | booking_rule_id manquant ou en double | CRITIQUE | Spec |
| BKR_024 | prior_notice_start_day interdit avec booking_type=1 et duration_max | MOYENNE | Spec |
| BKR_020 | booking_url invalide | MOYENNE | Spec |
| BKR_021 | info_url invalide | FAIBLE | Spec |
| BKR_022 | phone_number invalide | FAIBLE | Quality |
| BKR_023 | Le champ numérique prior_notice n’est pas un entier | MOYENNE | Spec |
| BKR_025 | Le champ horaire prior_notice n’est pas une heure valide | MOYENNE | Spec |

## AGN — Agence

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| AGN_002 | agency_name manquant | CRITIQUE | Spec |
| AGN_003 | agency_url manquant ou invalide | CRITIQUE | Spec |
| AGN_004 | agency_timezone manquant ou invalide | CRITIQUE | Spec |
| AGN_005 | Incohérence de fuseau horaire entre agences | MOYENNE | Spec |
| AGN_006 | agency_lang invalide | FAIBLE | Spec |
| AGN_007 | agency_phone invalide | FAIBLE | Quality |
| AGN_008 | agency_fare_url invalide | FAIBLE | Spec |
| AGN_009 | agency_email invalide | FAIBLE | Spec |
| AGN_010 | agency_id en double | CRITIQUE | Spec |
| AGN_011 | Plusieurs agences sans agency_id | CRITIQUE | Spec |
| AGN_012 | cemv_support invalide (agence) | FAIBLE | Spec |
| AGN_013 | Discordance entre la langue du jeu de données et celle de l’agence | FAIBLE | Interop |
| AGN_014 | Plusieurs agences mais agency_id manquant dans agency.txt | CRITIQUE | Spec |
| AGN_015 | agency_url utilise http non sécurisé | INFO | Quality |
| AGN_016 | agency_phone suspect ou factice | INFO | Quality |
| AGN_017 | agency_lang incohérent entre agences | FAIBLE | Interop |

## STP — Arrêts

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| STP_001 | stop_id en double | CRITIQUE | Spec |
| STP_002 | stop_id vide | CRITIQUE | Spec |
| STP_003 | stop_name manquant ou stop_lat hors plage | CRITIQUE | Spec |
| STP_004 | stop_lat non numérique | CRITIQUE | Spec |
| STP_005 | stop_lon invalide ou hors plage | CRITIQUE | Spec |
| STP_006 | stop_lat manquant | CRITIQUE | Spec |
| STP_007 | stop_lon manquant | CRITIQUE | Spec |
| STP_008 | location_type invalide | ÉLEVÉE | Spec |
| STP_009 | parent_station introuvable | CRITIQUE | Spec |
| STP_010 | parent_station n’est pas de location_type=1 | ÉLEVÉE | Spec |
| STP_011 | parent_station obligatoire pour une entrée, une sortie ou une zone d’embarquement | CRITIQUE | Spec |
| STP_012 | Station ou entrée utilisée dans stop_times | CRITIQUE | Spec |
| STP_013 | wheelchair_boarding invalide | FAIBLE | Spec |
| STP_014 | stop_timezone invalide | MOYENNE | Spec |
| STP_015 | level_id introuvable | CRITIQUE | Spec |
| STP_016 | Deux arrêts aux coordonnées identiques | MOYENNE | Quality |
| STP_017 | Deux arrêts trop proches l’un de l’autre | FAIBLE | Quality |
| STP_018 | Aucun arrêt trouvé | CRITIQUE | Spec |
| STP_019 | stop_name trop long | FAIBLE | Quality |
| STP_020 | Arrêt sans aucune course | MOYENNE | Analytics |
| STP_021 | Le parent de la zone d’embarquement n’est pas un quai | ÉLEVÉE | Quality |
| STP_022 | stop_code manquant | MOYENNE | Quality |
| STP_023 | tts_stop_name invalide | FAIBLE | Quality |
| STP_024 | stop_access hors de la plage de compatibilité K2 | INFO | Quality |
| STP_025 | stop_name comporte des espaces en début ou en fin | MOYENNE | Quality |
| STP_026 | Valeur stop_access invalide | FAIBLE | Spec |
| STP_027 | stop_access non défini sur une station avec cheminements | MOYENNE | Quality |
| STP_028 | stop_code trop long | INFO | Quality |
| STP_029 | Arrêt situé dans une station mais coordonnée trop éloignée | MOYENNE | Quality |
| STP_030 | La station parente n’a aucun arrêt enfant | MOYENNE | Quality |
| STP_031 | Le nom et la description de l’arrêt sont identiques | INFO | Quality |
| STP_032 | parent_station manquant pour un quai relié par cheminement | MOYENNE | Quality |
| STP_033 | zone_id de l’arrêt manquant (nécessaire au calcul tarifaire) | INFO | Quality |
| STP_034 | stop_url identique à l’URL de l’agence | FAIBLE | Quality |
| STP_035 | stop_url identique à l’URL de la ligne | FAIBLE | Quality |
| STP_036 | La station possède un parent_station (invalide) | FAIBLE | Spec |
| STP_037 | Certains arrêts n’indiquent pas l’accessibilité en fauteuil roulant | MOYENNE | Quality |
| STP_038 | Aucun arrêt n’indique l’accessibilité en fauteuil roulant | INFO | Quality |
| STP_039 | stop_code en double | FAIBLE | Quality |
| STP_040 | Le nom de l’arrêt contient un mot redondant (arrêt ou station) | FAIBLE | Quality |
| STP_044 | platform_code contient un mot redondant (quai ou voie) | FAIBLE | Quality |
| STP_041 | Le nom de l’arrêt enfant ne reprend pas celui de la station parente | FAIBLE | Quality |
| STP_042 | stop_url invalide | FAIBLE | Spec |
| STP_043 | stop_access utilisé dans un contexte interdit | MOYENNE | Spec |

## RTS — Lignes

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| RTS_001 | route_id en double | CRITIQUE | Spec |
| RTS_002 | agency_id introuvable | CRITIQUE | Spec |
| RTS_003 | route_short_name et route_long_name tous deux manquants | CRITIQUE | Spec |
| RTS_004 | route_type manquant ou invalide | CRITIQUE | Spec |
| RTS_005 | route_url invalide | MOYENNE | Spec |
| RTS_006 | route_color n’est pas une couleur hexadécimale valide | MOYENNE | Spec |
| RTS_007 | route_text_color n’est pas une couleur hexadécimale valide | MOYENNE | Spec |
| RTS_008 | Contraste insuffisant entre la couleur de la ligne et celle du texte | MOYENNE | Quality |
| RTS_010 | route_short_name trop long | FAIBLE | Quality |
| RTS_011 | route_long_name trop long | FAIBLE | Quality |
| RTS_012 | Ligne sans aucune course | MOYENNE | Quality |
| RTS_013 | continuous_pickup invalide | FAIBLE | Spec |
| RTS_016 | Ligne sans aucun jour de service actif | FAIBLE | Quality |
| RTS_017 | Ligne sans tracé défini | INFO | Quality |
| RTS_018 | continuous_drop_off invalide | FAIBLE | Spec |
| RTS_019 | Nom de ligne en double | MOYENNE | Quality |
| RTS_020 | La ligne et l’agence partagent la même URL | INFO | Quality |
| RTS_021 | route_short_name dépasse la limite Google Transit (6 caractères) | FAIBLE | Quality |
| RTS_022 | route_long_name contient route_short_name | FAIBLE | Quality |
| RTS_023 | route_desc reprend un nom de ligne | INFO | Quality |
| RTS_024 | cemv_support invalide (ligne) | FAIBLE | Spec |
| RTS_025 | agency_id vide dans routes.txt (recommandé) | INFO | Quality |
| RTS_026 | Nom court de ligne en double | INFO | Quality |
| RTS_027 | Nom long de ligne en double | INFO | Quality |
| RTS_028 | continuous_pickup/drop_off interdits sur une ligne Flex | ÉLEVÉE | Spec |
| RTS_029 | route_sort_order invalide | FAIBLE | Spec |
| RTS_030 | route_type utilise une valeur étendue hors de l’énumération principale | FAIBLE | Interop |
| RTS_031 | route_id manquant | CRITIQUE | Spec |

## TRP — Courses

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| TRP_001 | trip_id manquant ou en double | CRITIQUE | Spec |
| TRP_002 | route_id introuvable | CRITIQUE | Spec |
| TRP_003 | service_id introuvable | CRITIQUE | Spec |
| TRP_004 | shape_id introuvable | ÉLEVÉE | Spec |
| TRP_005 | direction_id invalide | MOYENNE | Spec |
| TRP_006 | wheelchair_accessible invalide | FAIBLE | Spec |
| TRP_007 | bikes_allowed invalide | FAIBLE | Spec |
| TRP_032 | cars_allowed invalide | FAIBLE | Spec |
| TRP_011 | Girouette de course non renseignée | ÉLEVÉE | Quality |
| TRP_012 | direction_id manquant sur une ligne bidirectionnelle | FAIBLE | Quality |
| TRP_013 | La ligne ne comporte qu’une seule course | FAIBLE | Quality |
| TRP_014 | trip_short_name trop long | INFO | Quality |
| TRP_015 | Course unique dans un groupe block_id | FAIBLE | Quality |
| TRP_017 | Course basée sur la fréquence absente de stop_times | MOYENNE | Quality |
| TRP_019 | shape_id manquant alors que le service continu est actif | ÉLEVÉE | Spec |
| TRP_020 | trip_headsign correspond au nom d’un arrêt intermédiaire | INFO | Analytics |
| TRP_021 | Transport des vélos (bikes_allowed) non précisé | INFO | Quality |
| TRP_022 | Chevauchement des horaires de courses au sein d’un enchaînement | ÉLEVÉE | Interop |
| TRP_023 | Aucune course active dans les 7 prochains jours | FAIBLE | Quality |
| TRP_024 | Type de ligne incohérent au sein de l’enchaînement | FAIBLE | Quality |
| TRP_025 | Forte proportion de courses sans information d’accessibilité en fauteuil roulant | INFO | Quality |
| TRP_026 | Course dont l’ensemble des dates actives est vide | MOYENNE | Analytics |
| TRP_028 | Certaines courses n’indiquent pas l’accessibilité en fauteuil roulant | MOYENNE | Quality |
| TRP_029 | Aucune course n’indique l’accessibilité en fauteuil roulant | INFO | Quality |
| TRP_031 | route_id manquant | CRITIQUE | Spec |
| TRP_033 | Des courses partageant un block_id portent des types de ligne différents | MOYENNE | Quality |
| TRP_034 | Le champ safe_duration n’est pas un nombre | MOYENNE | Spec |
| TRP_035 | trips.txt contient un service_id vide | CRITIQUE | Spec |

## STM — Horaires d’arrêt

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| STM_001 | trip_id introuvable | CRITIQUE | Spec |
| STM_002 | stop_id introuvable | CRITIQUE | Spec |
| STM_003 | Format de arrival_time invalide | CRITIQUE | Spec |
| STM_004 | Format de departure_time invalide | CRITIQUE | Spec |
| STM_005 | stop_sequence manquant ou invalide | CRITIQUE | Spec |
| STM_006 | stop_id manquant (stop_times) | CRITIQUE | Spec |
| STM_007 | Heure de départ antérieure à l’heure d’arrivée | ÉLEVÉE | Interop |
| STM_008 | L’heure diminue entre deux arrêts | CRITIQUE | Interop |
| STM_009 | pickup_type invalide | ÉLEVÉE | Spec |
| STM_010 | drop_off_type invalide | ÉLEVÉE | Spec |
| STM_012 | Vitesse irréaliste entre deux arrêts | ÉLEVÉE | Interop |
| STM_013 | Heures d’arrivée et de départ mélangées | ÉLEVÉE | Quality |
| STM_014 | Vitesse excessive sur un segment | ÉLEVÉE | Analytics |
| STM_015 | arrival_time manquant au premier arrêt | CRITIQUE | Spec |
| STM_016 | arrival_time manquant au dernier arrêt | CRITIQUE | Spec |
| STM_017 | Distance de tracé manquante dans les horaires d’arrêt | MOYENNE | Quality |
| STM_018 | continuous_pickup invalide (stop_times) | MOYENNE | Spec |
| STM_019 | continuous_drop_off invalide (stop_times) | MOYENNE | Spec |
| STM_020 | Temps de parcours nul (distance > 200 m) | ÉLEVÉE | Quality |
| STM_021 | Distance entre arrêts nulle ou négative | ÉLEVÉE | Quality |
| STM_022 | timepoint invalide | MOYENNE | Spec |
| STM_024 | Incohérence d’unité pour shape_dist_traveled | INFO | Quality |
| STM_025 | Durée de segment très courte | INFO | Analytics |
| STM_026 | Distance excessive entre deux arrêts | ÉLEVÉE | Quality |
| STM_028 | Durée de course trop longue | ÉLEVÉE | Analytics |
| STM_029 | Durée de course trop courte | MOYENNE | Analytics |
| STM_030 | shape_dist_traveled négatif ou non numérique | FAIBLE | Spec |
| STM_032 | stop_sequence en double au sein d’une course | FAIBLE | Spec |
| STM_033 | Course à un seul arrêt (inutilisable) | ÉLEVÉE | Interop |
| STM_034 | Une seule des heures d’arrivée ou de départ est définie | MOYENNE | Interop |
| STM_035 | Même arrêt desservi deux fois consécutivement (terminus ou boucle) | INFO | Analytics |
| STM_036 | stop_times non trié par trip_id + stop_sequence (unsorted_stop_times) | INFO | Quality |
| STM_037 | arrival_time/departure_time interdits dans une fenêtre Flex | ÉLEVÉE | Spec |
| STM_038 | start_pickup_drop_off_window >= end_pickup_drop_off_window | ÉLEVÉE | Interop |
| STM_039 | Fenêtre de prise en charge ou de dépose manquante en contexte Flex | CRITIQUE | Spec |
| STM_040 | pickup/drop_off_booking_rule_id manquant dans des stop_times Flex (Optionnel dans la spécification) | MOYENNE | Quality |
| STM_060 | Zone, fenêtre et prise en charge/dépose se chevauchent simultanément au sein d’une course | ÉLEVÉE | Spec |
| STM_061 | Vitesse impossible entre des arrêts éloignés non consécutifs | ÉLEVÉE | Interop |
| STM_059 | booking_rule_id recommandé lorsque pickup_type/drop_off_type=2 | FAIBLE | Quality |
| STM_041 | stop_id et location_id/group_id ne peuvent pas être utilisés ensemble | ÉLEVÉE | Spec |
| STM_042 | stop_headsign contient des caractères non pris en charge par Google Transit | FAIBLE | Interop |
| STM_043 | Course au nombre d’arrêts extrême (>200) | INFO | Analytics |
| STM_044 | Le fichier stop_times dépasse 2 000 000 d’enregistrements (avertissement de performance WASM) | INFO | Analytics |
| STM_045 | L’heure de départ de la course dépasse la fenêtre de la journée de service | MOYENNE | Quality |
| STM_046 | trip_id manquant | CRITIQUE | Spec |
| STM_047 | timepoint=1 sans heure d’arrivée ni de départ | CRITIQUE | Spec |
| STM_048 | Heures de journée de service après minuit écrites en 00:xx (spécification GTFS) | ÉLEVÉE | Spec |
| STM_049 | Départ après minuit écrit en 00:xx sur le même enregistrement (spécification GTFS) | ÉLEVÉE | Spec |
| STM_050 | Colonne timepoint présente mais valeur vide | FAIBLE | Quality |
| STM_051 | pickup_type 0/3 interdit avec une fenêtre Flex | ÉLEVÉE | Spec |
| STM_052 | drop_off_type 0 interdit avec une fenêtre Flex | ÉLEVÉE | Spec |
| STM_053 | De nombreux arrêts consécutifs ont la même heure | MOYENNE | Quality |
| STM_054 | continuous_pickup interdit avec une fenêtre Flex | ÉLEVÉE | Spec |
| STM_055 | continuous_drop_off interdit avec une fenêtre Flex | ÉLEVÉE | Spec |
| STM_056 | shape_dist_traveled n’augmente pas | CRITIQUE | Spec |
| STM_058 | Heure de fenêtre Flex de prise en charge ou de dépose invalide | CRITIQUE | Spec |

## PDW — Fenêtre de prise en charge/dépose

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| PDW_006 | Chevauchement de fenêtres de prise en charge/dépose pour un même couple course+zone | MOYENNE | Analytics |

## LOC — locations.geojson

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| LOC_001 | Type de géométrie inconnu ou invalide dans locations.geojson | ÉLEVÉE | Spec |
| LOC_002 | L’objet Feature a une géométrie nulle ou absente | CRITIQUE | Spec |
| LOC_003 | L’objet Feature n’a pas la propriété obligatoire « id » | CRITIQUE | Spec |
| LOC_004 | L’anneau du polygone n’est pas fermé | MOYENNE | Spec |
| LOC_005 | La FeatureCollection ne contient aucun objet Feature | FAIBLE | Quality |
| LOC_006 | La superficie du polygone dépasse 500 km² | MOYENNE | Quality |
| LOC_007 | Identifiant « id » d’objet Feature en double dans la FeatureCollection | MOYENNE | Spec |
| LOC_008 | Membre « type » absent ou différent de « Feature » | MOYENNE | Spec |
| LOC_009 | Objet « properties » manquant dans l’objet Feature | MOYENNE | Spec |
| LOC_011 | Polygone invalide : un anneau s’auto-intersecte ou un trou se situe à l’extérieur | ÉLEVÉE | Spec |
| LOC_010 | Le membre « coordinates » de la géométrie est absent ou n’est pas un tableau — la géométrie de la zone ne peut pas être résolue | CRITIQUE | Spec |

## CAL — Calendrier

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| CAL_001 | service_id en double | CRITIQUE | Spec |
| CAL_002 | Valeur invalide dans un champ de jour du calendrier | CRITIQUE | Spec |
| CAL_003 | start_date invalide | CRITIQUE | Spec |
| CAL_004 | end_date invalide | CRITIQUE | Spec |
| CAL_005 | start_date postérieure à end_date | CRITIQUE | Interop |
| CAL_006 | Tous les jours de la grille hebdomadaire sont désactivés | INFO | Quality |
| CAL_007 | Interruption dans la période de service | MOYENNE | Analytics |
| CAL_008 | Le service expire prochainement | ÉLEVÉE | Analytics |
| CAL_009 | Tous les services du jeu de données ont expiré | CRITIQUE | Quality |
| CAL_010 | Le service compte trop peu de jours actifs | MOYENNE | Analytics |
| CAL_011 | Service inutilisé | FAIBLE | Quality |
| CAL_012 | Interruption de service dans un futur proche | INFO | Analytics |
| CAL_013 | Période de service expirée | INFO | Analytics |
| CAL_014 | Dates de service hors de la plage de validité de feed_info | FAIBLE | Quality |
| CAL_015 | Toutes les dates du calendrier sont futures (aucune course active aujourd’hui) | FAIBLE | Quality |
| CAL_016 | Le service s’étend jusqu’à une date très lointaine | INFO | Quality |
| CAL_017 | Le calendrier n’a pas encore commencé (toutes les dates actives sont futures) | FAIBLE | Quality |
| CAL_018 | Le service n’a aucun jour de semaine actif (tous les jours à 0, aucun remplacé par calendar_dates) | FAIBLE | Quality |
| CAL_019 | Dates du calendrier de service hors de la fenêtre de validité de feed_info | FAIBLE | Quality |
| CAL_020 | La fenêtre de validité du jeu de données dépasse 5 ans | FAIBLE | Quality |
| CAL_021 | Actif aujourd’hui mais aucun service dans les jours à venir | INFO | Analytics |
| CAL_022 | service_id manquant | CRITIQUE | Spec |
| CAL_023 | end_date du calendrier très lointaine (suspect) | MOYENNE | Quality |
| CAL_024 | Calendrier inactif dans les 7 prochains jours | FAIBLE | Quality |
| CAL_025 | Champ de jour du calendrier vide | CRITIQUE | Spec |

## CLD — Dates de calendrier

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| CLD_001 | service_id manquant | CRITIQUE | Spec |
| CLD_002 | Format de date invalide | CRITIQUE | Spec |
| CLD_003 | exception_type manquant ou invalide | CRITIQUE | Spec |
| CLD_004 | Un service défini uniquement par calendar_dates n’a aucune date active | ÉLEVÉE | Quality |
| CLD_005 | Date hors plage | CRITIQUE | Quality |
| CLD_006 | Trop de jours d’exception | MOYENNE | Quality |
| CLD_007 | Nombre excessif d’exceptions de calendrier | INFO | Analytics |

## SHP — Tracés

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| SHP_001 | shape_id manquant | CRITIQUE | Spec |
| SHP_002 | shape_pt_lat manquant ou invalide | CRITIQUE | Spec |
| SHP_003 | shape_pt_lon manquant ou invalide | CRITIQUE | Spec |
| SHP_004 | shape_pt_sequence manquant ou invalide | CRITIQUE | Spec |
| SHP_005 | shape_dist_traveled diminue | CRITIQUE | Spec |
| SHP_006 | Le tracé ne comporte qu’un seul point | FAIBLE | Quality |
| SHP_008 | shape_pt_sequence en double | CRITIQUE | Spec |
| SHP_009 | Le tracé s’auto-intersecte | INFO | Analytics |
| SHP_010 | Point de tracé répété (coordonnées identiques consécutives) | FAIBLE | Quality |
| SHP_012 | Tracé trop éloigné des arrêts de la course | ÉLEVÉE | Analytics |
| SHP_014 | Premier ou dernier arrêt éloigné de l’extrémité du tracé | INFO | Analytics |
| SHP_015 | Le tracé comporte statistiquement trop peu de points | MOYENNE | Quality |
| SHP_016 | Sens du tracé incohérent avec le sens de la course | ÉLEVÉE | Quality |
| SHP_017 | La séquence d’arrêts est en conflit avec le tracé | INFO | Analytics |
| SHP_018 | Tracé référencé par aucune course | FAIBLE | Quality |
| SHP_019 | Les courses de ce tracé n’ont aucun horaire d’arrêt | MOYENNE | Quality |
| SHP_020 | Point répété dans le tracé | INFO | Analytics |
| SHP_021 | shape_dist_traveled négatif ou non numérique | FAIBLE | Quality |
| SHP_022 | Position de l’arrêt ambiguë sur le tracé | ÉLEVÉE | Quality |
| SHP_023 | Points consécutifs de même shape_dist_traveled aux mêmes coordonnées | MOYENNE | Quality |
| SHP_024 | Distance arrêt–tracé incohérente avec shape_dist_traveled | MOYENNE | Quality |
| SHP_025 | La distance des stop_times de la course dépasse la distance totale du tracé | MOYENNE | Quality |
| SHP_026 | Le tracé a un nombre de points extrême (>5 000) | INFO | Analytics |
| SHP_028 | Même shape_dist_traveled avec des coordonnées différentes | ÉLEVÉE | Spec |
| SHP_029 | Même shape_dist_traveled, écart de coordonnées infime | INFO | Quality |
| SHP_030 | stop_times utilise shape_dist_traveled mais les distances du tracé sont absentes | MOYENNE | Quality |

## FRQ — Fréquences

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| FRQ_001 | trip_id introuvable | CRITIQUE | Spec |
| FRQ_002 | start_time invalide | CRITIQUE | Spec |
| FRQ_003 | end_time invalide | CRITIQUE | Spec |
| FRQ_004 | headway_secs manquant ou invalide | CRITIQUE | Spec |
| FRQ_005 | end_time antérieure à start_time | CRITIQUE | Quality |
| FRQ_006 | headway_secs trop long | MOYENNE | Analytics |
| FRQ_007 | exact_times invalide | MOYENNE | Spec |
| FRQ_008 | headway_secs nul (fréquence invalide) | CRITIQUE | Spec |
| FRQ_009 | Intervalle de fréquence trop court | MOYENNE | Quality |
| FRQ_010 | Fréquence très élevée (risque d’effet accordéon) | INFO | Analytics |
| FRQ_011 | Périodes de fréquence qui se chevauchent | ÉLEVÉE | Spec |
| FRQ_012 | exact_times=1 avec end_time sur une limite d’intervalle | FAIBLE | Spec |

## TRF — Correspondances

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| TRF_001 | from_stop_id manquant | CRITIQUE | Spec |
| TRF_002 | to_stop_id manquant | CRITIQUE | Spec |
| TRF_003 | from_stop_id ou to_stop_id introuvable | CRITIQUE | Spec |
| TRF_004 | transfer_type invalide | ÉLEVÉE | Spec |
| TRF_005 | min_transfer_time manquant | CRITIQUE | Spec |
| TRF_006 | from_trip_id introuvable | CRITIQUE | Spec |
| TRF_007 | to_trip_id introuvable | CRITIQUE | Spec |
| TRF_008 | from_route_id introuvable | CRITIQUE | Spec |
| TRF_009 | to_route_id introuvable | CRITIQUE | Spec |
| TRF_010 | Temps de correspondance trop long | MOYENNE | Analytics |
| TRF_011 | Correspondance définie mais distance importante | INFO | Quality |
| TRF_012 | Enregistrement de correspondance en double | CRITIQUE | Spec |
| TRF_013 | Type de correspondance incohérent avec le contexte | CRITIQUE | Quality |
| TRF_014 | Aucune course pour une correspondance sans changement de véhicule | ÉLEVÉE | Spec |
| TRF_015 | Correspondance sans changement de véhicule invalide | ÉLEVÉE | Quality |
| TRF_016 | Conditions de correspondance contradictoires | CRITIQUE | Spec |
| TRF_017 | Correspondance de course sur la mauvaise ligne | ÉLEVÉE | Spec |
| TRF_018 | La correspondance de course référence la même course | MOYENNE | Quality |
| TRF_019 | route_type différent dans une correspondance sans changement de véhicule | MOYENNE | Interop |
| TRF_020 | Vitesse de marche trop élevée pour la correspondance | MOYENNE | Quality |
| TRF_021 | L’extrémité de la correspondance n’est ni un arrêt ni une station | CRITIQUE | Spec |
| TRF_022 | Calendriers contradictoires dans une continuation de course 1 vers n | ÉLEVÉE | Spec |
| TRF_023 | Calendriers contradictoires dans une continuation de course n vers 1 | ÉLEVÉE | Spec |

## GGL — Compatibilité Google Transit

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| GGL_001 | transfer_type=4/5 non pris en charge par Google Transit | FAIBLE | Interop |
| GGL_002 | Valeur ic_price invalide (spécifique à Google) | FAIBLE | Interop |

## FAR — Attributs tarifaires

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| FAR_001 | fare_id en double | CRITIQUE | Spec |
| FAR_002 | price manquant ou invalide | CRITIQUE | Spec |
| FAR_003 | currency_type manquant | CRITIQUE | Spec |
| FAR_004 | payment_method invalide | CRITIQUE | Spec |
| FAR_005 | transfers invalide | CRITIQUE | Spec |
| FAR_006 | transfer_duration invalide | MOYENNE | Spec |
| FAR_008 | agency_id introuvable | CRITIQUE | Spec |
| FAR_009 | Le tarif n’a aucune règle de ligne | FAIBLE | Quality |
| FAR_010 | Règles tarifaires qui se chevauchent | MOYENNE | Quality |
| FAR_011 | payment_method manquant | CRITIQUE | Spec |
| FAR_013 | price ne respecte pas le nombre de décimales ISO 4217 de la devise | FAIBLE | Spec |
| FAR_012 | fare_id manquant | CRITIQUE | Spec |

## FRL — Règles tarifaires

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| FRL_001 | fare_id introuvable | CRITIQUE | Spec |
| FRL_002 | route_id introuvable | CRITIQUE | Spec |
| FRL_003 | origin_id invalide | CRITIQUE | Spec |
| FRL_004 | destination_id invalide | CRITIQUE | Spec |
| FRL_005 | contains_id invalide | CRITIQUE | Spec |
| FRL_006 | Aucune règle tarifaire définie | INFO | Quality |
| FRL_007 | Incohérence logique dans une règle tarifaire | MOYENNE | Quality |
| FRL_008 | Aucun tarif défini pour l’ensemble des lignes | INFO | Quality |

## RCT — Catégories de voyageurs (Tarifs v2)

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| RCT_001 | rider_category_id en double | CRITIQUE | Spec |
| RCT_002 | rider_category_name manquant | CRITIQUE | Spec |
| RCT_003 | is_default_fare_category invalide | CRITIQUE | Spec |
| RCT_004 | min_age ou max_age invalide (champ d’extension GTFS — absent de la spécification officielle) | MOYENNE | Quality |
| RCT_005 | max_age inférieur à min_age | MOYENNE | Quality |
| RCT_006 | fare_product n’a pas exactement une catégorie de voyageur par défaut | MOYENNE | Spec |
| RCT_007 | eligibility_url invalide | FAIBLE | Spec |
| RCT_008 | rider_category_id vide | CRITIQUE | Spec |

## FMD — Supports tarifaires (Tarifs v2)

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| FMD_001 | fare_media_id en double | CRITIQUE | Spec |
| FMD_002 | fare_media_type manquant ou invalide | CRITIQUE | Spec |
| FMD_003 | fare_media_name recommandé pour TransitCard/MobileApp | FAIBLE | Quality |
| FMD_004 | fare_media_id vide | CRITIQUE | Spec |

## FPD — Produits tarifaires (Tarifs v2)

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| FPD_001 | Clé composite fare_products en double | CRITIQUE | Spec |
| FPD_002 | amount manquant ou négatif | CRITIQUE | Spec |
| FPD_003 | currency n’est pas un code ISO 4217 valide | CRITIQUE | Spec |
| FPD_004 | fare_media_id introuvable | CRITIQUE | Spec |
| FPD_005 | rider_category_id introuvable | CRITIQUE | Spec |
| FPD_007 | amount ne respecte pas le nombre de décimales ISO 4217 de la devise | FAIBLE | Spec |

## FLG — Règles de trajet tarifaire (Tarifs v2)

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| FLG_001 | fare_product_id introuvable | CRITIQUE | Spec |
| FLG_002 | network_id introuvable | CRITIQUE | Spec |
| FLG_003 | from_area_id introuvable | CRITIQUE | Spec |
| FLG_004 | to_area_id introuvable | CRITIQUE | Spec |
| FLG_005 | from_timeframe_group_id introuvable | CRITIQUE | Spec |
| FLG_006 | to_timeframe_group_id introuvable | CRITIQUE | Spec |
| FLG_007 | rule_priority invalide | MOYENNE | Spec |
| FLG_008 | fare_product_id manquant | CRITIQUE | Spec |

## FLJ

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| FLJ_001 | from_network_id manquant ou introuvable | CRITIQUE | Spec |
| FLJ_002 | to_network_id manquant ou introuvable | CRITIQUE | Spec |
| FLJ_003 | from_stop_id manquant ou introuvable | CRITIQUE | Spec |
| FLJ_004 | to_stop_id manquant ou introuvable | CRITIQUE | Spec |

## FTR — Règles de correspondance tarifaire (Tarifs v2)

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| FTR_001 | fare_transfer_type manquant ou invalide | CRITIQUE | Spec |
| FTR_002 | from_leg_group_id introuvable | CRITIQUE | Spec |
| FTR_003 | to_leg_group_id introuvable | CRITIQUE | Spec |
| FTR_004 | fare_product_id introuvable | CRITIQUE | Spec |
| FTR_005 | duration_limit_type invalide | CRITIQUE | Spec |
| FTR_006 | duration_limit invalide | MOYENNE | Spec |
| FTR_007 | duration_limit_type défini sans duration_limit | MOYENNE | Spec |
| FTR_008 | transfer_count invalide | MOYENNE | Spec |
| FTR_009 | transfer_count obligatoire lorsque les groupes de trajets sont identiques | MOYENNE | Spec |
| FTR_010 | transfer_count interdit lorsque les groupes de trajets diffèrent | MOYENNE | Spec |
| FTR_011 | duration_limit défini mais duration_limit_type manquant | MOYENNE | Spec |

## ARS — Zones (Tarifs v2)

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| ARS_001 | area_id en double | CRITIQUE | Spec |
| ARS_002 | areas.txt contient un area_id vide | CRITIQUE | Spec |

## SAR — Zones d’arrêt (Tarifs v2)

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| SAR_001 | area_id introuvable | CRITIQUE | Spec |
| SAR_002 | stop_id introuvable | CRITIQUE | Spec |
| SAR_003 | stop_areas contient un area_id vide | CRITIQUE | Spec |
| SAR_004 | stop_areas contient un stop_id vide | CRITIQUE | Spec |

## NET — Réseaux (Tarifs v2)

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| NET_001 | network_id en double | CRITIQUE | Spec |
| NET_002 | route_networks.network_id manquant ou introuvable | CRITIQUE | Spec |
| NET_003 | route_networks.route_id manquant ou introuvable | CRITIQUE | Spec |
| NET_004 | networks.txt contient un network_id vide | CRITIQUE | Spec |

## TFR — Plages horaires (Tarifs v2)

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| TFR_001 | timeframe_group_id manquant | CRITIQUE | Spec |
| TFR_002 | service_id introuvable | CRITIQUE | Spec |
| TFR_003 | Erreur de format sur start_time ou end_time | ÉLEVÉE | Spec |
| TFR_004 | end_time inférieure à start_time | MOYENNE | Quality |
| TFR_005 | Plages horaires qui se chevauchent au sein d’un même groupe et service_id | MOYENNE | Spec |
| TFR_006 | start_time ou end_time supérieure à 24:00:00 | CRITIQUE | Spec |
| TFR_007 | Une seule des valeurs start_time et end_time est renseignée | CRITIQUE | Spec |
| TFR_008 | timeframes contient un service_id vide | CRITIQUE | Spec |

## PTH — Cheminements

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| PTH_001 | pathway_id en double | CRITIQUE | Spec |
| PTH_002 | from_stop_id introuvable | CRITIQUE | Spec |
| PTH_003 | to_stop_id introuvable | CRITIQUE | Spec |
| PTH_004 | pathway_mode manquant ou invalide | CRITIQUE | Spec |
| PTH_005 | is_bidirectional manquant | CRITIQUE | Spec |
| PTH_006 | length invalide | MOYENNE | Spec |
| PTH_007 | traversal_time invalide | MOYENNE | Spec |
| PTH_008 | stair_count manquant | FAIBLE | Quality |
| PTH_009 | max_slope manquant | FAIBLE | Quality |
| PTH_010 | min_width invalide | FAIBLE | Spec |
| PTH_011 | Le cheminement forme une boucle | ÉLEVÉE | Quality |
| PTH_012 | Aucun cheminement accessible vers la station | ÉLEVÉE | Spec |
| PTH_013 | Analyse des cheminements accessibles | INFO | Analytics |
| PTH_014 | Le cheminement franchit la limite de la station | CRITIQUE | Quality |
| PTH_015 | Le cheminement mène à un arrêt inatteignable | MOYENNE | Analytics |
| PTH_016 | Sortie définie comme bidirectionnelle | ÉLEVÉE | Spec |
| PTH_017 | max_slope n’est pas un nombre | MOYENNE | Spec |
| PTH_018 | signposted_as trop long | FAIBLE | Quality |
| PTH_019 | Nœud générique relié à un seul cheminement (impasse) | MOYENNE | Quality |
| PTH_020 | pathway_id manquant | CRITIQUE | Spec |
| PTH_021 | from_stop_id manquant | CRITIQUE | Spec |
| PTH_022 | to_stop_id manquant | CRITIQUE | Spec |
| PTH_023 | pathway_mode manquant | CRITIQUE | Spec |
| PTH_024 | is_bidirectional manquant | CRITIQUE | Spec |
| PTH_025 | Longueur de cheminement recommandée manquante | FAIBLE | Quality |
| PTH_030 | Cheminement affecté à un quai qui possède des zones d’embarquement | FAIBLE | Spec |
| PTH_029 | traversal_time de cheminement recommandé manquant | FAIBLE | Quality |
| PTH_026 | L’extrémité du cheminement est une station | CRITIQUE | Spec |
| PTH_031 | L’extrémité du cheminement est un arrêt accessible depuis la rue (stop_access=1) | CRITIQUE | Spec |
| PTH_027 | stair_count invalide (nul ou non entier) | MOYENNE | Spec |
| PTH_028 | max_slope utilisé en dehors d’un cheminement piéton | FAIBLE | Quality |

## LVL — Niveaux

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| LVL_001 | level_id en double | CRITIQUE | Spec |
| LVL_002 | level_index invalide | CRITIQUE | Spec |
| LVL_003 | level_name manquant | FAIBLE | Quality |
| LVL_004 | Niveau inutilisé | FAIBLE | Quality |
| LVL_005 | level_name trop long | MOYENNE | Quality |
| LVL_006 | level_id manquant sur un arrêt relié par ascenseur | MOYENNE | Quality |
| LVL_007 | level_index manquant | CRITIQUE | Spec |
| LVL_008 | level_id manquant | CRITIQUE | Spec |

## FIN — Informations du jeu de données

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| FIN_001 | feed_publisher_name manquant | CRITIQUE | Spec |
| FIN_002 | feed_publisher_url manquant ou invalide | CRITIQUE | Spec |
| FIN_003 | feed_lang manquant | CRITIQUE | Spec |
| FIN_004 | default_lang invalide | MOYENNE | Spec |
| FIN_005 | feed_start_date invalide | MOYENNE | Spec |
| FIN_006 | feed_end_date invalide ou passée | ÉLEVÉE | Spec |
| FIN_007 | feed_version manquant | FAIBLE | Quality |
| FIN_008 | feed_contact_email invalide | FAIBLE | Spec |
| FIN_009 | feed_contact_url invalide | FAIBLE | Spec |
| FIN_010 | Validité du jeu de données expirée | ÉLEVÉE | Analytics |
| FIN_012 | feed_start_date postérieure à feed_end_date | FAIBLE | Spec |
| FIN_013 | fare_attributes.agency_id recommandé mais manquant | INFO | Quality |
| FIN_014 | Dates de validité du jeu de données (feed_start_date/feed_end_date) manquantes | FAIBLE | Quality |
| FIN_015 | Plusieurs enregistrements feed_info | MOYENNE | Quality |
| FIN_016 | feed_start_date dans le futur (jeu de données pas encore actif) | FAIBLE | Quality |
| FIN_017 | Le jeu de données expire dans un futur très lointain | INFO | Quality |
| FIN_018 | feed_contact_email et feed_contact_url tous deux manquants | FAIBLE | Quality |
| FIN_019 | La validité du jeu de données expire sous 7 jours | FAIBLE | Quality |
| FIN_020 | Fenêtre de validité du jeu de données inférieure à 7 jours | MOYENNE | Quality |

## TRN — Traductions

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| TRN_001 | Valeur table_name invalide | CRITIQUE | Spec |
| TRN_002 | field_name invalide pour cette table | CRITIQUE | Spec |
| TRN_003 | language invalide | MOYENNE | Spec |
| TRN_004 | record_id introuvable | ÉLEVÉE | Spec |
| TRN_005 | Traduction en double | CRITIQUE | Spec |
| TRN_006 | Enregistrement de traduction contradictoire | CRITIQUE | Spec |
| TRN_007 | Traduction dans la même langue que feed_lang | FAIBLE | Quality |
| TRN_008 | La valeur de traduction est vide | CRITIQUE | Spec |
| TRN_009 | record_id et field_value ne peuvent pas être utilisés ensemble | ÉLEVÉE | Spec |
| TRN_010 | record_sub_id invalide | ÉLEVÉE | Spec |
| TRN_011 | field_name n’est pas traduisible | ÉLEVÉE | Spec |
| TRN_013 | Un champ d’identité ne peut pas être utilisé dans une traduction de feed_info | ÉLEVÉE | Spec |
| TRN_014 | record_sub_id valide uniquement pour stop_times | ÉLEVÉE | Spec |
| TRN_017 | Traduction de stop_times sans record_sub_id — l’enregistrement est ambigu | MOYENNE | Spec |
| TRN_015 | record_id et field_value sont tous deux vides | ÉLEVÉE | Spec |
| TRN_016 | field_value ne correspond à aucun enregistrement — la traduction n’est jamais appliquée | MOYENNE | Spec |

## ATR — Attributions

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| ATR_001 | attribution_id manquant | ÉLEVÉE | Quality |
| ATR_002 | organization_name manquant | CRITIQUE | Spec |
| ATR_003 | Rôle d’attribution non défini | ÉLEVÉE | Spec |
| ATR_004 | is_producer invalide | CRITIQUE | Spec |
| ATR_005 | is_operator invalide | CRITIQUE | Spec |
| ATR_006 | is_authority invalide | CRITIQUE | Spec |
| ATR_007 | attribution_url invalide | CRITIQUE | Spec |
| ATR_008 | attribution_email invalide | FAIBLE | Spec |
| ATR_009 | Plusieurs champs de référence d’attribution renseignés | ÉLEVÉE | Spec |
| ATR_010 | agency_id introuvable | FAIBLE | Spec |
| ATR_011 | route_id introuvable | FAIBLE | Spec |
| ATR_012 | trip_id introuvable | FAIBLE | Spec |

## XFL — Inter-fichiers / sémantique

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| XFL_002 | La course n’a aucun enregistrement stop_times | ÉLEVÉE | Interop |
| XFL_006 | service_id ne contient que des exceptions d’annulation (aucun jour actif) | MOYENNE | Analytics |
| XFL_011 | Dates de calendrier hors de la plage feed_info | MOYENNE | Interop |
| XFL_012 | Ligne sans course exploitée | ÉLEVÉE | Quality |
| XFL_013 | shape_id utilisé dans plusieurs sens | ÉLEVÉE | Quality |
| XFL_014 | Référence de traduction invalide (enregistrement source introuvable) | MOYENNE | Quality |
| XFL_015 | Référence invalide dans une attribution | CRITIQUE | Spec |
| XFL_016 | La traduction référence feed_info mais feed_info.txt est absent | ÉLEVÉE | Spec |
| XFL_017 | route_cemv_support en conflit avec agency_cemv_support | FAIBLE | Quality |
| XFL_019 | Réseau défini dans deux fichiers distincts (routes.network_id + route_networks.txt) | MOYENNE | Spec |
| XFL_020 | Couple (from_trip_id/to_trip_id, route_id) invalide dans transfers | CRITIQUE | Spec |
| XFL_021 | Couple (from_trip_id/to_trip_id, stop_id) invalide dans transfers | ÉLEVÉE | Interop |
| XFL_022 | location_group_id introuvable | CRITIQUE | Spec |
| XFL_023 | stop_id introuvable (location_group_stops) | CRITIQUE | Spec |
| XFL_024 | location_group_id introuvable (stop_times) | CRITIQUE | Spec |
| XFL_025 | location_id introuvable (locations.geojson) | CRITIQUE | Spec |
| XFL_031 | Collision d’identifiants : stop_id, id de locations.geojson et location_group_id partagent un même espace de noms | CRITIQUE | Spec |
| XFL_032 | location_groups.txt contient un location_group_id vide | CRITIQUE | Spec |
| XFL_033 | location_group_stops contient un location_group_id vide | CRITIQUE | Spec |
| XFL_034 | location_group_stops contient un stop_id vide | CRITIQUE | Spec |
| XFL_026 | Ligne avec cemv=1 mais aucun produit sans contact applicable | MOYENNE | Quality |
| XFL_027 | Ligne avec cemv=2 mais produit sans contact applicable | MOYENNE | Quality |
| XFL_028 | Agence avec cemv=1 mais aucun support sans contact | INFO | Quality |
| XFL_029 | Ligne avec cemv=1 mais aucun support sans contact | INFO | Quality |
| XFL_030 | Support sans contact présent mais aucun cemv=1 | INFO | Quality |

## OPR — Cohérence d’exploitation

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| OPR_001 | Intervalle de passage trop long sur la ligne | MOYENNE | Analytics |
| OPR_003 | Effet accordéon (intervalle minimal trop faible) | FAIBLE | Analytics |
| OPR_004 | Aucun service le week-end | INFO | Analytics |
| OPR_005 | Fréquence de service inhabituelle | INFO | Analytics |
| OPR_006 | La course compte trop peu d’arrêts (non fonctionnelle) | ÉLEVÉE | Analytics |
| OPR_007 | Séquence d’arrêts répétée au sein d’une course | INFO | Analytics |
| OPR_008 | Vitesse excessive sur plusieurs segments | ÉLEVÉE | Analytics |
| OPR_009 | Heure de départ de la course de nuit trop tardive | INFO | Analytics |
| OPR_010 | Conflits de politique d’accessibilité ou de transport des vélos sur la ligne | MOYENNE | Analytics |
| OPR_011 | Le service n’a aucun jour actif | ÉLEVÉE | Analytics |
| OPR_012 | Interruption de service | MOYENNE | Analytics |
| OPR_013 | La ligne n’est exploitée que dans un seul sens (pas de sens retour) | INFO | Analytics |
| OPR_014 | Temps de correspondance moyen trop long | MOYENNE | Analytics |
| OPR_015 | La ligne n’est exploitée qu’avec un seul tracé | INFO | Analytics |
| OPR_016 | Aucun service actif sur l’ensemble du jeu de données | INFO | Analytics |
| OPR_017 | Distance de course trop courte | MOYENNE | Analytics |
| OPR_019 | Chevauchement de calendriers sur la ligne (plusieurs services le même jour) | INFO | Analytics |
| OPR_020 | Chevauchement de jours d’exception sur la ligne | FAIBLE | Analytics |
| OPR_021 | Conflit de remplacement de calendrier : le remplacement et le service de base sont actifs simultanément | ÉLEVÉE | Analytics |
| OPR_022 | Remplacement de calendrier non appliqué : le service de base circule le jour du remplacement | ÉLEVÉE | Analytics |
| OPR_023 | Vide de remplacement de calendrier : aucun service actif dans la fenêtre | MOYENNE | Analytics |
| OPR_024 | La ligne a un nombre de courses extrême | INFO | Analytics |
| OPR_025 | Durée moyenne des courses du jeu de données inférieure à 60 secondes | ÉLEVÉE | Analytics |

## GEO — Géographique / spatial

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| GEO_002 | Arrêt trop éloigné de la médiane du jeu de données | ÉLEVÉE | Analytics |
| GEO_006 | Saut important dans le tracé | ÉLEVÉE | Analytics |
| GEO_007 | Saut critique dans le tracé (3× le seuil) | ÉLEVÉE | Analytics |
| GEO_009 | Arrêt trop éloigné du tracé de la ligne | ÉLEVÉE | Quality |
| GEO_012 | Grappe d’arrêts (arrêts trop proches les uns des autres) | MOYENNE | Analytics |
| GEO_013 | Synthèse de la couverture géographique du jeu de données | INFO | Analytics |
| GEO_014 | Couverture géographique du jeu de données trop étendue | INFO | Analytics |
| GEO_015 | Coordonnées d’arrêt hors des limites du Japon (feed_lang : ja) | MOYENNE | Quality |
| GEO_016 | Arrêt situé à proximité de Null Island (\|lat\|<0,1 et \|lon\|<0,1) | ÉLEVÉE | Quality |
| GEO_017 | Point de tracé situé à proximité de Null Island | ÉLEVÉE | Quality |
| GEO_018 | Tous les arrêts du jeu de données dans un rayon de 200 m (données de test probables) | ÉLEVÉE | Analytics |
| GEO_019 | L’arrêt a des coordonnées entières (précision nulle) | MOYENNE | Quality |
| GEO_020 | Tracé dégénéré — tous les points au même endroit | ÉLEVÉE | Quality |
| GEO_021 | Plus de 30 % des arrêts partagent des coordonnées (problème systématique) | ÉLEVÉE | Analytics |
| GEO_022 | Latitude d’arrêt proche d’un pôle (\|lat\| > 89) | ÉLEVÉE | Quality |

## DQ — Qualité des données / expérience utilisateur

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| DQ_003 | Description de ligne manquante | INFO | Quality |
| DQ_004 | URL de ligne manquante | INFO | Quality |
| DQ_005 | Aucune période de service valide | ÉLEVÉE | Quality |
| DQ_005b | Aucune course n’a d’horaires d’arrêt | ÉLEVÉE | Quality |
| DQ_005c | Forte proportion d’arrêts sans coordonnées | ÉLEVÉE | Quality |
| DQ_006 | Forte proportion de courses sans tracé | ÉLEVÉE | Quality |
| DQ_009 | Aucun horaire d’arrêt dans les courses | INFO | Quality |
| DQ_010 | Agence utilisée par aucune ligne | INFO | Quality |
| DQ_011 | Un seul arrêt existe | FAIBLE | Quality |
| DQ_012 | Trop d’agences, agency_id non utilisé | FAIBLE | Quality |
| DQ_013 | Trop peu de courses | MOYENNE | Quality |
| DQ_016 | Espaces superflus dans la valeur d’un champ | MOYENNE | Quality |
| DQ_017 | Valeur de coordonnée suspecte | INFO | Quality |
| DQ_018 | Valeur tout en majuscules dans un champ recommandé | MOYENNE | Quality |
| DQ_019 | Valeur tout en minuscules dans un champ recommandé | MOYENNE | Quality |
| DQ_020 | Champ recommandé manquant ou vide | FAIBLE | Quality |
| DQ_021 | Clé primaire en double (duplicate_key) | ÉLEVÉE | Spec |
| DQ_022 | Plus de 80 % des arrêts partagent le même stop_name | ÉLEVÉE | Quality |

## VAT — Détection analytique d’entités

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| VAT_001 | Similarité de tracé entre lignes (doublon probable) | MOYENNE | Analytics |
| VAT_002 | Pôle de correspondance non défini — de nombreuses lignes y passent mais aucune correspondance n’est définie | INFO | Analytics |
| VAT_003 | Durée de course statistiquement aberrante | FAIBLE | Analytics |
| VAT_005 | Grappe d’arrêts isolée — arrêts déconnectés de la composante principale du réseau | MOYENNE | Analytics |
| VAT_006 | Déséquilibre de densité de service — une seule ligne représente une large part des courses du jeu de données | INFO | Analytics |
| VAT_007 | Correspondance au terminus manquante — une autre ligne dessert le terminus mais aucune correspondance n’est définie | INFO | Analytics |
| VAT_008 | Un même tracé est utilisé par plus de 30 % des lignes | INFO | Analytics |

## JPN

| Règle | Titre | Gravité | Classe |
|---|---|---|---|
| JPN_001 | GTFS-JP : lecture kana (ja-Hrkt) manquante pour le nom d’arrêt | MOYENNE | Quality |
| JPN_002 | GTFS-JP : jp_office_id non défini dans office_jp.txt | ÉLEVÉE | Interop |
| JPN_003 | GTFS-JP : agency_id de agency_jp non défini dans agency.txt | ÉLEVÉE | Interop |
| JPN_004 | GTFS-JP : translations.txt manquant | ÉLEVÉE | Interop |
| JPN_005 | GTFS-JP : office_name vide dans office_jp | ÉLEVÉE | Interop |
| JPN_006 | GTFS-JP : fichiers tarifaires manquants | MOYENNE | Quality |
| JPN_007 | GTFS-JP : feed_info.txt manquant | MOYENNE | Quality |
| JPN_008 | GTFS-JP : lecture kana manquante pour route_long_name | MOYENNE | Quality |
| JPN_009 | GTFS-JP : lecture kana manquante pour trip_headsign | MOYENNE | Quality |
| JPN_010 | GTFS-JP : lecture kana manquante pour agency_name | MOYENNE | Quality |
| JPN_011 | GTFS-JP : agency_id obligatoire même avec une seule agence | ÉLEVÉE | Interop |
| JPN_012 | GTFS-JP : agency_id manquant dans agency_jp | ÉLEVÉE | Interop |
| JPN_013 | GTFS-JP : format de agency_zip_number invalide | MOYENNE | Quality |
| JPN_014 | GTFS-JP : office_id manquant ou en double dans office_jp | ÉLEVÉE | Interop |
| JPN_015 | GTFS-JP hérité : route_id invalide dans routes_jp | ÉLEVÉE | Interop |
| JPN_016 | GTFS-JP : route_update_date invalide dans pattern_jp ou routes_jp hérité | MOYENNE | Quality |
| JPN_017 | GTFS-JP : jp_pattern_id manquant ou en double | ÉLEVÉE | Interop |
| JPN_018 | GTFS-JP : jp_pattern_id non défini | ÉLEVÉE | Interop |
| JPN_019 | GTFS-JP : référence de traduction kana invalide | MOYENNE | Interop |
| JPN_020 | GTFS-JP : format de contact du bureau suspect | MOYENNE | Quality |
| JPN_021 | GTFS-JP : traduction kana incohérente | MOYENNE | Quality |
| JPN_022 | GTFS-JP v4 : champ principal obligatoire manquant | MOYENNE | Interop |
