# NovyDeck 0.8.13 — audio switch on a key

English · [Français](#français)

## English
Dragging the **Basculer l'audio** (audio switch) action onto a key was rejected with “Affectation refusée : Choisis deux périphériques audio différents”, because the key was validated before its two devices could be chosen.
- The action can now be dropped first; a message then asks you to choose the two audio devices in the key inspector.
- Choosing the same device twice is still rejected.
- Pressing the key before both devices are set does nothing and shows a clear message instead of an error.

Validation: a UI test reproduces the refusal on 0.8.12 and passes on 0.8.13; 395 tests passed (20 ignored), MCP stdio/HTTP passed, upgrade 0.8.12 → 0.8.13 keeps configuration, media and plugins. [Test details](https://github.com/HatiFlash/NovyDeck/blob/v0.8.13/docs/TEST-0.8.13.md).

## Français
Glisser l'action **Basculer l'audio** sur une touche était refusé avec « Affectation refusée : Choisis deux périphériques audio différents », car la touche était validée avant que ses deux périphériques puissent être choisis.
- L'action peut maintenant être déposée d'abord ; un message invite ensuite à choisir les deux périphériques audio dans l'inspecteur de la touche.
- Choisir deux fois le même périphérique reste refusé.
- Un appui sur la touche avant le choix des deux périphériques ne fait rien et affiche un message clair au lieu d'une erreur.

Validation : un test UI reproduit le refus sur la 0.8.12 et réussit sur la 0.8.13 ; 395 tests réussis (20 ignorés), MCP stdio/HTTP réussis, mise à jour 0.8.12 → 0.8.13 avec configuration, médias et plugins conservés. [Détail des contrôles](https://github.com/HatiFlash/NovyDeck/blob/v0.8.13/docs/TEST-0.8.13.md).
