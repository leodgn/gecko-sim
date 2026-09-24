# gecko-sim

Émulateur rapide, en Rust, pour le SoC pédagogique **Gecko5** du cours cs200
(EPFL). But : remplacer le simulateur officiel `Vtb` (RTL cycle-précis,
extrêmement lent — plusieurs secondes par étape de calcul sur un programme
de taille modeste) par quelque chose d'assez rapide pour itérer sans
attendre, pour un projet perso de jeu de la vie écrit en assembleur RISC-V
dans le cadre du cours.

C'est aussi un **projet d'apprentissage de Rust** pour l'auteur. Privilégier
la clarté et l'idiomatique Rust à la performance à tout prix — le volume de
calcul réel est faible (quelques dizaines de milliers d'instructions RISC-V
par étape de jeu), la vitesse de Rust est de toute façon très confortable
pour ce cas d'usage.

## Décision de scope (déjà prise, ne pas remettre en question sans redemander)

**Ne pas réécrire le cœur RV32I à la main.** Réutiliser une bibliothèque Rust
existante pour le décodage/exécution des instructions, et construire
soi-même (c'est le vrai contenu du projet) :
- le chargement du binaire compilé (`.bin`, voir `ressources/mmio.ld` pour
  l'adresse de base) ;
- le bus mémoire avec les périphériques Gecko5 personnalisés ;
- les périphériques eux-mêmes (`LEDS`, `SEVEN_SEGS`, `BUTTONS`, `RANDOM`) ;
- une interface graphique native (voir décision UI ci-dessous), fidèle au
  comportement de l'extension VS Code `cs200`, sans les fonctionnalités de
  debugging (pas de breakpoints/step — juste "on allume, ça tourne").

Design complet et verrouillé :
`docs/superpowers/specs/2026-09-24-gecko-sim-design.md`. Feuille de route
d'implémentation (pédagogique, pas de code) : `TODO.md`.

## Décision UI (déjà prise, ne pas remettre en question sans redemander)

**UI graphique native en Rust avec `egui`/`eframe`.** Écartée volontairement :
une intégration par protocole **Debug Adapter Protocol (DAP)** qui aurait
permis de brancher l'extension VS Code `cs200` existante directement sur
notre émulateur (vérifié : `Vtb`, le simulateur officiel, lie `libcppdap` et
implémente un serveur DAP ; l'extension est un client DAP générique qui
relaie un événement custom `boardUpdate` et une requête custom `updateInput`
à une webview Svelte). Techniquement viable, mais l'auteur préfère construire
sa propre UI plutôt que dépendre de ce protocole/de VS Code.

**Comportement voulu** : fidèle à l'extension `cs200` (grille de LEDs
12×10×3 couleurs, 4 afficheurs 7-segments, pavé directionnel + 5 boutons,
dip switches pour la fidélité visuelle mais non câblés à une adresse MMIO),
boutons pilotés à la souris (pas au clavier — c'est ce que fait l'extension
de référence). Pas de debugging, le programme démarre immédiatement au
lancement (`PC = 0x80000000`) et tourne en continu, comme une vraie carte
qu'on allume.

## Mode de travail (déjà pris, ne pas remettre en question sans redemander)

**L'auteur écrit tout le code lui-même**, dans un but d'apprentissage Rust.
Le rôle de l'assistant est de guider (expliquer le *comment*, review, aider
à débloquer), pas d'écrire l'implémentation à la place de l'auteur — sauf
demande explicite ponctuelle.

Concrètement, étape par étape (suivre l'ordre de `TODO.md`) :
1. L'assistant écrit les **tests** de l'étape en cours (et seulement les
   tests — pas l'implémentation).
2. L'auteur écrit le code qui fait passer ces tests.
3. L'assistant review, explique les concepts Rust utiles si besoin, puis
   passe à l'étape suivante une fois les tests verts.

### Cœur RV32I : piste retenue

**[`lib-rv32`](https://github.com/trmckay/lib-rv32)** (MIT, `trmckay`). Exécute
les instructions contre n'importe quelle mémoire/banc de registres qui
implémente les traits `lib_rv32_common::traits::{Memory, RegisterFile}` —
c'est exactement le point d'accroche nécessaire pour brancher les
périphériques personnalisés dans `Memory::load`/`Memory::store`.

**Pas encore vérifié** (à faire en tout début de projet, avant de s'engager) :
- la signature exacte de `Memory`/`RegisterFile` (le README ne les montre
  pas en clair, il faut lire le code source) ;
- si la crate gère un sous-ensemble RV32I pur proprement (elle annonce
  `rv32imac` — I+M+A+C ; comme le programme cible n'utilise que la base I,
  ça devrait passer, mais à confirmer en testant) ;
- l'activité récente du dépôt (pas de date de dernier commit trouvée lors
  de la recherche initiale).

Alternative explorée mais moins bien adaptée : **`riscv-rust`**
(`takahirox`, MIT) — plus complet (démarre Linux/xv6), mais construit autour
de périphériques standards (UART, PLIC...), moins direct à détourner pour
des adresses MMIO propres au Gecko5.

Si `lib-rv32` s'avère inutilisable (API trop rigide, abandonné, ne gère pas
proprement RV32I seul), redemander avant de basculer sur un cœur écrit à la
main — ce serait un changement de scope majeur (voir la conversation
d'origine : concrètement +1 à 3 semaines de travail en plus).

## Dossier `ressources/`

- `GameOfLife.pdf` — énoncé complet du labo (25 pages). Source faisant foi
  en cas de doute sur le comportement attendu.
- `hardware-spec.md` — résumé condensé et vérifié du plan mémoire et des
  périphériques (`LEDS`, `SEVEN_SEGS`, `BUTTONS`, `RANDOM`). **Commencer par
  ce fichier**, ne relire le PDF que si un détail y manque ou semble
  incohérent.
- `mmio.ld` — linker script du template du cours (adresse de chargement du
  binaire : `0x80000000`).
- `gol.s` — l'implémentation assembleur du jeu de la vie de l'auteur
  (fonctionnelle, testée, mais pas garantie sans bug). Utile comme :
  - binaire de test réel une fois assemblé (`riscv64-unknown-elf-gcc
    -march=rv32i -mabi=ilp32 ...`, voir le Makefile du labo, non copié ici) ;
  - exemple concret de séquences d'écriture dans les registres matériels
    (`LEDS`, `BUTTONS`, etc.), utile pour vérifier que l'émulateur les
    interprète comme attendu ;
  - source de motifs de test connus : `seed0` (au fond du fichier) contient
    trois formes stables classiques (deux blocs 2×2, une ruche) qui doivent
    rester **parfaitement immobiles** d'une génération à l'autre — bon test
    de non-régression pour la logique du jeu si jamais l'émulateur sert
    aussi à valider `gol.s` lui-même, pas seulement à l'exécuter vite.

## Historique utile

Ce projet est né d'une frustration très concrete : `update_gsa` (la
génération suivante du jeu de la vie) prenait 21 secondes par étape dans
`Vtb`, réduit à 2,43 secondes après optimisation de l'assembleur — jugé
encore trop lent pour itérer confortablement. Pas de detail supplémentaire
nécessaire ici ; si une question porte sur *pourquoi* telle ou telle
contrainte matérielle existe, `hardware-spec.md` et le PDF sont les sources
à consulter, pas cet historique.
