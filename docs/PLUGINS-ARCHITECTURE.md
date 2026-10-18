# Plugins — décision et audit ciblé (étape A)

DeckForge est MIT. `THIRD-PARTY-NOTICES.txt` existe ; aucun code OpenDeck (GPL) n'est repris. Choix : hôte DeckForge indépendant, processus distincts et protocole local documenté plutôt qu'une passerelle OpenDeck (dépendance externe et licences à gérer). Importer ≠ héberger ≠ tester le bon fonctionnement. Aucun plugin tiers n'est publié dans le catalogue statique version 1, vide et disponible hors réseau.

Code existant réutilisé : `plugins.rs:parse/discover` (manifestes, actions, runtime, ELGATO\x01), `plugin_import.rs:import_folder/import_archive/sources` (copie locale, extraction), `plugin_host.rs:Host::start/socket_loop` (processus et WebSocket loopback), `plugin_runtime.rs:run/event` (cycle des touches), `plugin_http.rs:safe_file/response` (ressources et inspecteur HTML), `ui_plugin_store.rs:App::plugin_store` (magasin et actions). Les profils ne sont pas touchés lors de l'import.

Matrice vérifiée dans `plugin_runtime.rs:run`, `ui.rs` et `plugin_host.rs` : « partiel » signifie chemin présent mais contrat complet / persistance / état non garantis.

| Événement | Aujourd'hui | Constat |
| --- | --- | --- |
| willAppear | supporté | émis à l'ajout et au redémarrage |
| willDisappear | supporté | émis à la disparition de la touche |
| keyDown / keyUp | supporté | `Request::Key` |
| setImage / setTitle | partiel | messages reçus, rendu lié à la touche ; pas de validation complète SDK |
| setState | partiel | dépend du traitement UI et des états déclarés |
| showOk / showAlert | partiel | retour UI non testé sur toutes les surfaces |
| getSettings / setSettings / didReceiveSettings | partiel | traitement par instance et inspecteur ; reprise complète à tester |
| getGlobalSettings / setGlobalSettings | partiel | chemin de message existant, persistance à confirmer |
| deviceDidConnect / deviceDidDisconnect (D2) | partiel | lancement/redémarrage et changement de grille ; pas de preuve matérielle ni identifiant série |
| titleParametersDidChange / systemDidWakeUp / applicationDidLaunch / applicationDidTerminate (D2) | absent | émission non implémentée, voir limites D2 |

Les plugins exécutables tournent avec les droits de l'utilisateur : processus séparé ≠ sandbox. Le catalogue n'accorde aucun droit ; la compatibilité est « non vérifiée » jusqu'à un essai explicite. Ni archive chiffrée ni API non reconnue ne sont déchiffrées ou adaptées automatiquement. L'installation est une copie atomique sans liaison de touche ; affecter une action ultérieurement peut démarrer le plugin. SHA256 en métadonnée locale pour les archives seulement, non preuve d'éditeur.

Étape B (périmètre vérifié) : témoin MIT `tests/fixtures/counter.sdPlugin` (Node natif WebSocket, sans code GPL), action `org.deckforge.counter.increment`. Contrat : `willAppear` transmet les réglages et déclenche `getSettings` ; `didReceiveSettings` charge les paramètres `step` (>0 entier, sinon 1) et `count` ; chaque `keyDown` renvoie `setTitle` (nouveau total) et `setSettings` ; `keyUp` n'incrémente pas. `error` = `crash`, `timeout`, `malformed` ou `foreign` uniquement pour les essais contrôlés. La validation C1 instancie `App::init` isolée avec un chemin temporaire, un `Manager` limité au témoin et un émetteur virtuel `Event::Key/KeyRelease` : la porte `InputGate`, la configuration appliquée et `App::plugin_message` sont exercées sans D6, USB ni application réelle. Elle compare le code Node au témoin embarqué avant toute exécution ; un plugin tiers (ou témoin modifié) n'est jamais exécuté par cette validation : résultat « Non vérifié » pour chaque action. Le rapport `validation-report.json` du dossier installé contient action, verdict, raison, mode simulé et date Unix ; le bouton « Tester le plugin (simulation uniquement) » travaille sur thread de fond. Aucun essai réel par défaut, aucun badge fonctionnel déduit de l'import. Reproduction : `cargo test --locked plugin_validation::tests` (Node avec WebSocket global requis).

