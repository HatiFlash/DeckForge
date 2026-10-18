# DeckForge 0.5 (développement) — état au 17 octobre 2026

## Atelier minimaliste & assistant — A3 (phase 3)

- Livré : barre compacte de simulation en bas de la séquence, journal fermé par défaut ; ouverture au premier événement d'un essai, étapes imbriquées cliquables et chronométrées, messages et bilan de l'exécution. « Simuler » passe par le moteur en mode sec ; « Tester pour de vrai » exige une confirmation explicite, sans exécution à l'ouverture. Les simulations n'émettent aucune action : les blocs sont court-circuités avant les opérations clavier, souris, audio, réseau, OBS, USB, fichiers et navigation ; les conditions peuvent consulter l'état local en lecture seule. Les branches des conditions et un seul tour de répétition sont explorés ; ce n'est pas une prévision fidèle du comportement réel. L'ancien aperçu statique reste limité aux tests de compatibilité.
- Testé automatiquement : agrégation des événements, état initial fermé, dépliage avec événements injectés et absence d'exécution réelle avant confirmation ; `cargo check` et build Windows complet (résultat au commit). Aucune macro réelle lancée pendant les tests.
- À tester avec Hatim : lisibilité du panneau et clic des lignes imbriquées, annulation/confirmation de l'exécution réelle sur une macro volontairement bénigne, simultanéité avec les déclencheurs physiques. Les messages textuels du journal partagé peuvent inclure d'autres événements pendant un essai ; les durées par étape sont disponibles pour les étapes réussies, pas pour celle interrompue. Le chat flottant est un objectif ultérieur.

## Atelier minimaliste & assistant — A2 (phase 2)

- Livré : menu de carte (dupliquer, supprimer, monter, descendre, désactiver/réactiver, insérer après), insertion à la ligne entre cartes via la bibliothèque, réorganisation par glisser-déposer au même niveau (pas entre branches), repère bleu, raccourcis de sélection et d’édition hors champs texte, erreurs de validation sur cartes et propriétés. Opérations pures sur chemins imbriqués ; modifications annulables.
- Testé automatiquement : opérations imbriquées, bornes et chemins invalides, scénario egui sélection → Ctrl+D → Suppr → annuler ; `cargo check`, build Windows complet. Aucun déclenchement de macro réelle.
- À tester avec Hatim : visibilité du « + » au survol, confort du glisser-déposer et du panneau d’erreurs, raccourcis sur clavier réel et branches profondes. Glisser-déposer interbranches non livré ; simulation/journal et mini chat restent pour les phases suivantes.

## Atelier minimaliste & assistant — A1 (phase 1)

- Livré : deux zones permanentes, bibliothèque redimensionnable avec recherche et catégories repliables, séquence simple en cartes imbriquées et état vide ; inspecteur flottant uniquement sur sélection, réinitialisé au changement de touche/page/profil. Mode Avancé et éditeur détaillé préservés ; bouton Assistant ouvre le connecteur MCP existant, sans nouveau chat ni appel IA.
- Testé automatiquement : affichage egui 1366×768 et 1920×1080, blocs imbriqués, inspecteur contextuel, touche vide, recherche sans résultat et changement de touche ; compilation et build Windows complet (voir commit). Aucune macro réelle exécutée.
- À tester avec Hatim : lisibilité et manipulation à l'écran, édition de paramètres dans l'inspecteur, redimensionnement de la bibliothèque et ergonomie tactile/souris. Glisser-déposer et édition avancée des sous-blocs restent dans les phases suivantes ; le journal de simulation et le mini chat flottant ne sont pas livrés par A1.

## M5 — Grille dynamique (code livré)

Mini 2×3, D6 5×3 et XL 8×4 : pavé et aperçu des touches visibles selon le modèle HID détecté. Les 15 touches historiques restent inchangées et conservées quand un modèle plus petit est connecté ; XL étend les pages jusqu’à 32 touches. MCP : indices limités à la grille active (15 hors connexion). Grilles alternatives expérimentales, non vérifiées sur matériel ; à tester avec Hatim et ses collègues. Tests purs, rendu egui hors écran et build Windows complet réalisés.

