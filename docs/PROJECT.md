# DeckForge — synthèse du projet

**16 octobre 2026** · Dépôt privé : [HatiFlash/DeckForge](https://github.com/HatiFlash/DeckForge).

## Objectif et matériel

Remplacer l’application FIFINE par un contrôleur Windows natif en Rust,
priorité aux macros et automatisations, avec une interface épurée.
Le D6 testé est la révision **`3142:0060`**, à 15 touches LCD.

Les bases publiques mirajazz, duby, FifineControlDeck et opendeck-ampgd6 ont été
étudiées. Leurs révisions USB et leur support Windows diffèrent ; les informations
ont été recoupées avec le matériel réel.

Le test Windows confirme : rapports de sortie 1024 octets, JPEG 126×126 tournés
à 180°, inversion des lignes à l’affichage, appuis en ordre naturel.
Les captures `ACK … OK` contiennent le numéro à l’octet 9 et l’état à l’octet 10 :
`1` à l’appui, `0` au relâchement. L’utilisateur a confirmé les 15 numéros affichés ;
les appuis 01 → 08 → 15 ont été reçus une fois chacun après correction.

L’app FIFINE installée, 3.10.194.0805, possède `SDLibrary1.dll` et ses symboles PDB.
L’examen des exports et du lecteur HID a permis de recouper `readDataFromHidDevice`,
`hid_read_timeout` et le décodage des champs d’entrée. Aucun binaire propriétaire
n’est embarqué dans DeckForge.

## Version 0.2

La 0.1 démarrait en diagnostic : « Lecture » fonctionnait via le bouton de test,
mais un appui sur le D6 sélectionnait seulement la touche. La 0.2 remplace ce
fonctionnement par **Touches actives**, visible et activé après synchronisation.

La nouvelle version ajoute connexion et sauvegarde automatiques, confirmations
USB par révision, rejet des appuis obsolètes, interface claire à cartes arrondies,
icônes vectorielles, aperçu des images et journal repliable.

Le connecteur MCP facultatif propose stdio et HTTP local protégé, avec 8 outils.
Une IA peut lire les profils, préparer des macros et demander une exécution.
Les changements et les exécutions réelles sont validés dans l’app.

## Architecture

- `model.rs` : profils, touches, étapes et validation.
- `device.rs` : HID, protocole CRT, images, appuis, reconnexion.
- `control.rs` : état des touches, révisions, rejet des appuis anciens.
- `engine.rs` : exécution séquentielle, saisie Windows, annulation.
- `store.rs` : sauvegarde atomique et copie précédente.
- `ui.rs` / `theme.rs` : interface egui et thème clair.
- `assistant.rs` : propositions IA et détection d’obsolescence.
- `mcp_server.rs` : pont local authentifié vers l’app.
- `deckforge-mcp` : transports MCP stdio et HTTP.

## Validation et suite

Les captures réelles sont rejouées jusqu’au moteur avec des actions de délai
sans effet sur le PC. Les tests couvrent aussi sauvegarde, annulation,
répétitions, propositions IA et échanges MCP. Le rendu normal et compact a été
vérifié ; la connexion réelle de la 0.2 aboutit à **Touches actives**.
Voir [VALIDATION.md](VALIDATION.md) pour les résultats et limites.

Prochaines étapes : endurance et veille/reprise, import de profils FIFINE avec
aperçu, OBS WebSocket, audio par application, actions Discord, variables,
conditions, sous-macros, enregistreur de raccourcis, notification et démarrage Windows.

La rétro-ingénierie sert à comprendre comportements et formats. Un programme
Qt/C++ compilé ne se transforme pas directement en sources Rust : les fonctions
utiles sont réimplémentées et testées progressivement.
