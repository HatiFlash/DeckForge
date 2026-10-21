# Corpus de manifestes plugins — inventaire statique

Date UTC : 2026-10-18
Empreinte SHA256 du corpus (SHA256 des SHA256 des manifestes uniques triés, séparés par LF et terminés par LF) : `3fab8b14737b6507d203c853574db5c2e8d87d8e947fcd64ca4b92a93e4b0fcd`

| Mesure | Nombre |
| --- | ---: |
| found | 30 |
| unique | 11 |
| duplicates | 5 |
| rejected encrypted | 11 |
| rejected invalid | 3 |
| rejected unsafe | 0 |
| rejected oversize | 0 |
| missing/unsafe root | 0 |
| runtime exe | 5 |
| runtime node | 0 |
| runtime html | 3 |
| runtime none/other | 3 |
| actions | 110 |
| OS prerequisites | 11 |
| Node version prerequisites | 0 |
| PropertyInspectorPath | 3 |
| ApplicationsToMonitor manifests | 0 |
| ApplicationsToMonitor entries | 0 |

Deux racines locales fournies en arguments ; seuls les manifest.json des sous-dossiers immédiats sont lus (1 Mio maximum chacun). Aucun fichier de ressource, de code ou de réglages de plugin lu ; aucun plugin exécuté, copié ou modifié. Les chemins symboliques/jonctions sont ignorés. Les manifestes chiffrés, illisibles, illégaux ou aux chemins connus risqués sont rejetés ; déduplication par UUID, version et SHA256 du manifeste brut. Les champs inconnus ne sont pas interprétés.

Les nombres décrivent les déclarations des manifestes, pas la présence des exécutables, des logiciels requis ni le fonctionnement. Aucune conformité runtime ou compatibilité NovyDeck/SDK n’est prouvée. Aucun appui, événement willAppear, appel réseau ou lancement d’application.
