# Connecter une IA à DeckForge

Le connecteur propose **11 outils MCP**, via stdio ou Streamable HTTP local.
DeckForge valide les profils et exécute les actions. Il n’appelle aucun modèle
lui-même et ne demande aucune clé OpenAI/Anthropic.

## Connecter en 1 clic

Dans DeckForge, clique **IA : désactivée** dans la barre, active **Autoriser les IA connectées**, puis **Connecter une IA**. Ce choix est conservé dans `preferences.json` ; par défaut l'assistant reste désactivé. **IA : connectée** permet de le désactiver en un clic. La clé locale change à chaque lancement ; la validation des propositions et l'attente de trois secondes avant une exécution restent obligatoires.

Choisis une carte : Claude Desktop, Claude Code, Codex CLI ou Cursor. **Configurer automatiquement** ne remplace que l'entrée DeckForge ; les autres réglages sont conservés. Une sauvegarde `.bak-deckforge` est créée avant chaque modification. **Retirer** supprime uniquement l'entrée DeckForge. Redémarre le client après configuration. Pour Claude Code, si la CLI est trouvée, DeckForge lance `claude mcp add --scope user` (et retire l'ancienne entrée si elle existe) ; sinon, il fusionne `%USERPROFILE%\.claude.json`. Aucun jeton ni clé n'est copié vers ces fichiers. Le connecteur lit son descripteur à côté de `deckforge-mcp.exe`, même lancé depuis un autre dossier.

**Tester la connexion** interroge le connecteur stdio (`initialize` puis `tools/list`) et attend 11 outils. Ce test n'exécute aucune macro. La carte **Autre client** copie le JSON standard.

Pour ChatGPT, la carte décrit le tunnel sécurisé : ouvrir la documentation, copier le chemin du connecteur stdio, puis ChatGPT → Plugins → Ajouter un serveur MCP → Tunnel. Ce parcours dépend du compte ChatGPT et reste à vérifier ; DeckForge n'expose pas son port HTTP sur le réseau.
## Client local : Claude, Codex et autres clients stdio

1. Ouvre DeckForge, puis **Réglages → Assistant IA / MCP**, et active l’assistant.
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
Seule une action explicite sur une carte modifie le fichier du client correspondant.

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
| `deckforge_get_audio_devices` | Lire les périphériques audio et la sélection active |
| `deckforge_get_icons` | Chercher les icônes locales et les symboles audio intégrés |
| `deckforge_create_profile` | Proposer un nouveau profil vide |
| `deckforge_create_page` | Proposer une page ou un dossier |
| `deckforge_configure_key` | Proposer la configuration d’une touche |
| `deckforge_simulate_macro` | Simuler une itération sans effet sur le PC |
| `deckforge_request_run` | Demander une exécution à confirmer dans l’app |
| `deckforge_stop_macro` | Arrêter la macro et mettre les touches en pause |
| `deckforge_proposal_status` | Lire l’état d’une proposition |

Positions : **1 à 15**, de gauche à droite et de haut en bas.
Étapes : `delay`, `hotkey`, `text`, `launch`, `url`, `wait_window`, `media`, `profile`,
`audio_switch`, `if_window`, `repeat`, `set_variable`, `if_variable`, `auto_hotkey`,
ainsi que les actions de navigation décrites ci-dessous.
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


## Pages et dossiers en 0.3

Les outils configure_key, simulate_macro et request_run acceptent le champ optionnel
`page` : identifiant d’une page ou d’un dossier existant ; `""` ou champ absent vise
la page principale du profil. get_profile expose les contenus et identifiants.
Les étapes `page` et `folder` prennent `id`; `back`, `page_next` et `page_previous`
n’ont aucun autre champ. Une touche de navigation contient une seule étape, avec
repeat=1. Les pages se créent dans l’app ou par une proposition
`deckforge_create_page`. Une demande d’exécution de navigation nécessite
que le profil et la page concernés soient déjà ouverts dans l’app.

## Créer une automatisation avec les blocs (0.4.1)

Dans l’Atelier, clique **Créer avec mon IA**. Le champ prépare une demande avec le profil, la page et la touche actuellement sélectionnés. Copie-la dans ton client IA connecté à DeckForge. Ce champ ne lance pas de modèle local.

L’IA peut appeler `deckforge_get_audio_devices`, `deckforge_get_icons`, puis `deckforge_configure_key` pour proposer une macro avec des conditions, répétitions, variables et actions audio. `deckforge_create_page` propose une page ou un dossier. Les modifications attendent toujours **Valider cette proposition** dans DeckForge ; cette validation ne lance pas la macro.

Pour chaque état audio, `first_icon` et `second_icon` acceptent un chemin PNG/JPEG/SVG ou `builtin:headphones`, `builtin:speakers`, `builtin:microphone`. Une chaîne vide sélectionne le symbole par défaut. `deckforge_get_icons` retourne au maximum 50 résultats, sans contenu des fichiers.

Exemple de demande : « Crée sur cette touche un bouton qui bascule entre mon casque et mes enceintes, avec une icône et un titre pour chaque état. Utilise les identifiants audio réels retournés par DeckForge. Ne lance pas la macro. »

Le guide de connexion ChatGPT est décrit dans [la documentation officielle OpenAI](https://developers.openai.com/plugins/deploy/connect-chatgpt). Pour un serveur privé local, voir [Secure MCP Tunnel](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels). La disponibilité et les autorisations dépendent du compte ; aucun compte ni tunnel n’est configuré automatiquement par DeckForge.


## Fond de touche en 0.4.3

`deckforge_configure_key` accepte `background: false` pour un écran noir ou `background: true` pour rétablir la couleur RGB mémorisée. Si ce champ est absent, le réglage actuel est conservé. Le connecteur 0.4.3 doit être rechargé dans le client IA pour exposer ce nouveau champ ; les connecteurs plus anciens peuvent continuer les opérations existantes.

La proposition `deckforge_configure_key` accepte `icon_scale` (entier facultatif de 20 à 200 %, 100 % par défaut), validé localement avant application.

## Actions natives supplémentaires

deckforge_configure_key accepte close_window (sans paramètre), mouse_click (button : left/right/middle, double : booléen), mouse_move (x/y : coordonnées entières, relative : booléen) et mouse_scroll (amount : crans -100 à 100). Ces étapes sont validées localement, sans exécution lors de la proposition. La simulation n'agit pas sur Windows ; l'export AutoHotkey refuse explicitement ces étapes non prises en charge.


Soundboard : deckforge_configure_key accepte play_sound avec path (fichier WAV/MP3 local) et olume (entier 0 à 100), ainsi que stop_sounds sans paramètre. Les fichiers ne sont ni lus ni transmis par MCP lors de la proposition : la validation reste locale et la lecture ne démarre que sur un appui après validation. deckforge_simulate_macro n'émet aucun son. L'export AutoHotkey refuse ces blocs plutôt que de les ignorer.


Actions réseau : configure_key accepte udp_send (host, port, text) et http_request (method GET/POST/PUT, url http://, body, content_type). HTTPS est refusé explicitement faute de TLS ; validation locale avant proposition, aucune requête en simulation. La réponse HTTP est journalisée sur une longueur bornée ; l’export AutoHotkey refuse ces blocs.

## Bloc `device_volume` — volume d'un périphérique

`{"type":"device_volume","input":true,"device":"default","action":"toggle_mute","step":5,"level":50}`

`input` choisit le microphone (`true`) ou la sortie (`false`). `device` désigne `default` (rôle eConsole) ou l'ID Core Audio d'un périphérique actif. Les actions `up`, `down`, `set`, `mute`, `unmute`, `toggle_mute` règlent le périphérique entier et non les sessions d'une application. Les champs omis reçoivent leurs valeurs par défaut.

## Bloc `app_volume` — volume d'une application

`{"type":"app_volume","target":"foreground","action":"up","step":5,"level":0,"include_children":true}`

- `target` : `foreground` (application au premier plan au moment de l'appui ; après le compte à rebours pour « Tester dans 3 s ») ou un nom d'exécutable (`chrome.exe`).
- `action` : `up`, `down` (en points : 40 → 45 %), `set` (utilise `level` 0-100), `mute`, `unmute`, `toggle_mute`.
- Agit seulement sur les sessions audio Windows des processus ciblés (toutes sorties actives), jamais sur le volume général ni une autre application. Même bloc que l'Atelier ; une proposition MCP reste soumise à la validation locale.
