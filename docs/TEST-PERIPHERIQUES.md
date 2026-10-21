# Tester un périphérique avec NovyDeck

Ferme complètement Elgato Stream Deck, Stream Dock et FIFINE Control Deck (y compris dans la zone de notification) avant de lancer NovyDeck. Ne teste qu'un appareil à la fois.

1. Branche le périphérique, ouvre NovyDeck puis **Réglages → Tester mon périphérique…**. Note le modèle détecté et le statut « expérimental » le cas échéant.
2. Clique sur **Lancer le test** : les touches affichent leurs numéros. Vérifie leur orientation et réponds à « Les numéros sont-ils à l’endroit ? ».
3. Appuie sur toutes les touches dans l'ordre indiqué, de 1 à N. Le logiciel enregistre le paquet brut, la touche décodée et celle attendue. Les macros restent suspendues ; ne les réactive pas avant d'avoir arrêté le test.
4. Observe la variation de luminosité 30 % puis 100 % après la dernière touche. Le rapport confirme l'envoi des commandes, pas leur effet visuel : précise dans ton message si tu as vu la variation.
5. Clique sur **Enregistrer** et envoie le fichier `rapport-test-novydeck.txt` au responsable du test (ou **Copier** et colle son texte). Le rapport contient le diagnostic HID et peut contenir des informations sur les périphériques USB présents : relis-le avant tout envoi.

Si une touche ne répond pas, arrête le test et joins tout de même le rapport avec le numéro attendu et ce que tu vois à l'écran. En cas de conflit d'accès, ferme aussi les logiciels constructeurs puis reconnecte le périphérique. Les modèles expérimentaux ne sont pas validés sur le matériel des collègues.
