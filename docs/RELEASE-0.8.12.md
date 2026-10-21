# NovyDeck 0.8.12 — community plugins and hardware reports

English · [Français](#français)

## English

### Community plugin library
A new **Communauté** (Community) section in **Bibliothèque → Plugins** lists plugins shared by the community in [HatiFlash/novydeck-plugins](https://github.com/HatiFlash/novydeck-plugins).
- NovyDeck stays offline by default: it connects to GitHub only after you click **Se connecter à la bibliothèque** (consent remembered for the current session only).
- Before installing, a confirmation shows the plugin, its author, download address and expected SHA-256, with a warning that third-party plugins run code with your user rights.
- The download (HTTPS only, 50 MB maximum) is checked against its SHA-256 before anything is installed; a mismatch installs nothing. Installation then uses the existing plugin import.
- Community plugins are marked **Communauté — non vérifié** (unverified). Authors propose plugins by pull request; an automated check validates each entry and the maintainer approves it by merging.

### Hardware report
Testing a deck brand that has not been tested yet? **Signaler un problème avec mon deck** (Report a problem with my deck), in **À propos** and next to the device tools:
- prepares an anonymized report (app and Windows versions, detected HID devices without serial numbers, connection state, recent app log), with user name, computer name, personal paths and e-mail addresses masked;
- shows a preview first; nothing is sent automatically;
- saves the text file, selects it in Explorer and opens a pre-filled GitHub issue using the new **Hardware report** form, where you add your deck model and the problem, then drag the file in.

### Fix
- The **Recadrer** (Crop) button under the key preview in the inspector was cut off at the bottom of the panel. The preview now shrinks to leave room for both buttons, which stay on one line.

### Validation
394 tests passed (20 ignored), MCP stdio/HTTP tests passed, Windows build valid. The UI test for the Crop button fails on 0.8.11 (button 4.5 px below the panel edge at 820 px) and passes on 0.8.12. The community library was checked against the real GitHub catalog over HTTPS. Upgrade 0.8.11 → 0.8.12 on the test bench keeps configuration, media and plugins. [Test details](https://github.com/HatiFlash/NovyDeck/blob/v0.8.12/docs/TEST-0.8.12.md).

Profiles, actions, macros, existing plugins and media are unchanged.

## Français

### Bibliothèque communautaire de plugins
Une nouvelle section **Communauté** dans **Bibliothèque → Plugins** liste les plugins partagés par la communauté dans [HatiFlash/novydeck-plugins](https://github.com/HatiFlash/novydeck-plugins).
- NovyDeck reste hors ligne par défaut : il ne se connecte à GitHub qu'après un clic sur **Se connecter à la bibliothèque** (accord mémorisé pour la session en cours seulement).
- Avant l'installation, une confirmation affiche le plugin, son auteur, l'adresse de téléchargement et l'empreinte SHA-256 attendue, avec un avertissement : un plugin tiers exécute du code avec vos droits d'utilisateur.
- Le téléchargement (HTTPS uniquement, 50 Mo au maximum) est comparé à son SHA-256 avant toute installation ; en cas d'écart, rien n'est installé. L'installation passe ensuite par l'import de plugins existant.
- Les plugins communautaires sont signalés **Communauté — non vérifié**. Les auteurs les proposent par pull request ; une vérification automatique contrôle chaque fiche et le mainteneur valide en fusionnant.

### Rapport matériel
Vous testez une marque de deck pas encore testée ? **Signaler un problème avec mon deck**, dans **À propos** et près des outils du périphérique :
- prépare un rapport anonymisé (versions de l'application et de Windows, périphériques HID détectés sans numéro de série, état de connexion, journal récent) en masquant nom d'utilisateur, nom de l'ordinateur, chemins personnels et adresses e-mail ;
- affiche d'abord un aperçu ; rien n'est envoyé automatiquement ;
- enregistre le fichier texte, le sélectionne dans l'Explorateur et ouvre une issue GitHub préremplie avec le nouveau formulaire **Hardware report**, où vous indiquez le modèle du deck et le problème avant d'y glisser le fichier.

### Correction
- Le bouton **Recadrer** sous l'aperçu de la touche, dans l'inspecteur, était coupé en bas du panneau. L'aperçu rétrécit désormais pour laisser la place aux deux boutons, qui restent sur une seule ligne.

### Validation
394 tests réussis (20 ignorés), tests MCP stdio/HTTP réussis, compilation Windows valide. Le test UI du bouton Recadrer échoue sur la 0.8.11 (bouton 4,5 px sous le bord du panneau à 820 px) et réussit sur la 0.8.12. La bibliothèque communautaire a été vérifiée avec le vrai catalogue GitHub en HTTPS. La mise à jour 0.8.11 → 0.8.12 sur le banc conserve configuration, médias et plugins. [Détail des contrôles](https://github.com/HatiFlash/NovyDeck/blob/v0.8.12/docs/TEST-0.8.12.md).

Les profils, actions, macros, plugins existants et médias ne changent pas.
