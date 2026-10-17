# DeckForge 0.5 — fiabilité (lot 1)

- Dialogues de fichiers et dossiers déplacés hors du verrou de l’application ; les résultats reviennent au frame suivant via un canal. La lecture Stream Deck et l’analyse des packs d’icônes sont également faites sur le thread de dialogue. Les sélections d’étapes sont appliquées à la touche/page d’origine, si elle existe encore.
- Sauvegarde `config.json.bak` par fichier temporaire puis remplacement atomique. Un `config.json` invalide peut être récupéré depuis la sauvegarde sans détruire celle-ci ; récupération et erreur de sauvegarde à la fermeture consignées dans `deckforge-error.log`.
- Import Stream Deck : rejet des codes de touche Windows dépassant `u16`. Jetons MCP comparés à durée indépendante du préfixe commun. HTTP MCP limité à huit connexions actives, avec réponse 503 en cas de saturation et délais existants.
- Bouton renommé « Arrêter les macros en cours » : il n’arrête pas les applications déjà ouvertes.

## Vérifications exécutées

- `tools/Build-Windows.ps1 -Action All` : 78 tests de l’application et 5 tests du binaire MCP réussis ; tests Node stdio et HTTP réussis (dont huit clients lents, refus 503 et reprise) ; compilation sans avertissement de build.
- `cargo clippy --locked --all-targets` : réussi ; 31 avertissements dans la cible application-tests (base 33 avant ce lot) et 2 dans le binaire MCP avant les deux simplifications mineures finales.

## Restant sur matériel / sécurité

- Vérifier sur D6 physique les appuis USB et MCP pendant un dialogue Windows prolongé, ainsi que l’import de packs et les chemins d’icônes/étapes. Aucun programme natif ni macro réelle n’a été lancé pendant ces tests.
- La restriction ACL du fichier `mcp-connection.json` à l’utilisateur courant n’est pas encore appliquée ; le jeton reste local mais dépend des permissions du dossier contenant l’exécutable.

## Lot 2 - Interface et édition

