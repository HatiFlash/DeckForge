# DeckForge 0.8.1 — correctifs et confort

## Correctifs prioritaires
- **Onglet Plugins** : il ne gèle plus l'application. La liste des plugins était relue à chaque image de l'interface et lançait `reg.exe` (rafale de fenêtres console). Elle est désormais lue une fois (bouton « Actualiser la liste »), la version de Windows est lue directement dans le registre et la sonde Node.js est mise en cache, sans console.
- **Windows Defender** : l'exécutable embarque maintenant son icône, ses informations de version (éditeur HatiFlash, produit DeckForge, version) et un manifeste Windows (`asInvoker`, DPI par écran). Cela réduit les faux positifs heuristiques ; un binaire non signé peut toutefois encore être signalé par SmartScreen/Defender.

## Nouveautés
- **Installateur Windows** `DeckForge-0.8.1-Setup.exe` : installation sans droits administrateur dans `%LOCALAPPDATA%\Programs\DeckForge`, raccourcis menu Démarrer et Bureau (optionnel), désinstallation propre. Profils, préférences, icônes et plugins sont conservés lors des mises à jour et de la désinstallation. L'archive ZIP reste disponible.
- **Thèmes** : thème clair refait, quatre styles (Bleu nuit, Graphite, Clair, Sable) et cinq couleurs d'accent dans Réglages → Thème de l'interface.
- **Assistant de l'atelier** : comprend davantage de demandes naturelles en français et en anglais (ex. « ferme toutes les fenêtres sauf celle sur laquelle je suis », « réduis les autres fenêtres », « affiche le bureau », « fais une capture d'écran », « wait 2 seconds then open https://… »). Nouveaux blocs : *Fermer les autres fenêtres* et *Réduire les autres fenêtres* (la fenêtre active et DeckForge sont toujours conservés ; les applications peuvent proposer d'enregistrer).
- **Fenêtre de l'assistant** : textes et boutons ne sont plus coupés, panneau redimensionnable, destination lisible, choix « Transmettre à mon IA / Non merci » visible sans défiler.
- **Galerie d'icônes** : bouton « Importer une icône… » (image, GIF ou vidéo) appliqué directement à la touche, avec passage par l'éditeur de recadrage.
- **Aperçu des touches** : les images en portrait sont cadrées vers le haut (le haut n'est plus coupé), le titre utilise un dégradé avec ombre au lieu d'un bandeau noir, et un bouton « Recadrer » est disponible sous l'aperçu.

## Compatibilité
Les profils et préférences existants sont conservés ; les nouveaux réglages de thème ont des valeurs par défaut.
