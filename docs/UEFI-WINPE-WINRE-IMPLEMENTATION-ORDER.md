# Priorités d'implémentation : UEFI, WinPE, WinRE et Windows installé

État initial de ce plan : **NOT_VALIDATED**. Ce document fixe l'ordre de développement et les preuves requises ; il ne prétend pas qu'une voix fonctionne déjà sur la machine physique.

## Ordre non négociable

1. **UEFI natif : lecteur d'écran avant le démarrage de Windows.**
2. **WinPE / Windows Setup : installation accessible au clavier et à la voix.**
3. **WinRE : récupération et dépannage accessibles.**
4. **Windows installé : continuité de l'accessibilité, pilotes, UI Automation et lecteurs d'écran.**

La réussite d'une couche ne valide jamais les couches suivantes. QEMU/OVMF et VMware sont des preuves de simulation, pas des preuves physiques.

## 1. UEFI natif — priorité P0

### Architecture cible

- Conserver le chargeur de référence et le backend audio fonctionnel comme artefacts « golden » en lecture seule.
- Développer le lecteur d'écran sur une voie d'intégration séparée ; comparer les empreintes SHA-256 avant/après.
- Énumérer les contrôles à partir des packages HII Forms, analyser l'IFR avec vérification stricte des longueurs, résoudre les chaînes HII et construire un arbre sémantique borné.
- Donner à chaque contrôle une identité stable ; annoncer libellé, valeur non secrète, rôle, état, position et aide.
- Supporter navigation précédente/suivante, début/fin, pages, navigation par rôle, recherche par initiale et sélecteur d'élément.
- Ne jamais prononcer les valeurs de mot de passe. Les contrôles désactivés restent identifiables mais non activables par l'adaptateur.
- Utiliser la couche audio native existante uniquement si elle est déjà validée pour le chemin concerné. Ne pas supposer qu'un pilote HDA générique ou un périphérique audio UEFI existe sur tous les firmwares.
- Détecter explicitement les limites : HII ne décrit pas nécessairement tous les écrans OEM, les dialogues graphiques, les écrans de démarrage ou les menus de démarrage tiers.

### Garde-fous

- Aucun flash BIOS, aucune écriture NVRAM/variables de démarrage, aucun changement Secure Boot, aucune écriture automatique sur USB et aucun redémarrage automatique.
- Aucun remplacement du chargeur de référence tant que les tests du cœur, de l'adaptateur HII/IFR, du planificateur vocal et de l'audio ne sont pas verts.
- Les tests virtuels doivent vérifier les marqueurs sémantiques et les régressions ; le test physique est un gate manuel distinct.

### Critères d'acceptation UEFI

- PASS-logiciel : tests unitaires, analyse statique et sanitizers réussis ; données IFR malformées rejetées sans corrompre la session active.
- PASS-virtuel : démarrage UEFI simulé, navigation et événements vocaux observables, artefact de test identifié par SHA-256.
- PASS-physique : opérateur confirme la voix réellement audible avant Windows, teste les touches et documente modèle, BIOS, sortie audio, commit et empreinte de l'artefact.
- Toute étape non exécutée reste NOT_VALIDATED ; un journal de compilation ne prouve pas une sortie sonore.

## 2. WinPE / Windows Setup — priorité P1

WinPE est un environnement distinct de Windows installé. La présence de Narrator dans le système installé ne prouve ni sa disponibilité ni son fonctionnement dans WinPE ou Windows Setup.

### Procédure d'ingénierie

1. Inventorier l'ISO source en lecture seule : SHA-256, version/build, architecture, langue, `boot.wim`, `install.wim`/`install.esd`, chargeurs EFI et fichiers de configuration.
2. Déterminer l'index exact de `boot.wim` qui contient Windows PE/Setup ; relever les packages, pilotes, langues et fichiers d'accessibilité existants avant toute modification.
3. Créer une copie de travail distincte. Ne jamais modifier l'ISO d'origine.
4. Identifier dans la documentation officielle de la version d'ADK/WinPE utilisée les composants de voix et d'accessibilité réellement pris en charge. Les optional components doivent correspondre à la version/build et à la langue de l'image. Ne pas copier au hasard des DLL depuis Windows installé.
5. Servicer uniquement la copie de travail avec les outils Microsoft appropriés ; journaliser chaque package ajouté, son origine, sa version, sa langue et son empreinte. Si la voix Narrator ne peut pas être intégrée proprement par une méthode prise en charge, enregistrer le blocage plutôt que fabriquer une image supposée accessible.
6. Construire l'ISO de test dans un répertoire séparé. Valider les fichiers EFI, BCD, `boot.wim`, les index WIM et les empreintes ; comparer le contenu source et la copie de travail.
7. Démarrer d'abord dans une VM isolée pour valider le chemin de démarrage et la navigation. Ce résultat n'est pas une preuve d'audio physique.
8. Tester ensuite sur un support dédié uniquement après accord explicite de l'opérateur ; aucune écriture sur clé USB ni aucun redémarrage n'est automatique.

### Scénarios WinPE/Setup

- Voix activable dès l'écran initial, avant toute saisie requise.
- Navigation clavier uniquement entre langue, région, disposition clavier, installation, licence, choix du disque, erreurs et retour.
- Annonce cohérente du focus, du nom du contrôle, du rôle, de l'état, de l'aide et des messages d'erreur.
- Retour vocal en cas de pilote manquant, de stockage indisponible ou d'échec de démarrage.
- Aucun secret prononcé ni transmis dans les journaux.
- Si un écran n'expose pas sa sémantique, consigner précisément le contrôle et le chemin concerné ; ne pas le déclarer accessible par simple présence de Narrator.

## 3. WinRE — priorité P2

WinRE est une image de récupération séparée et peut avoir une configuration différente de WinPE/Setup.

- Inventorier séparément l'image WinRE réellement utilisée, son état, sa version, sa langue, ses pilotes et ses outils de récupération.
- Évaluer la disponibilité de Narrator et des options d'accessibilité par les mécanismes pris en charge pour cette version de Windows.
- Tester clavier et voix dans les écrans de récupération, dépannage, options avancées, désinstallation des mises à jour, paramètres de démarrage et invite de commandes lorsqu'elle est disponible.
- Vérifier que le lecteur de récupération, BitLocker et les demandes de clé de récupération restent utilisables sans divulguer la clé.
- Préserver la configuration de récupération existante. Pas de réenregistrement de WinRE, modification de partition, changement BCD ou commande de récupération destructive dans l'audit.
- Toute modification ultérieure doit se faire sur une copie/instance de test, avec procédure de retour arrière et confirmation explicite.

## 4. Windows installé — priorité P3

- Vérifier Narrator, NVDA et JAWS séparément ; noter les versions et les résultats propres à chaque lecteur.
- Prioriser les API sémantiques/UI Automation, le focus clavier, l'ordre de tabulation, les libellés, états et notifications ; l'OCR n'est qu'un secours explicitement signalé.
- Contrôler la continuité entre l'écran de connexion, le bureau, les paramètres système, le gestionnaire de périphériques et les outils de récupération.
- Distinguer les tests logiciels des confirmations d'écoute humaine.

## Format de preuve requis

Chaque gate doit enregistrer : identifiant, statut (`PASS`, `FAIL`, `PARTIAL`, `BLOCKED`, `NOT_VALIDATED`), commit, machine/environnement, version d'image, SHA-256, date UTC, étapes exactes, résultat observé et opérateur.

Ne jamais passer un statut à PASS sur la base d'une intention, d'un build seul ou d'une simulation lorsqu'une preuve physique est exigée.
