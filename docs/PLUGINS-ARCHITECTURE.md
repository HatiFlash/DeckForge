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

Les plugins exécutables tournent avec les droits de l'utilisateur : processus séparé ≠ sandbox. Le catalogue n'accorde aucun droit ; la compatibilité est « non vérifiée » jusqu'à un essai explicite. Ni archive chiffrée ni API non reconnue ne sont déchiffrées ou adaptées automatiquement. L'installation est une copie atomique sans liaison de touche ; affecter une action ultérieurement peut démarrer le plugin. SHA256 en métadonnée locale pour les archives seulement, non preuve d'éditeur.

Étape B (périmètre vérifié) : témoin MIT `tests/fixtures/counter.sdPlugin` (Node natif WebSocket, sans code GPL), action `org.deckforge.counter.increment`. Contrat : `willAppear` transmet les réglages et déclenche `getSettings` ; `didReceiveSettings` charge les paramètres `step` (>0 entier, sinon 1) et `count` ; chaque `keyDown` renvoie `setTitle` (nouveau total) et `setSettings` ; `keyUp` n'incrémente pas. `error` = `crash`, `timeout`, `malformed` ou `foreign` uniquement pour les essais contrôlés. La validation utilise une Config virtuelle et `Manager::key(index, true/false)`, identique aux appels `Event::Key/KeyRelease` du matériel, mais sans D6 ni USB. Elle compare le code Node au témoin embarqué avant toute exécution ; un plugin tiers (ou témoin modifié) n'est jamais exécuté par cette validation : résultat « Non vérifié » pour chaque action. Le rapport `validation-report.json` du dossier installé contient action, verdict, raison, mode simulé et date Unix ; le bouton « Tester le plugin (simulation uniquement) » travaille sur thread de fond. Aucun essai réel par défaut, aucun badge fonctionnel déduit de l'import. Reproduction : `cargo test --locked plugin_validation::tests` (Node avec WebSocket global requis).

| Événement | État après B | Preuve/limite |
| --- | --- | --- |
| willAppear / willDisappear | routés | ajout/reconfiguration/retrait de touche ; réception Node de willAppear vérifiée |
| keyDown / keyUp | routés | même `Manager::key` pour appareil et appareil virtuel ; compteur contrôlé |
| setTitle | vérifié pour le témoin | titre exact 1, 4 puis 7 après sauvegarde/relecture/redémarrage |
| setImage / setState / showOk / showAlert | partiel | chemin UI préexistant, pas d'oracle témoin pour tous ces résultats |
| getSettings / setSettings / didReceiveSettings | vérifié pour le témoin, partiel pour tiers | deux contextes indépendants, sauvegarde et relecture via `store` en configuration virtuelle ; le chemin UI sur profil utilisateur n'est pas exercé |
| getGlobalSettings / setGlobalSettings | partiel | stockage UI préexistant, non validé en bout en bout |
| enregistrement / erreurs | partiel | file d'attente des commandes jusqu'au handshake ; JSON malformé, événement inconnu, contexte étranger et arrêt de processus signalés ; timeout observé par oracle |

Limites pour C : inspecteur HTML et réglages globaux testés de bout en bout, persistance du rapport en présence de fichiers non accessibles, session WebSocket authentifiée, rapport pour oracles déclarés de plugins tiers, vérification du rendu UI avec harnais, image/état/alertes observables, rétablissement testé après crash, confirmations d'essais réels et capacités externes. Processus séparé n'est pas sandbox ; jamais tester un tiers par défaut. Catalogue distant signé, conversion traçable et compatibilité multiplateforme restent hors de B.
