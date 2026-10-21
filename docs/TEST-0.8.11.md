# Validation 0.8.11 — marquage du châssis

## Régression reproduite puis corrigée

- Nouveau test UI `visible_chassis_brand_is_novydeck`, utilisant le harnais réel en 1380 × 900 sur une configuration de test, sans profil personnel ni action locale.
- Avec le code de la 0.8.10 : échec attendu, texte NOVYDECK absent.
- Après remplacement du libellé peint : test réussi ; galley NOVYDECK présent et DECKFORGE absent.
- La position SVG (700,783), la police, la couleur et la géométrie du pavé sont inchangées. Le nom accessible du SVG est actualisé.

## Contrôles Windows

- `cargo build --release --locked` : succès, version 0.8.11.
- Suite locale : **384 tests réussis, 0 échec, 19 ignorés**. Tests Node MCP stdio et HTTP réussis. Test Media Foundation dépendant du bureau non exécuté par SSH.
- Quatre captures réelles 2560 × 1600 régénérées. L'inscription du châssis a été relue sur un recadrage non retouché de la capture principale.
- Installateur 0.8.11 construit ; icône de l'installateur extraite avec alpha du coin égal à 0. Mise à jour de banc 0.8.10 → 0.8.11 réussie, sans nouveau dossier et avec empreinte du fichier témoin conservée.
- Aucun changement aux actions, profils, macros, plugins, médias des touches ni contrats persistants ; pas de test D6 physique.

## Vidéo et logo

- TypeScript et ressources du projet Remotion contrôlés ; film réexporté sur le portable Windows avec les captures corrigées et la signature 0.8.11.
- MP4 H.264/AAC, 1920 × 1080, 1290 images à 30 i/s ; durée conteneur 43,008 s.
- Taille : 21 515 702 octets ; SHA-256 `a90c0ac3da0c8c5513b12a194d7f54bf252db9aaa5747783944ee8d9736cf04e`.
- Images extraites du MP4 final relues. Les ressources PNG/ICO transparentes de la 0.8.10 restent inchangées.

Les anciens tags et releases restent archivés sans modification.