| Événement | État après B | Preuve/limite |
| --- | --- | --- |
| willAppear / willDisappear | prouvé pour le témoin, partiel pour tiers | `willAppear` observé par Node ; `willDisappear` émis par `Manager::sync` lors de la navigation App et accusé par le témoin via `logMessage` |
| keyDown / keyUp | prouvé pour le témoin, partiel pour tiers | événements injectés dans `App::events` ; compteur 1, 4, 1 puis 7 ; `keyUp` ne réincrémente pas ; pas de matériel vérifié |
| setTitle | vérifié pour le témoin | titre exact 1, 4 puis 7 après sauvegarde/relecture/redémarrage |
| setImage / setState / showOk / showAlert | prouvé dans l’état App pour le témoin, partiel pour le rendu physique | image SVG décodée en octets de la face, état 1 et signal `flash` vrai/faux observés après appui ; aucun rendu USB/écran physique vérifié |
| getSettings / setSettings / didReceiveSettings | prouvé pour le témoin, partiel pour tiers | Node demande les réglages ; App répond et sauvegarde chaque `setSettings` dans `config.json` temporaire, relu via `store::load`, deux touches isolées et nouveau processus Node ; pas de configuration utilisateur |
| getGlobalSettings / setGlobalSettings | prouvé pour le témoin, partiel pour tiers | Node demande et écrit les globaux ; App persiste dans `plugin-settings.json` temporaire ; après redémarrage Node accuse la valeur relue |
| enregistrement / erreurs | partiel | file d'attente des commandes jusqu'au handshake ; JSON malformé, événement inconnu, contexte étranger et arrêt de processus signalés ; timeout observé par oracle |

Limites après C1 : inspecteur HTML non testé de bout en bout, persistance du rapport en présence de fichiers non accessibles, session WebSocket authentifiée, rapport pour oracles déclarés de plugins tiers, rendu physique/image USB non vérifié, rétablissement après crash non prouvé, confirmations d'essais réels et capacités externes non testées. Processus séparé n'est pas sandbox ; jamais tester un tiers par défaut. Catalogue distant signé, conversion traçable et compatibilité multiplateforme restent hors de B.


## Étape C2b — frontières d’essai

L’oracle local `schema_version: 1` décrit l’action, le mode (`simulated` ou `real`), `real_effects`, les prérequis, le délai et une observation bornée (titre, image, état, fichier). `run` rapporte Réussi seulement après résultat observé par `validate_oracle` via `Event::Key/KeyRelease` dans une `App::init_with(..., false)` temporaire ; les cas sans oracle/prérequis/confirmation sont Non vérifié. Aucun essai réel n’est lancé par défaut. `openUrl` reçu par l’App isolée est enregistré, pas exécuté ; en revanche le JavaScript HTML peut appeler directement `window.open` : le harnais refuse tous les runtimes HTML avant de démarrer Edge. Il faudra intercepter aussi les fenêtres et le réseau dans un navigateur confiné avant d’essayer Tutorial. Le processus natif n’est pas sandboxé : ni logiciel/compte tiers ni capture Gizmos ne doivent être lancés sans analyse et accord. Matrice et empreintes : `PLUGINS-COMPATIBILITE.md`.

## D1 — tranche livrée, conformité restante

Le Job Object Windows KILL_ON_JOB_CLOSE regroupe le plugin et les inspecteurs lancés par DeckForge ; un test vérifie la fin d’un descendant à la fermeture du handle. L’affectation suit immédiatement le spawn et laisse encore une très courte fenêtre de course ; suspendre le processus dès la création pour supprimer cette fenêtre. Les navigateurs déjà démarrés avec un profil partagé restent à isoler dans un profil temporaire.