Point de reprise après la session Claude du 17/10/2026. Version de travail : branche `claude/0.5`, commit de fusion `45d73e1` (+ ce document). Construite et vérifiée par `tools/Build-Windows.ps1 -Action All` : 162 tests application + 6 tests MCP, contrôles MCP stdio/HTTP, aucun avertissement de compilation, Clippy ≤ 30. **Pas encore validée sur le D6 dans sa forme fusionnée.**

Exécutables prêts sur le PC fixe : `Bureau\DeckForge-0.5-fusion\DeckForge.exe` (+ `deckforge-mcp.exe`, config, assets, `plugins\`). Version d'essai plugins seule : `Bureau\DeckForge-plugins-test`.

## M2 — Rapport périphériques (18/10/2026)

- Livré : journal HID local par démarrage et reconnexion dans `diagnostics/peripheriques-AAAAMMJJ-HHMMSS.log` (10 derniers), inventaire complet et décision de transport, traces D6 des commandes/entrées/images/erreurs, sérial masqué et chemins du profil Windows remplacés. Fenêtre Réglages → Rapport périphérique… : affichage, copie, dossier et export .txt. Les autres modèles restent expérimentaux et non pilotés.
- Testé automatiquement : anonymisation, format hexadécimal, rotation ; build complet Windows.
- À tester avec Hatim : lecture du rapport et reconnexion réelle du D6, code Windows des erreurs HID sur machines tierces, ouverture du dossier et export ; aucun périphérique tiers testé sur matériel.

## E1 — Déclencheurs automatiques (17/10/2026)

- Livré : ouverture/fermeture d’application, heure fixe avec jours, minuteur, branchement/débranchement de périphérique audio ; ajout et suppression dans « Windows et raccourcis », destination profil/page/touche conservée. Surveillance hors verrou, une seconde, sans action au démarrage ; pause globale respectée.
- Testé automatiquement : transitions pures et compatibilité des anciennes préférences ; build complet Windows.
- À tester avec Hatim : vrais lancements/fermetures d’application, connexion/déconnexion d’un périphérique audio, heure locale et passage de minute, pause globale et fonctionnement sans D6. Aucun test automatisé ne déclenche de macro réelle.

## M4 — Elgato (expérimental)

- Livré : pilote HID Elgato Original, Mini, générations v2 et modules ; images BMP/JPEG, découpage des rapports, luminosité, reset, firmware, touches. Stream Deck Plus : touches uniquement ; événements de molettes et écran ignorés et journalisés. Tous les modèles Elgato restent non vérifiés sur matériel.
- Testé automatiquement : vecteurs purs de rapports, fonctionnalités, mapping et décodage, sans ouvrir de périphérique HID.
- À tester avec Hatim et ses collègues : tailles réelles des rapports HID, écran/touches/firmware de chaque variante, coexistence avec application Elgato, touches au-delà de 15 (limite actuelle de l’interface), déconnexion. Aucun essai matériel effectué.

## E2 — Contrôle d’exécution par touche (17/10/2026)

- Livré : `run_mode` facultatif (`ignore`, `restart`, `queue`) et `debounce_ms` (défaut 0) sur chaque touche ; inspecteur Avancé, file globale bornée à cinq macros, arrêt et redémarrage annulables.
- Testé automatiquement : décision pure en cours/nouvel appui/anti-rebond, compatibilité JSON et build Windows complet.
- À tester avec Hatim : appuis physiques répétés et enchaînement de macros longues, touche Arrêter, raccourcis globaux et déclencheurs automatiques. Aucun test automatisé ne lance de macro réelle.

## Blocs de l'Atelier — lot A

- Livré et testé (automatique) : `device_volume`, contrôle du périphérique de sortie ou d'entrée Core Audio, liste des périphériques actifs, simulation sans effet, cinq modèles audio et schéma MCP.
- À tester avec Hatim : régler le volume d'un microphone et d'une sortie réelle, muet / rétablissement, déconnexion d'un périphérique sélectionné. Aucun test automatisé ne modifie le son réel.
- `notify` livré et testé automatiquement : notification via l’icône Windows existante, journal/bandeau en secours ; substitution pure des variables. Migration FocusAudio non livrée.
- À tester avec Hatim : apparition de la notification Windows et secours lorsque l’icône de zone de notification manque. Les conditions et l'attente font partie du lot A2 ci-dessous.

## Blocs de l'Atelier — lot A2

- Livré et testé automatiquement : « Si… sinon » (`if_condition`), conditions Windows en lecture seule et logique booléenne/variables ; les deux branches sont simulées sans effet.
- À tester avec Hatim : application ouverte par rapport au premier plan, titre de fenêtre, sessions audio actives et muettes, périphériques branchés/débranchés, branches de macro sur le D6. Aucun test automatisé n'agit sur le son ou le clavier.
- Livré et testé automatiquement : « Attendre que… » (`wait_until`), sondage annulable, délai borné et simulation en lecture seule.
- À tester avec Hatim : attente d’une condition réelle, expiration et bouton Arrêter pendant une attente.

## Lot C3 — opérations sur les fenêtres (17/10/2026)

- Livré : « Opération sur une fenêtre » (`window_op`) : activation, réduction, maximisation, restauration, déplacement, redimensionnement, changement d’écran et fenêtre toujours au premier plan. Cibles premier plan, fragment de titre ou exécutable ; protection de DeckForge sauf recherche explicite par titre. Atelier, trois modèles et schéma MCP inclus ; simulation sans effet.
- Testé automatiquement : défauts et aller-retour JSON, validation, résumé et calcul pur multi-écrans ; build Windows complet. Aucun test ne touche aux fenêtres réelles.
- À tester avec Hatim : ciblage d’une application non élevée, activation refusée par Windows, écran secondaire débranché, maximisation/restauration et bascule toujours au premier plan.

## Lot C2 — souris avancée (17/10/2026)

- Livré : « Clic souris » (`mouse_click`) avec coordonnées facultatives et « Déplacer la souris » (`mouse_move`) avec origine `screen`, `window` (zone client capturée au déclenchement) ou `cursor` ; « Défilement souris » (`mouse_scroll`) horizontal ou vertical. Coordonnées bornées aux écrans virtuels ; anciennes macros `relative` acceptées. Atelier et MCP inclus.
- Testé automatiquement : sérialisation, validation et calculs purs des coordonnées ; build Windows complet. Aucun test ne clique, déplace la souris ou ne défile réellement.
- À tester avec Hatim : disposition multi-écrans, zone client de la fenêtre ciblée, clic positionné et défilement horizontal dans une application choisie.

## Lot C1 — maintien de touche (17/10/2026)

- Livré : « Maintenir une touche » (`key_hold`), touches au format Raccourci, maintien borné ou jusqu’au bloc Relâcher. Registre de touches dans le moteur ; libération à la fin, lors d’Arrêter / Ctrl+Alt+F12 et à la fermeture. Atelier, modèle et schéma MCP inclus ; simulation sans effet.
- Testé automatiquement : sérialisation, validation, résumés, registre pur et build Windows complet (voir commit). Aucun test ne produit de saisie clavier réelle.
- À tester avec Hatim : maintien / relâchement sur une application volontairement ouverte, arrêt pendant maintien, fin de macro et fermeture de DeckForge ; ne pas lancer les tests sur une application sensible.

## Lot C — application déjà ouverte

- Livré (tests automatiques) : « Lancer une application » (`launch`) accepte `focus_if_running` (défaut false, anciennes configurations compatibles). Recherche une fenêtre visible du même exécutable et tente de l’activer sans créer de processus.
- À tester avec Hatim : activation d’une application ouverte, refus Windows du premier plan, application élevée/inaccessible. Aucun test automatisé n’ouvre de fenêtre réelle.
- Non livré dans ce sous-lot : maintien des touches, coordonnées et défilement avancés de la souris, opérations sur les fenêtres.

## Lot B4 — interpolation des gabarits

- Livré et testé automatiquement : `{nom}` et les variables spéciales `{date}`, `{heure}`, `{horodatage}`, `{presse_papiers}` dans la saisie `text`, l'adresse `url`, `http_request` (adresse et corps), `udp_send` (texte), `launch` (arguments). Configuration conservée sous forme de gabarit ; lecture presse-papiers Win32 Unicode avec réessais, sans écriture ; simulation sans effets. Les types JSON existants ne changent pas.
- À tester avec Hatim : date/heure locale sur PC, presse-papiers texte occupé ou non textuel, URL réseau et arguments sur une macro réelle. Aucun test automatisé ne manipule les fenêtres, clavier ou presse-papiers du PC.

## Lot B5 — presse-papiers

- Livré et testé automatiquement : « Presse-papiers » (`clipboard`), copie Unicode, lecture dans une variable, vidage, sauvegarde/restauration du texte pendant une macro, simulation sans lecture ni écriture ; modèle « Coller sans mise en forme ». Tests purs seulement, sans accès au presse-papiers ni au clavier réel.
- À tester avec Hatim : copier/coller Unicode dans une application, presse-papiers occupé ou non textuel, restauration après collage. Le modèle laisse le texte copié dans le presse-papiers après collage ; il ne restaure pas les formats non textuels.

## Nouveautés 0.5

| Domaine | Contenu | Vérifié |
|---|---|---|
| Plugins Stream Dock / FIFINE (`plugins.rs`, `plugin_import.rs`, `plugin_host.rs`, `plugin_runtime.rs`, `plugin_http.rs`, `ui_plugin_store.rs`) | Magasin en cartes (Disponibles / Installés, Importer tout) ; hôte WebSocket compatible SDK Elgato/Stream Dock ; démarrage à la demande des plugins visibles ; willAppear/willDisappear ; keyDown/keyUp ; setTitle/setImage/setState/showOk/showAlert → face runtime envoyée au D6 (`Command::KeyImage`) ; icône par défaut du manifeste ; réglages (settings, global settings → `plugin-settings.json`) ; redémarrage borné (3 en 5 min) puis état erreur ; plugins HTML dans Edge headless ; Property Inspector dans une fenêtre Edge `--app`, habillé aux couleurs de DeckForge | Tests + **essai réel** : VolumeController affiche icône et « 50 % » sur le D6 ; Windows Gizmos démarre ; PI s'ouvre et se connecte |
| OBS (`obs.rs`, `obs_monitor.rs`) | WebSocket v5 + authentification ; scène, enregistrement, stream, caméra virtuelle, replay, mute, visibilité de source ; connexion persistante avec reconnexion ; état REC/LIVE/CAM sur la touche ; listes de scènes/sources dans l'atelier | Faux serveur local uniquement |
| Actions natives | Soundboard WAV/MP3 (MCI, 8 sons max, stop) ; souris (clic, déplacement, molette) ; fermer la fenêtre active (protège DeckForge) ; luminosité/veille du D6 ; UDP ; HTTP sans TLS (vMix, Home Assistant local) ; touche à deux états `Toggle` avec badge 1/2 ; assistant raccourcis Discord ; schémas MCP de ces actions | Tests uniquement |

Correctifs trouvés pendant l'essai réel : UUID de plugin absent du manifeste → déduit du dossier ; `CodePathWin` prioritaire sur `CodePath` ; chemins HTTP `%20` (dossier « property inspector ») ; le PI doit être connecté via `connectElgatoStreamDeckSocket` (SDK MiraBox/Stream Dock) et recevoir `didReceiveSettings` à l'ouverture.

## Reprise finale en cours — signalement Hatim (17/10/2026)

- Nouveau défaut distinct du GIF : toute la fenêtre clignote quand la souris va de la prévisualisation D6 (haut) aux réglages (bas), puis se stabilise souris immobile. Reproduire/mesurer sur fenêtre isolée sans remplacer l’application ouverte ; cause et correctif non encore établis. Le correctif GIF antérieur ne valide pas ce problème.
- Décision UX : actions audio Core Audio configurables nativement dans l’inspecteur (vrais périphériques, entrée/sortie, rafraîchir, volume/muet/bascule suivant capacités). Magasin : signaler cette alternative au plugin VolumeController sans supprimer la compatibilité tiers ni convertir les données utilisateur sans accord. Pas de WebView2 pour le besoin natif. PI VolumeController vide = problème de compatibilité distinct, non résolu.
- Essai passif de la fenêtre déjà ouverte DeckForge-plugins-test : PrintWindow 134 captures / 2 s, 0 fond vide, +0,5 s CPU. Souris non déplacée : le clignotement décrit par Hatim n’est ni reproduit ni corrigé. La fenêtre utilisée occupe potentiellement le D6 : aucune autre instance lancée, pas de macro ni de manipulation audio.
- Magasin (en cours de validation) : propose les actions natives multimédias volume + / - / muet et le basculement Core Audio de deux sorties ou de deux entrées réellement détectées ; configuration editable dans l’inspecteur existant. Pas de contrôle de volume par périphérique ajouté, pas de conversion automatique des touches de plugin. La liste audio est actualisée par le surveillant Core Audio toutes les secondes, non sur demande.
- Première exécution Build-Windows.ps1 -Action All : 161 tests réussis, 2 échecs (mise en page du magasin après ajout du panneau natif ; node_fixture_exchanges_key_messages : timeout). Panneau déplacé après les deux titres pour préserver leur visibilité. Deuxième exécution : 163 tests app + 6 MCP réussis ; stdio/HTTP réussis ; aperçus PNG produits, non inspectés visuellement. Le test Node reste potentiellement instable, cause non démontrée.
- Livraison 0.5 de test uniquement après vérification ; pas de release stable ni revendication matériel non démontrée. Journaliser ici chaque essai, échec, correctif et commit.

## Problèmes ouverts (priorité)

1. **PI VolumeController : liste « Périphérique » vide.** La page se connecte et se traduit, mais le plugin ne lui renvoie pas la liste des périphériques. Pistes : (a) journaliser tous les messages PI ↔ plugin (`sendToPlugin`, `sendToPropertyInspector`) dans le Journal pour voir qui ne répond pas ; (b) vérifier le `context` du `sendToPropertyInspector` émis par le plugin (le routage actuel exige `context == contexte de la touche`) ; (c) le plugin peut attendre `propertyInspectorDidAppear` avec un payload/`device` précis, ou l'info `devices` d'inscription du plugin. Le titre de fenêtre « Input Device Control » sur une touche liée à `outputDeviceActionButton` est à vérifier aussi (mauvais `PropertyInspectorPath` choisi ?).
2. **Réglages de plugin dans une fenêtre séparée.** Choix technique : la page de réglages d'un plugin est du HTML/JavaScript qui dialogue en WebSocket ; l'interface DeckForge (egui) ne sait pas afficher du HTML. Options pour l'intégrer : (a) WebView2 incrusté dans la zone inspecteur (crate `wry`/`webview2-com`, fenêtre enfant positionnée sur le panneau) — rendu identique à Stream Dock, effort moyen ; (b) formulaires natifs egui pour les plugins les plus utilisés (VolumeController, Discord, OBS) — plus beau, plugin par plugin ; (c) a minima, fenêtre Edge accrochée/positionnée sur l'inspecteur et fermée avec la sélection.
3. Windows Gizmos lancé en deux processus : vérifier qu'un plugin n'est démarré qu'une fois.
4. Test `plugin_host::node_fixture_exchanges_key_messages` encore instable sous charge (passe en relance).
5. Essai réel de la 0.5 fusionnée : plugins, OBS réel, Soundboard, Toggle, luminosité/veille, Discord MiraBox (autorisation Discord à la première connexion).

## Branches

| Branche | Rôle |
|---|---|
| `main` | 0.4.3 publiée (Codex) puis avance rapide vers la 0.5 de développement |
| `claude/0.5` | Intégration 0.5 (fusion de `claude/native` et `claude/plugins`) |
| `claude/native` | Actions natives + OBS |
| `claude/plugins` | Système de plugins |

## Livraison partielle du 17/10/2026
Audio natif propose dans le magasin, tests rapportes du worker : 163 + 6 MCP, build complet reussi. Clippy execute par le coordinateur avec succes (voir log df-final-clippy.log). Dossier Bureau\DeckForge-0.5-test-final prepare, empreintes des deux binaires verifiees, configuration copiee et sauvegardee. Application non lancee. Clignotement au mouvement de souris NON corrige ; PI vide et test Node intermittent toujours ouverts. Pas une version finale stable.

## Lancement 0.5 et CI Windows - 17/10/2026

- L'ancien PID 42916 (DeckForge-plugins-test) avait disparu avant la bascule : aucun processus force ni autre terminal ferme. Copie source/destination identique pour config.json et preferences.json ; 6 assets dans chaque dossier, aucun asset manquant. La sauvegarde `DeckForge-plugins-test/config.avant-bascule-20261017-172216.json` existe.
- `DeckForge-0.5-test-final/DeckForge.exe` lance dans la session Windows : PID 43900, chemin confirme, vraie fenetre `DeckForge - FIFINE D6` (caracteres Unicode affiches selon le terminal), Responding=True. Capture PrintWindow reussie : `DeckForge-0.5-test-final/capture-demarrage.png`, 1395x937, 485858 octets. Ne prouve ni interactions D6 ni absence du clignotement en mouvement.
- Runs 39043742520, 39043740553 et 39044487223 : le test `plugin_host::tests::node_fixture_exchanges_key_messages` attend `registerPlugin` pendant 15 secondes puis `Timeout` a `src/plugin_host.rs:121`. L'avertissement checkout Node n'est pas l'erreur. Le workflow ne fixait pas Node et la fixture JS exige le WebSocket global ; version/capacite du Node des runners en echec non journalisee, donc cause environnementale precise non prouvee. Correctif cible : Node 24 fixe avec controle WebSocket avant build ; le test echoue explicitement si Node/WebSocket manque, au lieu de sauter silencieusement. Le dialogue plugin reste teste.
- Build-Windows.ps1 -Action All avec toolchain explicite : 163 tests app et 6 tests MCP, transports stdio/HTTP OK. Journal local hors Git `build-ci-fix.log`. Verifier la CI du nouveau commit avant toute declaration verte.
## Diagnostic du clignotement — instrumentation en cours

Objectif : depuis Réglages, démarrer une capture locale bornée, survoler entre aperçu et inspecteur, arrêter et transmettre le chemin JSONL. Aucun correctif de rendu/GIF revendiqué. Tests/build et livraison à inscrire après validation.

Checkpoint instrumentation : capture locale opt-in câblée dans Réglages, cache aperçu et App::update ; premier build compile et 163 tests passent, mais test Node plugin_host intermittent échoue (timeout), donc build complet non validé à ce stade. Procédure : docs/DIAGNOSTIC-CLIGNOTEMENT.md. Aucun essai de survol réel et aucune correction du clignotement.

## Correctifs du 17/10/2026 (soir, Claude) - mesures réelles

- Réglages : fenêtre « À propos de DeckForge » (version Cargo, hash et date de compilation, copie, page GitHub) ; vérification manuelle des mises à jour du dépôt privé via GitHub CLI, hors thread UI et sans réseau dans les tests.

- **Clignotement au survol : cause trouvée et corrigée.** Mesure PrintWindow en rafale avec la souris déplacée automatiquement entre l'aperçu et l'inspecteur (script `%TEMP%\df-flash2.ps1`, image « vide » = 48 points de grille d'au plus 2 couleurs) : rendu OpenGL (glow) = 2 images vides sur 428 et 1 coeur CPU à 100 % pendant le mouvement, identique avec l'exécutable de `DeckForge-0.5.0-test` (pas une régression des lots plugins/natif). Image vide = couleur de fond seule : le pilote présente une image effacée sans le dessin egui. Correctif : `eframe` avec la fonctionnalité `wgpu` et `renderer: eframe::Renderer::Wgpu` (DirectX 12/Vulkan, présentation flip). Après : 0 image vide sur 1 221 captures (dont 908 en mouvement), CPU ~40 % pendant le mouvement, rendu vérifié à l'écran. Le journal de diagnostic (Réglages du D6) reste disponible.
- **Test Node intermittent / CI GitHub en échec : cause trouvée.** Sous Windows, un socket accepté depuis un écouteur non bloquant hérite du mode non bloquant : la poignée de main WebSocket ou la requête HTTP arrivée quelques millisecondes après `accept()` était abandonnée (`WouldBlock`). Corrigé (`set_nonblocking(false)` après `accept`) dans `plugin_host.rs` (plugins), `plugin_http.rs` (pages HTML / réglages de plugin) et `mcp_server.rs` (pont MCP). Tests de régression avec client lent ajoutés ; 5 exécutions consécutives sans échec (1,5 s au lieu de 15 s de délai). Ce défaut pouvait aussi faire échouer aléatoirement la connexion d'un vrai plugin, le chargement d'un script de page de réglages ou une requête MCP.
## Bloc natif « Volume d'une application » (17/10/2026, Claude)

- Fait : bloc `app_volume` (Atelier > Médias et audio, 4 modèles dans le magasin : App +5 %, App -5 %, App muette, App à 50 %, MCP `configure_key`). Cible au premier plan (capturée à l'appui, ou après le compte à rebours de « Tester dans 3 s ») avec processus enfants, ou exécutable précis. Sessions Core Audio de toutes les sorties actives ; jamais le volume général. Variation en points, bornes 0-100, muet conservé, bascule muet cohérente sur plusieurs sessions. DeckForge au premier plan ou aucune session : message, rien n'est modifié. Simulation : résumé seulement.
- Testé : 173 tests + 6 MCP (calcul en points et bornes, bascule multi-sessions, valeurs par défaut, validation, résumé, refus des champs inconnus). Aucun son réel modifié pendant les tests.
- À tester avec Hatim : deux applications qui jouent du son (seule celle au premier plan change), navigateur, changement de sortie audio, jeu, appuis rapides, application fermée.
- Étape suivante : migration des 4 binds « Audio · App active » qui appellent `FocusAudio.exe` (reconnaître chemin + arguments, conserver titre/icône, sauvegarder avant), seulement après l'essai réel.
- Pavé de l'aperçu : il suit la taille de la fenêtre, sans barre de défilement (commit e1c284c).
## Lot A terminé (17/10/2026)
- `device_volume` (e22a7ff), `if_condition` + 14 conditions (79eb430), `wait_until` (4dc1cd8), `notify` avec {variable} (f71fe8a), conversion FocusAudio.exe -> `app_volume` (Réglages > À propos, sauvegardes `*.avant-migration-<horodatage>.json`). Build : 180 tests + 6 MCP.
- Le worker Sol est en limite d'usage (~110 h) : lots suivants (B variables/presse-papiers, C fenêtres/clavier/souris, D affichage d'état, E déclencheurs) à reprendre par Opus ou un autre worker.
## Lot D — affichage suivant l'état réel (17/10/2026)

- Livré dans la branche : champ optionnel `state_display` sur une touche (condition réutilisée, faces « vrai »/« sinon », titre, couleur et icône), surveillance groupée hors verrou et rendu ciblé par `KeyImage`, sans nouvelle action Windows ; ancienne configuration inchangée.
- Tests automatiques : sérialisation et refus de paramètres invalides, changement d'état sans notification si état inchangé, schéma MCP et bibliothèque de trois modèles. Le build complet est à consigner après exécution ; aucun changement de son réel dans les tests.
- À tester avec Hatim : basculer le muet du micro et d'une application depuis Windows, brancher/débrancher un casque, choisir deux icônes dans la galerie, veille/réveil et changement de page sur le D6. Ne pas considérer ces scénarios matériels comme validés automatiquement.

## Lot B — variables et texte

Implémenté mais non livré : Calculer (`math`). Le build complet échoue sur `ui::tests::inspector_dpi_keeps_keys_and_primary_actions_visible_without_button_overlap` (182 tests réussis, 1 échec ; fichier `src/ui_tests.rs` déjà modifié avant ce lot). Aucun commit tant que le build complet ne passe pas.
À tester avec Hatim : ergonomie de l’inspecteur et calculs dans une macro réelle ; aucun test automatique ne touche au clavier ni au presse-papiers.
Non livré dans le premier passage : opérations texte, persistance des variables, interpolation élargie et presse-papiers.

## Lot B2 — bloc Texte (`text_op`)

- Livré : bloc Texte dans l’Atelier et MCP ; opérations définir, ajouter, préfixer, majuscules, minuscules, rogner, remplacer et compter les caractères. Modèle « Mettre en majuscules » dans le magasin après les médias ; variables locales à la macro, interpolation des valeurs et recherches, simulation sans effet.
- Testé automatiquement : sérialisation, valeurs par défaut, refus des champs inconnus et des opérations invalides, résumés et calculs purs Unicode ; build complet Windows vert : 191 tests Rust et 6 MCP, zéro warning.
- À tester avec Hatim : enchaîner des blocs Texte avec les variables d’une macro et contrôler l’ergonomie de l’Atelier. Aucun test automatique ne saisit au clavier ni n’utilise le presse-papiers.

## B3 — variables persistantes

Livré : option « Conserver entre les macros » pour Définir une variable (`set_variable`), Calculer (`math`) et Texte (`text_op`) ; fichier atomique `variables.json`, limite 256 noms et 4 Ko par valeur ; fenêtre Réglages → Variables enregistrées (édition/suppression). Les valeurs sont chargées au démarrage et accessibles par `{nom}`. Tests automatiques : compatibilité JSON, limites et persistance sur disque ; simulation sans écriture. À tester avec Hatim : enchaînement de deux macros, redémarrage puis modification/suppression depuis les Réglages.

## Multi-périphériques M3 — pilote Mirabox expérimental

- Livré : sélection automatique du modèle via le catalogue M1, priorité au D6 vérifié, journal des autres candidats, commandes Mirabox V1/V2 paramétrées (512/1024 octets), tailles et orientation des images, ACK borné pour les modèles expérimentaux, état du modèle affiché dans la barre périphérique. Le chemin D6 conserve ses octets et son calendrier d'envoi ; aucun périphérique tiers n'a été vérifié sur matériel.
- Testé automatiquement : comparaison pure des en-têtes BAT D6 avec l'ancien calcul, tailles, indices, rendu et décodage ; build complet Windows vert (219 tests application, 8 MCP, aucun avertissement).
- À tester avec Hatim : fonctionnement réel du D6 et des modèles Mirabox sur les appareils de collègues ; ordre physique des touches, firmware, ACK et orientation des images. Elgato et Mirabox V3 reconnus mais non pilotés ; touches supplémentaires et molettes Mirabox sont journalisées, sans assignation de macros.

## Multi-peripheriques M1 - catalogue et detection pure

- Livre : table FIFINE D6, AKP153/HSV293S/293V3, AKP03/N3 et Elgato Original/Mini/MK.2/XL/Neo/Plus/Modules ; detection VID:PID et interface HID sans ouvrir de peripherique. D6 seul valide localement ; tous les autres modeles sont experimentaux. Collision documentaire 0300:1001 affectee provisoirement a AKP03, sans supposer le layout sur materiel.
- Teste automatiquement : unicite et geometrie de la table, filtre D6 identique au code actuel, candidats PID inconnu et interface non retenue ; build complet Windows.
- A tester avec Hatim et ses collegues : enumeration reelle et identification de chaque modele, rapports HID, affichage et controles ; aucune integration au transport, aucune image envoyee par M1.

## Assistant de test des périphériques — M6

Livré : numérotation des touches, capture et comparaison des appuis bruts/décodés, commandes de luminosité 30 % / 100 %, orientation demandée, rapport texte copiable et enregistrable, macros suspendues. Tests unitaires purs du mapping et de la détection d’inversion ; build Windows complet. À tester avec Hatim et des collègues : affichage numéroté, orientation, réception des touches et effet visuel de la luminosité sur chaque modèle (les autres que D6 restent expérimentaux).
