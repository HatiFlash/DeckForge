# Connecter une IA à DeckForge

Le connecteur propose **8 outils MCP**, via stdio ou Streamable HTTP local.
DeckForge valide les profils et exécute les actions. Il n’appelle aucun modèle
lui-même et ne demande aucune clé OpenAI/Anthropic.

## Client local : Claude, Codex et autres clients stdio

1. Ouvre DeckForge et active **Assistant IA** dans la barre latérale.
2. Clique **Copier la configuration MCP**.
3. Ajoute cette entrée à la configuration MCP du client choisi, sans remplacer
   ses autres serveurs, puis reconnecte ce client.

Exemple ; le bouton de l’app fournit le chemin réel :

```json
{
  "mcpServers": {
    "deckforge": {
      "command": "C:\\Applications\\DeckForge\\deckforge-mcp.exe",
      "args": []
    }
  }
}
```

Le client lance le connecteur, qui parle JSON-RPC sur stdin/stdout et rejoint
l’app par un pont local authentifié. L’app doit rester ouverte. Tant que
l’assistant est désactivé, les appels aux outils sont refusés.
Les fichiers des comptes IA ne sont pas modifiés automatiquement.

Exemple de demande :

> Lis mes profils DeckForge et propose une macro sur la touche 6 du profil Bureau :
> ouvrir mon application, attendre sa fenêtre, puis envoyer Ctrl+Shift+S.
> Attends ma validation dans DeckForge avant de considérer la proposition appliquée.

## ChatGPT web ou client HTTP

Active l’assistant, puis clique **Démarrer HTTP**.

- Endpoint : `http://127.0.0.1:9846/mcp`
- Authentification : `Authorization: Bearer <clé HTTP>`
- La clé est accessible avec **Copier la clé HTTP**.
- L’initialisation retourne `Mcp-Session-Id` ; les requêtes suivantes transmettent
  cet identifiant et `MCP-Protocol-Version`.
- Réponses JSON ; GET retourne 405 car aucun flux SSE n’est proposé.

Un service distant ne peut pas joindre directement localhost sur ton PC.
Pour ChatGPT, utilise un mode local pris en charge ou **Secure MCP Tunnel** / un
proxy HTTPS authentifié adapté au client. La disponibilité dépend du produit
et du compte. Aucun serveur distant ni compte IA n’est configuré automatiquement.

Sources officielles : [OpenAI MCP servers](https://developers.openai.com/api/docs/guides/tools-connectors-mcp),
[Secure MCP Tunnel](https://developers.openai.com/blog/connect-private-mcp-servers-to-openai-products),
[plugins locaux ChatGPT](https://developers.openai.com/plugins/deploy/desktop-only).
La publication d’un plugin local dans l’annuaire OpenAI a ses propres conditions
d’accès ; ce projet ne constitue pas une publication dans cet annuaire.

Le serveur HTTP exige un jeton, valide Host et Origin, limite les requêtes et
expire les sessions. Il écoute uniquement localhost. Il peut aussi être lancé
explicitement pour un client géré séparément :

```powershell
.\deckforge-mcp.exe --http 127.0.0.1:9846
```

Ne publie ni `mcp-http-token.txt` ni `mcp-connection.json` dans un dépôt ou une note.

## Outils

| Outil | Effet |
|---|---|
| `deckforge_get_state` | Lire la connexion, les profils et l’état des touches |
| `deckforge_get_profile` | Lire les actions d’un profil |
| `deckforge_create_profile` | Proposer un nouveau profil vide |
| `deckforge_configure_key` | Proposer la configuration d’une touche |
| `deckforge_simulate_macro` | Simuler une itération sans effet sur le PC |
| `deckforge_request_run` | Demander une exécution à confirmer dans l’app |
| `deckforge_stop_macro` | Arrêter la macro et mettre les touches en pause |
| `deckforge_proposal_status` | Lire l’état d’une proposition |

Positions : **1 à 15**, de gauche à droite et de haut en bas.
Étapes : `delay`, `hotkey`, `text`, `launch`, `url`, `wait_window`, `media`, `profile`.
Les schémas détaillés sont fournis par `tools/list`.

Exemple de proposition pour `deckforge_configure_key` :

```json
{
  "profile": "Bureau",
  "key": 2,
  "label": "Lecture / pause",
  "repeat": 1,
  "steps": [{"type": "media", "command": "play_pause"}]
}
```

La proposition s’affiche dans DeckForge avec ses actions. **Valider** applique
la configuration, ou démarre trois secondes avant une exécution. **Refuser** ne
change rien. Une proposition devient obsolète si la configuration a changé.
Expiration : dix minutes ; file limitée à dix propositions.

Les erreurs d’outils utilisent `isError: true`. Les réessais identiques sont mis
en cache pour éviter les doublons. `approved` signifie accepté par l’utilisateur,
et ne garantit pas l’achèvement des actions dans les applications externes.
