# NovyDeck 0.8.14 — in-app updates

English · [Français](#français)

## English
The **About → Vérifier les mises à jour** button now checks the latest published GitHub release directly, instead of comparing commits through a separately installed `gh` command.
- For installed copies, **Télécharger et installer** asks for confirmation, downloads the official installer, verifies its SHA-256 and announced size, saves the configuration, then closes NovyDeck and runs the installer silently. The installer is configured to reopen NovyDeck afterward.
- Portable copies are directed to the release page instead of being replaced automatically.
- Network and integrity errors are reported; an unverified installer is never launched.
- This first update-enabled version must be installed manually from the release page. Later updates can use the in-app button.

Windows VM validation: build, installer, MCP stdio/HTTP and Rust tests passed; the 0.8.13 → 0.8.14 installer upgrade kept configuration/media/plugin witnesses, and the published 0.8.13 installer was downloaded and SHA-256 verified by the new code. The Inno Setup relaunch entry was invoked, but this headless VM has no compatible WGPU adapter, so the reopened graphical process immediately exits; GUI relaunch remains unverified on a GPU-equipped desktop. See [test details](https://github.com/HatiFlash/NovyDeck/blob/v0.8.14/docs/TEST-0.8.14.md).

## Français
Le bouton **À propos → Vérifier les mises à jour** interroge maintenant directement la dernière release GitHub publiée, au lieu de comparer les commits via la commande `gh` à installer séparément.
- Pour une copie installée, **Télécharger et installer** demande confirmation, télécharge l'installateur officiel, vérifie son SHA-256 et sa taille annoncée, enregistre la configuration, puis ferme NovyDeck et lance l'installation silencieuse. L'installateur est configuré pour rouvrir NovyDeck ensuite.
- Une copie portable est renvoyée vers la page de la release, sans remplacement automatique.
- Les erreurs réseau ou d'intégrité sont indiquées ; un installateur non vérifié n'est jamais lancé.
- Cette première version dotée du bouton doit encore être installée manuellement. Les mises à jour suivantes pourront passer par l'application.

Validation sur VM Windows : compilation, installateur, MCP stdio/HTTP et tests Rust réussis ; la mise à jour 0.8.13 → 0.8.14 a conservé les témoins de configuration/médias/plugins, et le nouvel outil a téléchargé l'installateur 0.8.13 publié en vérifiant son SHA-256. L'entrée de relance Inno Setup a été exécutée, mais cette VM sans adaptateur WGPU compatible ferme immédiatement le processus graphique rouvert ; la relance sur un bureau avec GPU n'est pas vérifiée. [Détail des contrôles](https://github.com/HatiFlash/NovyDeck/blob/v0.8.14/docs/TEST-0.8.14.md).
