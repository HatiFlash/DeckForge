# NovyDeck 0.9.3

## English

- The in-app updater now launches its wait using a hidden console inherited by `ping`; the installer remains silent and relaunches NovyDeck. When upgrading **from 0.9.2 to 0.9.3**, the 0.9.2 launcher is still in use and its visible console may appear **one last time**.
- Plugin settings use a smaller, resizable, named window associated with the NovyDeck window when its owner is available. The plugin receives the current interface language instead of always receiving French.
- The settings window is still a separate isolated WebView2 process, for plugin isolation. Reopening settings may create another window; the existing limit of four inspector windows remains. Plugin-supplied HTML controls its own page appearance, so a completely dark page cannot be guaranteed.
- No profiles, macros, or plugin actions are changed by this presentation update. Toggle key faces now keep their on/off appearance across page changes during this app session only; on restart, they begin off. Condition-driven faces continue to follow the live condition.
- Step summaries and the default Ignore choice are translated in the English interface. Installed plugin keys show the plugin and action's readable names; the tooltip retains their technical identifiers.
- Secondary in-app windows are centered, fixed in place, non-collapsible and size-limited. Long settings content can scroll. Plugin-supplied WebView2 windows remain independently resizable and are not affected.
- The plugin store distinguishes Official, Scanned (only a clean, passed scan of the exact archive), and Tested by NovyDeck (exact version). Review or failed scans show a warning; installation remains possible with a highlighted confirmation. Automated analysis reduces risks without eliminating them.

## Français

- La mise à jour depuis l'application lance maintenant son attente avec une console masquée héritée par `ping` ; l'installateur reste silencieux et relance NovyDeck. Lors du passage **de 0.9.2 à 0.9.3**, c'est encore le code de 0.9.2 qui lance la mise à jour : sa console peut apparaître **une dernière fois**.
- Les réglages du plugin utilisent une fenêtre plus compacte, redimensionnable et nommée, rattachée à celle de NovyDeck lorsque son propriétaire est disponible. Le plugin reçoit la langue réelle de l'interface au lieu de recevoir toujours le français.
- La fenêtre reste un processus WebView2 isolé pour préserver l'isolation du plugin. La rouvrir peut créer une autre fenêtre ; la limite existante de quatre inspecteurs reste en vigueur. Le contenu HTML du plugin contrôle son propre aspect : un fond entièrement sombre ne peut pas être garanti.
- Cette mise à jour de présentation ne modifie ni les profils, ni les macros, ni les actions des plugins. L'apparence ON/OFF des touches à bascule est conservée entre les pages pendant la session seulement ; au redémarrage, elle repart sur OFF. Les faces pilotées par une condition continuent à suivre la condition en direct.
- Les résumés des étapes et le choix « Ignorer » par défaut sont traduits en anglais. Les touches de plugins installés affichent les noms lisibles du plugin et de l'action ; l'infobulle conserve les identifiants techniques.
- Les fenêtres secondaires de l'application sont centrées, fixes, non repliables et de taille limitée. Les longs réglages peuvent défiler. Les fenêtres WebView2 fournies par les plugins restent redimensionnables indépendamment.
- Le magasin distingue les badges « Officiel », « Analysé » (analyse réussie et Defender propre pour l'archive exacte) et « Testé par NovyDeck » (version exacte). Une analyse à examiner ou échouée affiche un avertissement ; l'installation reste possible après confirmation avec un avertissement coloré. Une analyse réduit les risques sans les supprimer.
