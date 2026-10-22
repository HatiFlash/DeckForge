# Translating NovyDeck / Traduire NovyDeck

English · [Français](#français)

## English

NovyDeck's interface is written in French in the source code; every other language is a JSON file in [`res/i18n/`](../res/i18n). Files placed there are built into the application automatically: no code change is needed to add a language.

```json
{
  "language": "Español",
  "strings": {
    "Réglages": "Ajustes",
    "Nouvelle version {version} disponible": "Nueva versión {version} disponible"
  }
}
```

1. Copy `res/i18n/en.json` to `res/i18n/<code>.json`, where `<code>` is the lowercase ISO 639-1 code of the language (`es`, `de`, `it`, `pt-br`…).
2. Set `"language"` to the language name written in that language (it appears in **Settings → Language**).
3. Translate the **values** only. Never change the keys (the French text on the left). Keep every `{name}` placeholder exactly as it is; you can move it within the sentence.
4. Untranslated or empty values automatically fall back to English, so a partial translation is fine.
5. Check the file with `cargo test --locked i18n::` if you can build NovyDeck, then open a pull request. CI rejects invalid JSON and mismatched placeholders.

Proper names (NovyDeck, OBS, Stream Deck, FIFINE…) are not translated. Short labels must stay short: they sit on buttons.

## Français

L'interface est écrite en français dans le code ; chaque autre langue est un fichier JSON dans [`res/i18n/`](../res/i18n), intégré automatiquement à la compilation. Ajouter une langue ne demande aucune modification du code.

1. Copiez `res/i18n/en.json` vers `res/i18n/<code>.json` (`<code>` = code ISO 639-1 en minuscules : `es`, `de`, `it`, `pt-br`…).
2. Indiquez dans `"language"` le nom de la langue écrit dans cette langue (affiché dans **Réglages → Langue**).
3. Traduisez uniquement les **valeurs**, jamais les clés (texte français à gauche). Conservez chaque variable `{nom}` telle quelle ; vous pouvez la déplacer dans la phrase.
4. Une valeur absente ou vide retombe sur l'anglais : une traduction partielle est acceptée.
5. Vérifiez avec `cargo test --locked i18n::` si vous pouvez compiler NovyDeck, puis ouvrez une pull request. La CI refuse un JSON invalide ou des variables différentes.

Les noms propres (NovyDeck, OBS, Stream Deck, FIFINE…) ne se traduisent pas. Les libellés courts doivent le rester : ils figurent sur des boutons.