Trace opt-in : `DECKFORGE_PLUGIN_TRACE_UUID=<uuid>` avant lancement ; JSONL dans `%TEMP%/deckforge-plugin-traces/<uuid>-<pid>.jsonl`, métadonnées seulement (sens, commande, non-pris-en-charge), jamais de payload ni de réglages ; taille par processus limitée, événements inconnus listés dans la fiche. Ce n’est pas une preuve du comportement du plugin. La réponse `-info` utilise la version réelle de DeckForge, mais le modèle de périphérique est encore fixe (D6), les couleurs sont vides et `platformVersion` reste à lire dans Windows.

Restent à implémenter et vérifier : titleParametersDidChange, systemDidWakeUp et applications surveillées (didReceiveGlobalSettings est déjà envoyé depuis l’App), `switchToProfile` limité, version Nodejs.Version et contraintes OS/MinimumVersion, profil navigateur éphémère avec blocage et observation CDP de window.open/navigation/réseau, banc de conformité sur copies des plugins en clair (sans appui), et analyse causale Gizmos par trace. Aucun effet ni capture Gizmos lancés dans D1.

## D2 — tranche partielle, événements et appareil

Le lancement Exe/Node conserve -port, -pluginUUID, -registerEvent et -info ; l’injection HTML appelle connectElgatoStreamDeckSocket ou l’ancien connectSocket. Les appareils du champ devices reflètent la grille remontée par Event::DeviceModel au thread du gestionnaire, sinon la grille D6 virtuelle en validation. deviceDidConnect précède willAppear au lancement et au redémarrage ; le changement de modèle émet deviceDidDisconnect puis deviceDidConnect. willAppear, willDisappear, keyDown, keyUp, didReceiveSettings et l’inspecteur emploient les colonnes réelles de la grille ; test de contrat XL 8 × 4, index 17 → (1, 2). Le témoin Node WebSocket continue de tester le parcours. Aucun appareil physique n’a été ouvert ni pressé pour cette étape.

Limites D2 : l’identifiant SDK physique reste dérivé de la taille de grille et non du numéro de série (deux appareils de même grille sont confondus) ; le type SDK des modèles non reconnus est par défaut 0 ; connexion initiale sans modèle remonte comme virtuel. platformVersion est toujours la valeur de repli 10.0, colors vide et police vide. L’événement de déconnexion à l’arrêt de l’hôte n’est pas garanti, ni les transitions matérielles sans changement de grille. Pas encore de titleParametersDidChange, systemDidWakeUp, applicationDidLaunch/applicationDidTerminate ni sonde de processus. Ces points réclament une tranche ultérieure et des tests dédiés ; aucune conformité intégrale SDK v2 revendiquée.

## D3 — tranche runtime et limites

CodePathWin prévaut sur CodePath ; OS[] doit contenir windows, MinimumVersion est confrontée à la version Windows (build du registre ou valeur de test explicite), sinon lancement refusé avec prérequis explicite. Nodejs.Version sélectionne une version majeure compatible parmi PATH, FIFINE et Elgato ; les entrées SDK v2 bin/plugin.js demandent au moins Node 20. Le répertoire de travail reste le dossier du plugin pour Node et exe. Le magasin affiche les prérequis du manifeste. Tests unitaires couvrent manifeste et seuil SDK v2, sans démarrage de plugin tiers.

Les commandes usuelles étaient déjà gérées dans l'App isolée et l'hôte ; switchToProfile est désormais refusée explicitement plutôt que prétendue réussie. Restent non implémentés : bascule vers profil DeckForge existant, vérification du rendu setTitle/setImage par target et state, événements de titre configuré / réveil / applications surveillées, tests ciblés de chaque commande et démarrage SDK v2 réel. Aucune compatibilité totale revendiquée.


## D7 — diagnostic générique opt-in

