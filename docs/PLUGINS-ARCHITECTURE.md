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

Reste pour B : plugin témoin compteur, deux instances, protocole et persistance après redémarrage, tests virtuels sans matériel. À approfondir ensuite : signature d'un catalogue distant, isolation réelle, vérification des exigences de plateformes, conversion traçable et inspection des capacités SDK.