- Copier / coller / dupliquer une touche (menu clic droit sur l'aperçu, Ctrl+C / Ctrl+V), y compris entre pages et profils.
- Annuler / Rétablir multi-niveaux (50 étapes, frappes regroupées par champ) : boutons en haut, Ctrl+Z / Ctrl+Y. Retrait d'étape annulable.
- Simuler devient le bouton principal ; Tester dans 3 s est secondaire. Flèches ↑ puis ↓ partout.
- Bouton de thème explicite ; indicateur D6 coloré (connecté / en attente / déconnecté) ; aperçu du deck agrandi.
- Correctif : la Galerie ciblait l'ancienne touche après changement de sélection (Touche 07 au lieu de 08).
- Bouton Retirer l'image intégré à la ligne Galerie (la colonne de l'inspecteur était coupée).
- Seuls des glyphes présents dans les polices egui par défaut sont utilisés (↶ ↷ ✕ s'affichaient en carrés).

Tests exécutés : Build-Windows.ps1 -Action All : 81 tests application, 5 tests connecteur, MCP stdio et HTTP OK ; aperçus régénérés et contrôlés visuellement.
## Lot 3a/3b - Atelier de blocs

- L'atelier s'ouvre dans la zone centrale (plus de fenêtre flottante qui masque le deck) avec Retour au deck ; la bibliothèque de droite est masquée pendant l'édition.
- Palette de blocs par catégories avec recherche ; ajout par clic ; + Insérer ici entre deux blocs.
- Cartes repliables avec résumé lisible (ex. Attendre 250 ms, Mémoriser mode = travail), barre de couleur par catégorie, boutons ↑ ↓ Dupliquer Désactiver Retirer.
- Bloc désactivé : conservé mais ignoré à l'exécution, en simulation, à la validation et à l'export AutoHotkey. Sérialisé comme {"type":"disabled","step":{...}}.
  ATTENTION retour arrière : DeckForge 0.4.3 ne connaît pas ce type et refusera une configuration contenant un bloc désactivé. Réactiver les blocs ou garder config.json.bak avant de revenir en 0.4.3.
- Bloc invalide signalé localement (bordure orange + message).
- Simuler les blocs est le bouton principal.

Tests exécutés : Build-Windows.ps1 -Action All : 86 tests application, 5 tests connecteur, MCP stdio/HTTP OK ; workflow.png contrôlé.
## Lot 3c - Atelier : guider le débutant, peaufiner pour l'expert

- Six modèles prêts à personnaliser (application puis site, copier puis coller, signature, casque / enceintes, mode travail, répéter une touche) ; remplacer ou ajouter à la fin.
- Mode Simple / Avancé mémorisé dans `preferences.json` : le mode Simple montre les champs essentiels et une phrase d'aide sous chaque bloc ; aucun réglage n'est perdu en changeant de mode.
- Déroulé de la simulation : chaque bloc simulé, durée, branche prise, itérations, blocs désactivés sautés ; clic = carte surlignée. Aucune action réelle.
- Raccourcis : Ctrl+D dupliquer, Suppr retirer (annulable), Alt+↑/↓ déplacer. En-tête de l'atelier sur une ligne.

## Lot 4a - Appui long et taille du titre

- Le D6 envoie aussi les relâchements : ils sont maintenant décodés. Nouveau champ par touche `long_steps` et réglage global `long_press_ms` (250 à 2000, défaut 500).
- Sans `long_steps` : exécution immédiate à l'appui, aucune latence ajoutée. Avec : relâchement avant le seuil = actions courtes ; seuil atteint = actions longues une seule fois ; le relâchement ensuite ne déclenche rien. Révision synchronisée et filtre de rebond respectés.
- Si un firmware n'envoyait jamais de relâchement, une touche avec appui long déclencherait toujours l'action longue au seuil (à vérifier sur le D6).
- Inspecteur : sélecteur Appui court / Appui long ; MCP `deckforge_configure_key` accepte `long_steps`.
- Taille du titre par touche (`title_size`), appliquée au rendu canonique partagé aperçu / D6.

## Lot 4b - Profil automatique et veille

- `Profile.apps` (exécutables associés, ex. `obs64.exe`) et `Config.auto_profile` : bascule selon l'application au premier plan, retour au profil choisi manuellement, pas de bascule pendant une macro, fenêtre DeckForge ignorée.
- Veille : `sleep_after_min` (0 = jamais) et `sleep_brightness` ; luminosité envoyée seule (LIG) sans redessiner ; l'appui de réveil n'exécute pas la macro.

## Lot 4c - Icônes animées et taille d'icône

- `icon_scale` (20 à 200 %, défaut 100 = rendu 0.4.3) avec curseur et réinitialisation ; mouvements regroupés dans l'historique ; MCP `configure_key` accepte `icon_scale`.
- GIF animés bornés (10 s, 150 images, 20 Mo) pré-rendus image par image par le rendu canonique ; aperçu animé à partir des mêmes JPEG.
- D6 : mise à jour d'une seule touche par trame (BAT + données + STP), débit global ≤ 30 images/s, cadence par touche ≤ `animation_fps` (défaut réglable, 1 à 15), jamais pendant un envoi complet, appuis lus entre les trames. Réglages `d6_animations` et `animation_fps`. Variante de terminaison BAT + ULEND + STP via la variable d'environnement `DECKFORGE_D6_FRAME_ULEND` si le firmware l'exige.
- Vidéos MP4 / WebM / MOV / MKV / AVI converties en GIF dans `assets/mes-icones` via FFmpeg (PATH, à côté de l'exe ou installation winget Gyan.FFmpeg), hors du fil d'interface, sans console, délai 60 s ; message clair si FFmpeg est absent. Badge « GIF » dans la galerie.
- Dépendance ajoutée : feature `gif` du crate `image`.

## État final de la branche claude/0.5

- `tools/Build-Windows.ps1 -Action All` : 102 tests de l'application, 5 tests du connecteur MCP, contrôles MCP stdio et HTTP réussis ; aucun avertissement de compilation.
- Clippy installé : 30 avertissements au plus (33 au départ), tous de style.
- Aperçus de l'interface régénérés sans fenêtre native et contrôlés visuellement à chaque lot.

## Correctif inspecteur - affichage des tailles

- Colonne Apparence défilable avec identifiant stable, taille conservée dans l’inspecteur ; disposition resserrée pour la taille de l’icône et bouton Défaut compact.
- Taille du titre affichée en pixels (« 14 px ») ; taille de l’icône conservée en pourcentage.
- Test egui sans fenêtre : à 1380 × 1050, vérifie les unités et l’accès à la taille de l’icône par défilement si nécessaire.
- Tests : cargo test --locked (103 application, 5 connecteur) ; Build-Windows.ps1 -Action All avec aperçus dans %TEMP%\df-fix1 : terminé sans avertissement de compilation, contrôles MCP stdio/HTTP OK ; clippy --locked --all-targets : code 0, 28 lignes warning (dont résumés), limite 30 respectée.
- À vérifier sur matériel : défilement à la souris dans la fenêtre réelle et rendu sur le pavé D6.

## Retours après essai D6 - inspecteur et atelier

- Cartes d’étapes bornées par la largeur de l’inspecteur ; défilement vertical sans extension horizontale, éditeurs de texte ajustés à la place disponible. Boutons ↑ ↓ × accessibles dans le panneau central même avec un long script AutoHotkey.
- Ouverture depuis la notification : restauration native seulement si la fenêtre est réellement cachée/réduite, et commandes viewport demandées seulement pour l’ouverture explicite, selon l’état connu (pas pour Applied, Device ni l’animation). Ancien doublon : SW_RESTORE + SetForegroundWindow natifs sur chaque événement Open, puis Visible + Minimized + Focus egui ; aucune commande répétée sur les frames inactives observée. Le rafraîchissement de dessin à 60 ms reste nécessaire au moteur/UI ; aucune preuve que le clignotement réel observé vient exclusivement de ce doublon.
- Atelier de blocs rendu visible dans l’inspecteur, aide sans code dans l’étape AutoHotkey, et accès à l’atelier ou aux modèles quand la touche est vide.
- Tests egui sans fenêtre : boutons de suppression visibles à 1380×900 et 1100×800 avec script long et média ; accès atelier/modèles et aucune commande viewport sur 8 frames stables. `Build-Windows.ps1 -Action All -PreviewDirectory %TEMP%\df-fix2` : 105 tests app et 5 tests MCP, transports stdio et HTTP validés, zéro warning de build. `cargo clippy --locked --all-targets` : 26 lignes warning (dont doublons et résumés), exit 0 ; 21 warnings binaires et 26 pour tests, dont 21 doublons.
- À vérifier sur le PC et le D6 : absence de clignotement après redémarrage et ouverture/fermeture tray, lisibilité à l’échelle Windows réelle ; pas d’exécution de macros ou d’ouverture native pendant ces tests.

## Correctif - clignotement de la fenêtre

- Cause mesurée sur la vraie fenêtre (captures PrintWindow en rafale) : l'aperçu animé des GIF présentait par moments une image vide (fond par défaut d'eframe) ; 6 à 11 images vides pour environ 550 captures, jusqu'à 55 % d'un cœur. Bissection : introduit par l'aperçu animé du lot 4c.
- Une seule texture par touche animée mise à jour sur place, cadence plafonnée, une seule demande de rafraîchissement, décodage des GIF dans un thread ; redessin continu à 60 ms supprimé ; fond de fenêtre opaque aux couleurs du thème.
- GIF de plus de 2048 px refusés pour l'animation (image fixe) ; à l'import, un GIF de plus de 512 px est réduit automatiquement à 256 px.
- Mesure après correctif, configuration de l'utilisateur avec deux GIF : 0 image vide sur 658 captures, environ 19 % d'un cœur (0.4.3 : 21 %). Animation visible dans l'aperçu et sur le D6.
- Reste possible : mettre en cache les trames D6 entre deux synchronisations (non nécessaire à ce stade).
## Fusion 0.5 — actions natives et plugins Stream Dock

- Fusion de `claude/native` et `claude/plugins` dans `claude/0.5` : OBS, son, souris, réseau, Toggle et plugins avec Property Inspector, sans retirer les 11 outils MCP.
- Conflits résolus dans Cargo.lock, main.rs, model.rs, key_render.rs, ui.rs, ui_key_preview.rs et ui_tests.rs. L'affichage conserve les états audio/OBS et le badge Toggle sur les touches à étapes ; la face du plugin prime pour une touche liée à un plugin. Le rendu du badge est partagé avec la face plugin.
- Test de régression ajouté : la face plugin remplace l'icône native tout en conservant le badge d'état. Test ciblé exécuté et réussi.
- Vérifications exécutées : `Build-Windows.ps1 -Action All -PreviewDirectory %TEMP%\df-merge05` (161 tests application, 6 tests MCP, transports MCP stdio/HTTP validés, aucune alerte de compilation) ; `cargo clippy --locked --all-targets` (code 0, 29 avertissements uniques dans la cible tests et 23 doublons pour la cible application, sous le plafond de 30). La suite complète et Clippy ont été relancés après ce test de régression, tous deux avec code 0.
- À vérifier sur le matériel : LCD et appuis physiques du D6, états OBS, son, réseau, Toggle et Property Inspector des plugins ; aucun lancement de fenêtre ni commande USB réelle pendant la fusion.