`DECKFORGE_PLUGIN_TRACE_UUID=<uuid>` active seulement pour cet UUID une trace JSONL locale bornée dans `%TEMP%/deckforge-plugin-traces/`. Elle marque `tcp_accept`, `handshake_ok` ou `handshake_error` (catégorie fixe), le premier message pré-enregistrement (nom `event` filtré ASCII, 64 caractères au maximum, ou `first_malformed_json`), `register_plugin`, `no_register_timeout` après trois secondes de socket WebSocket sans enregistrement, fermeture et erreur WebSocket (catégorie seulement). Les compteurs `startup_counts` à la fin du thread donnent `tcp_accepted`, `handshakes_ok`, `registered` ; ils ne sont pas des preuves d'action utilisateur. Aucun en-tête, URL, contenu de message, `-info`, CommandLine, chemin, réglage ou texte libre du plugin n'est écrit. Une commande inconnue est signalée par `<unknown>` et `unsupported` sans conserver son texte libre. La trace est limitée en nombre d'enregistrements et par rotation du fichier à l'ouverture ; si elle sature, des événements et les compteurs peuvent manquer.

`TCP Established` signifie seulement qu'une connexion TCP existe : ce n'est ni une poignée de main WebSocket réussie, ni un `registerPlugin` validé, ni une action exécutée. Compter séparément connexions acceptées, handshakes réussis et enregistrements reçus avant de conclure. Le handshake est encore effectué sur le thread réseau avec timeout ; ces traces n'ajoutent pas de travail au thread UI. Aucun plugin tiers ni matériel n'est exercé par les tests D7.


## D8 — écriture WebSocket et file d'attente

La trace opt-in distingue désormais `host->plugin attempted` (avant écriture), `host->plugin sent` (écriture locale WebSocket réussie) et `host->plugin send_failed` (catégorie fixe `timeout`, `io`, `protocol`, `capacity` ou `other`). Les champs de commande, réglages, chemins et erreurs brutes ne sont jamais tracés ; un nom d'événement inconnu reste `<unknown>`. Une écriture réussie ne prouve ni réception par le processus distant, ni exécution, ni accusé applicatif. Un échec d'écriture remonte à l'App via `__hostError` puis invalide la connexion concernée ; l'inspecteur garde son routage `__pi` hors du message transmis. Les commandes destinées à un inspecteur absent produisent également une erreur explicite.

Avant l'enregistrement du plugin, le canal de commandes et la file réseau sont bornés à 64 éléments chacun ; une commande sans enregistrement expire après trois secondes dans la file réseau et génère `__hostError`. Si le canal est plein, `Host::send` ne bloque pas l'UI et signale une erreur. Les tests utilisent des sockets WebSocket loopback contrôlés (écriture réussie et socket local fermé avant l'envoi) ; ils n'exécutent aucun plugin tiers ni touche physique. Les traces restent opt-in et limitées en nombre d'entrées : leur absence n'est pas une preuve de livraison.

## D9a — composant proxy expérimental, tests uniquement

`plugin_validation_proxy.rs` est compilé uniquement sous `cfg(test)` ; il n’est pas relié au lancement HTML ni aux plugins tiers. Écoute loopback éphémère ; seuls GET/HEAD HTTP et upgrade WebSocket vers les deux ports loopback configurés sont transmis, par adresse/port/méthode exacts, sans CONNECT ni redirection. En-têtes strictement bornés, connexions/durée/flux bornés, délais de lecture et journal limité aux catégories et aux ports autorisés. Les fichiers du serveur de ressources restent soumis à `plugin_http.rs` ; le proxy ne remplace pas son contrôle des chemins (les composants de chemin symboliques y sont refusés). Tests hors ligne avec canaris locaux.

Ce proxy ne crée **aucun sandbox navigateur/OS** : Chromium peut contourner un proxy (DNS, UDP, extensions, accès direct). Le refus intégral des runtimes HTML dans `plugin_validation.rs` demeure. Pas de navigateur lancé, de plugin tiers essayé ni de garantie d’isolation. Avant toute activation future : confinement OS/navigateur démontré, audit des réponses HTTP et tests d’évasion.
