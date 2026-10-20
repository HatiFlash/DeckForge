# Backport egui-wgpu 0.31.1 — soumission lors d'une sortie précoce

Origine : crate `egui-wgpu` 0.31.1 publiée sur crates.io ; correctif ciblé du [PR officiel #7928](https://github.com/emilk/egui/pull/7928), pour le [défaut #7434](https://github.com/emilk/egui/issues/7434). La seule modification fonctionnelle est dans `src/winit.rs` : un garde soumet une commande vide à la file WGPU si une frame se termine après des écritures de buffers mais avant `Queue::submit`. Le chemin normal ne soumet qu'une fois. Aucune migration des API egui/eframe 0.31.1.

Vérification du chemin ciblé (VM Windows, programme de banc séparé, sans D6 ni actions DeckForge) : en forçant **uniquement dans le banc** une sortie juste après `update_buffers`, la mémoire de buffers WGPU est montée de 917 504 à 3 765 960 704 octets en 9 s sans le garde ; elle est restée à 917 504 octets durant 14 s avec le garde. Cette injection de test **n'est pas présente** dans la version livrée.

La minimisation ordinaire sur la VM n'a pas reproduit cette croissance : 917 504 octets stables pendant 90 s. La cause des 35 Go signalés sur le PC de l'utilisateur reste donc **non démontrée**. Une fenêtre recouverte par un jeu ne garantit pas un échec de `get_current_texture`. Mesurer sur la version installée si le symptôme réapparaît ; ne pas revendiquer un correctif mémoire confirmé sur le PC ou le D6.

Licence : MIT OR Apache-2.0 ; voir `LICENSE-MIT` et `LICENSE-APACHE`.
